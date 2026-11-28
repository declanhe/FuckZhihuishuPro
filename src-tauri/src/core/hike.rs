//! 校内学分课（hike）—— 对应 fuckZHS `fucker.py` 的 hike 方法组。
//!
//! 与共享课不同，hike 接口用 GET + MD5 签名（见 `crypto::sign_study_record`）。

use std::sync::Arc;

use rand::Rng;
use serde_json::{json, Value};

use super::error::{Result, ZhsError};
use super::events::{sleep_or_stop, CourseEvent, Emitter, PauseToken, StopHandle, StopToken};
use super::session::{now_ms, ZhsSession};

const COURSE_LIST_URL: &str = "https://hikeservice.zhihuishu.com/student/course/aided/getMyCourseList";
const MENU_TREE_URL: &str =
    "https://studyresources.zhihuishu.com/studyResources/stuResouce/queryResourceMenuTree";
const VIEW_FILE_URL: &str =
    "https://studyresources.zhihuishu.com/studyResources/stuResouce/stuViewFile";
const SAVE_RECORD_URL: &str = "https://hike-teaching.zhihuishu.com/stuStudy/saveStuStudyRecord";
const INIT_VIDEO_URL: &str = "https://newbase.zhihuishu.com/video/initVideo";

const HIKE_HEADERS: &[(&str, &str)] = &[
    ("Origin", "https://hike.zhihuishu.com"),
    ("Referer", "https://hike.zhihuishu.com/"),
];

fn json_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// 获取全部校内课列表（对应 `getHikeList`），返回 (课程名, courseId)。
pub async fn get_hike_list(session: &ZhsSession) -> Result<Vec<(String, String)>> {
    let uuid = session
        .uuid()
        .ok_or(ZhsError::NotLoggedIn)?;
    // 对应 Python: datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"
    let data = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    let resp = session
        .api_query(
            COURSE_LIST_URL,
            Some(&json!({"uuid": uuid, "data": data})),
            reqwest::Method::GET,
            // 原版 getHikeList 前只 _sessionReady()（基础头，无 Origin/Referer）
            &[],
        )
        .await?;
    let mut courses = Vec::new();
    if let Some(list) = resp
        .pointer("/result/startInngcourseList")
        .and_then(|l| l.as_array())
    {
        for c in list {
            let name = c["courseName"].as_str().unwrap_or("").to_string();
            let id = c["courseId"]
                .as_i64()
                .map(|i| i.to_string())
                .or_else(|| c["courseId"].as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            if !id.is_empty() {
                courses.push((name, id));
            }
        }
    }
    Ok(courses)
}

/// hike 刷课执行器（对应 hike 方法组 + context 缓存）。
pub struct HikeRunner {
    pub session: Arc<ZhsSession>,
    /// 播放倍速（<= 0 时取平台最高 1.25）
    pub speed: f64,
    /// 完成阈值
    pub end_thre: f64,
    /// 单课时限（秒，0 = 不限）
    pub limit_seconds: u64,
    pub stop: StopToken,
    /// 全局暂停（引擎级）：暂停时视频循环冻结进度
    pub pause: PauseToken,
    root: Option<Value>,
    fucked_time: u64,
    /// 伪装观看后台任务的终止句柄（切视频/停止时掐断旧任务，防残留请求）
    fake_watch_guard: std::sync::Mutex<Option<Arc<StopHandle>>>,
}

impl HikeRunner {
    pub fn new(
        session: Arc<ZhsSession>,
        speed: f64,
        end_thre: f64,
        limit_minutes: u64,
        stop: StopToken,
        pause: PauseToken,
    ) -> Self {
        Self {
            session,
            speed: if speed > 0.0 { speed } else { 1.25 },
            end_thre,
            limit_seconds: limit_minutes * 60,
            stop,
            pause,
            root: None,
            fucked_time: 0,
            fake_watch_guard: std::sync::Mutex::new(None),
        }
    }

    /// 获取（或缓存）课程目录树。
    async fn root(&mut self, course_id: &str, emit: &Emitter) -> Result<Value> {
        if let Some(r) = &self.root {
            return Ok(r.clone());
        }
        emit.emit(CourseEvent::logc(
            "loading_course",
            format!("正在加载课程 {course_id} …"),
            course_id,
        ));
        let resp = self
            .session
            .hike_query(
                MENU_TREE_URL,
                &json!({"courseId": course_id}),
                false,
                Some(200),
                HIKE_HEADERS,
            )
            .await?;
        let rt = resp["rt"].clone();
        self.root = Some(rt.clone());
        Ok(rt)
    }

    /// 刷整门课程（对应 `fuckHikeCourse` + `_traverse`）。
    pub async fn run_course(&mut self, course_id: &str, emit: &Emitter) -> Result<()> {
        // 加载目录阶段也可能 -12，必须发 CaptchaRequired 让前端展示恢复引导
        let root = match self.root(course_id, emit).await {
            Ok(r) => r,
            Err(ZhsError::Captcha) => {
                emit.emit(CourseEvent::CaptchaRequired);
                return Err(ZhsError::Captcha);
            }
            Err(e) => return Err(e),
        };
        // queryResourceMenuTree 的 rt 直接是章节数组（实测），包裹成伪根节点
        // 统一交给 traverse 递归处理
        let pseudo_root = json!({
            "name": "",
            "childList": if root.is_array() { root } else { json!([root]) },
        });
        let result = self.traverse(course_id, &pseudo_root, emit).await;
        match result {
            Ok(()) => emit.emit(CourseEvent::done(
                "course_finished",
                format!("课程 {course_id} 已完成"),
            )),
            Err(ZhsError::Stopped) => return Err(ZhsError::Stopped),
            Err(ZhsError::Captcha) => {
                emit.emit(CourseEvent::CaptchaRequired);
                return Err(ZhsError::Captcha);
            }
            Err(e) => emit.emit(CourseEvent::fail(e.to_string())),
        }
        Ok(())
    }

    /// 递归遍历目录树（对应 `_traverse`）。
    fn traverse<'a>(
        &'a mut self,
        course_id: &'a str,
        node: &'a Value,
        emit: &'a Emitter,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(children) = node["childList"].as_array() {
                // 章节
                emit.emit(CourseEvent::Chapter {
                    name: node["name"].as_str().unwrap_or("").to_string(),
                });
                for child in children {
                    if self.stop.is_stopped() {
                        return Err(ZhsError::Stopped);
                    }
                    self.traverse(course_id, child, emit).await?;
                }
                return Ok(());
            }
            // 文件节点
            let name = node["name"].as_str().unwrap_or("").to_string();
            let study_time = node["studyTime"].as_f64().unwrap_or(0.0);
            let total_time = node["totalTime"].as_f64().unwrap_or(0.0);
            let data_type = node["dataType"].as_i64();
            let file_id = match &node["id"] {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            };
            if file_id.is_empty() {
                return Ok(());
            }
            // 全局暂停：等恢复后再处理下一个资源
            self.pause.wait_resume().await;
            // 已看完则跳过（与原版一致：total_time=0 时 study_time>=0 成立也跳过）
            if study_time >= total_time * self.end_thre {
                emit.emit(CourseEvent::Skipped {
                    name: name.clone(),
                    reason: "watched".into(),
                });
                return Ok(());
            }
            match data_type {
                Some(3) => {
                    // 视频
                    let r = self
                        .run_video(course_id, &file_id, &name, study_time, emit)
                        .await;
                    match r {
                        Ok(()) => {}
                        Err(ZhsError::Stopped) => return Err(ZhsError::Stopped),
                        // 【关键】-12 账号限制必须立刻停止整门课：继续请求后面的视频
                        // 会加重风控（原版也是 CaptchaException 直接 return）
                        Err(ZhsError::Captcha) => {
                            emit.emit(CourseEvent::CaptchaRequired);
                            return Err(ZhsError::Captcha);
                        }
                        Err(ZhsError::TimeLimit) => {
                            emit.emit(CourseEvent::LimitReached);
                            return Ok(());
                        }
                        Err(e) => emit.emit(CourseEvent::fail(format!("视频 {name} 失败: {e}"))),
                    }
                }
                None => {
                    emit.emit(CourseEvent::Skipped {
                        name: name.clone(),
                        reason: "quiz".into(),
                    });
                }
                Some(_) => {
                    // 其他资源类型：访问一次即可（对应 fuckFile）。
                    // 但 -12 必须立刻停止，不能当作普通失败忽略（会加重风控）。
                    if let Err(ZhsError::Captcha) = self.stu_view_file(course_id, &file_id).await {
                        emit.emit(CourseEvent::CaptchaRequired);
                        return Err(ZhsError::Captcha);
                    }
                    let view_sleep = rand::rng().random::<f64>() * 2.0 + 1.0;
                    if sleep_or_stop(view_sleep, &self.stop).await {
                        return Err(ZhsError::Stopped);
                    }
                    emit.emit(CourseEvent::Skipped {
                        name: name.clone(),
                        reason: "resource".into(),
                    });
                }
            }
            Ok(())
        })
    }

    /// 刷单个视频（对应 `fuckHikeVideo`）。
    pub async fn run_video(
        &mut self,
        course_id: &str,
        file_id: &str,
        name: &str,
        prev_progress: f64,
        emit: &Emitter,
    ) -> Result<()> {
        // 全局暂停：等恢复后再进入下一个资源
        self.pause.wait_resume().await;
        if self.limit_seconds > 0 && self.fucked_time >= self.limit_seconds {
            return Err(ZhsError::TimeLimit);
        }
        let file_info = self.stu_view_file(course_id, file_id).await?;
        let data_id = file_info
            .pointer("/dataId")
            .map(json_str)
            .unwrap_or_default();
        // 与原版 int(file_info.totalTime) 一致，避免小数导致 endWatchTime 差 1 秒
        let total_time = file_info["totalTime"].as_f64().unwrap_or(0.0).trunc();

        // 伪装真实观看
        self.watch_video_fake(&data_id);

        let start_date = now_ms();
        let mut prev_time = prev_progress;
        let mut played_time = prev_time;
        let end_time = (total_time * self.end_thre).max(1.0);
        let interval: i64 = 30;
        let mut end_retries = 0u32; // 终点上报失败重试计数（防无限重复提交）

        emit.emit(CourseEvent::VideoStart {
            video_id: file_id.to_string(),
            name: name.to_string(),
            played: played_time,
            total: total_time,
        });

        while played_time <= end_time {
            if sleep_or_stop(1.0, &self.stop).await {
                return Err(ZhsError::Stopped);
            }
            // 全局暂停（用户操作）：冻结进度，不上报、不计入时限
            if self.pause.is_paused() {
                emit.emit(CourseEvent::VideoProgress {
                    video_id: file_id.to_string(),
                    played: played_time,
                    total: end_time,
                    paused: true,
                    quiz: false,
                });
                continue;
            }
            self.fucked_time += 1;
            if self.limit_seconds > 0 && self.fucked_time >= self.limit_seconds {
                return Err(ZhsError::TimeLimit);
            }
            played_time = (played_time + self.speed).min(end_time);
            let reached_end = played_time >= end_time;
            // 到达间隔或终点：上报进度（失败不终止，下个周期重试）
            if reached_end || ((played_time - prev_time) as i64) % interval == 0 {
                match self
                    .save_study_record(course_id, file_id, played_time, prev_time, start_date)
                    .await
                {
                    Ok(ret_time) => {
                        // 【防 -8 进度倒退】服务器返回的累计时长若异常小于当前值，
                        // 不采信（否则下次上报 endWatchTime 倒退会被判"学习总时长下降"）。
                        if ret_time >= prev_time {
                            prev_time = ret_time;
                            played_time = ret_time;
                        } else {
                            // 服务器值倒退（异常）：上报其实已成功，不重复提交同一段进度。
                            // 保留 prev_time 不动，直接结束该视频避免陷入重复上报。
                            emit.emit(CourseEvent::warn(
                                "服务器返回的进度倒退，已跳过重复上报",
                            ));
                            break;
                        }
                        // 【风控关键】终点已成功上报，必须退出循环！
                        // 否则 played 被钉死在 end_time，下面的 while 条件恒真，
                        // 每秒重复上报同一段进度 → 服务器判定异常 → 账号级 -12 限制。
                        // 原版的 while <= 写法在 ret_time > end_time 时侥幸退出，
                        // 我们的 min() 步进把它钉死，必须显式 break。
                        if reached_end {
                            break;
                        }
                    }
                    Err(ZhsError::Stopped) => return Err(ZhsError::Stopped),
                    Err(e) => {
                        emit.emit(CourseEvent::warn(format!(
                            "进度上报失败（将继续尝试）: {e}"
                        )));
                        // 终点上报失败：有限次重试（最多 5 次，间隔 3s）。
                        // 绝不能无限循环重复提交同一段进度——那是 -12 的直接诱因。
                        if reached_end {
                            end_retries += 1;
                            if end_retries >= 5 {
                                emit.emit(CourseEvent::warn(
                                    "终点进度上报多次失败，跳过该视频（下轮会重刷）",
                                ));
                                break;
                            }
                            if sleep_or_stop(3.0, &self.stop).await {
                                return Err(ZhsError::Stopped);
                            }
                        }
                    }
                }
            }
            emit.emit(CourseEvent::VideoProgress {
                video_id: file_id.to_string(),
                played: played_time,
                total: end_time,
                paused: false,
                quiz: false,
            });
        }
        let tail_sleep = rand::rng().random::<f64>() + 1.0;
        if sleep_or_stop(tail_sleep, &self.stop).await {
            return Err(ZhsError::Stopped);
        }
        emit.emit(CourseEvent::VideoDone {
            video_id: file_id.to_string(),
            name: name.to_string(),
        });
        Ok(())
    }

    /// 获取文件详情（对应 `stuViewFile`）。
    pub async fn stu_view_file(&self, course_id: &str, file_id: &str) -> Result<Value> {
        let resp = self
            .session
            .hike_query(
                VIEW_FILE_URL,
                &json!({"courseId": course_id, "fileId": file_id}),
                false,
                Some(200),
                HIKE_HEADERS,
            )
            .await?;
        Ok(resp["rt"].clone())
    }

    /// 上报学习记录（对应 `saveStuStudyRecord`）。
    async fn save_study_record(
        &self,
        course_id: &str,
        file_id: &str,
        played_time: f64,
        prev_time: f64,
        start_date: i64,
    ) -> Result<f64> {
        let uuid = self.session.uuid().ok_or(ZhsError::NotLoggedIn)?;
        let params = json!({
            "uuid": uuid,
            "courseId": course_id,
            "fileId": file_id,
            "studyTotalTime": (played_time - prev_time).max(0.0) as i64,
            "startWatchTime": prev_time as i64,
            "endWatchTime": played_time as i64,
            "startDate": start_date,
            "endDate": now_ms(),
        });
        let resp = self
            .session
            .hike_query(SAVE_RECORD_URL, &params, true, Some(200), HIKE_HEADERS)
            .await?;
        let rt = &resp["rt"];
        if rt.is_null() {
            return Err(ZhsError::msg("保存学习记录失败（rt 为空）"));
        }
        Ok(rt.as_f64().unwrap_or(played_time))
    }

    /// 伪装视频观看请求（对应 `watchVideo`）。
    ///
    /// 【风控】绑定停止令牌 + 每个视频只留一个后台任务，切视频/停止时掐断旧的；
    /// 视频流只读前 64KB，避免完整下载与长时间占用连接形成异常流量。
    fn watch_video_fake(&self, video_id: &str) {
        let session = self.session.clone();
        let video_id = video_id.to_string();
        let stop = self.stop.clone();
        let pause = self.pause.clone();
        let mut guard = self.fake_watch_guard.lock().unwrap();
        if let Some(old) = guard.take() {
            old.stop();
        }
        let handle = Arc::new(StopHandle::new());
        *guard = Some(handle.clone());
        drop(guard);

        let task_stop = handle.token();
        tokio::spawn(async move {
            let query = vec![
                ("jsonpCallBack".to_string(), "result".to_string()),
                ("videoID".to_string(), video_id.clone()),
                ("_".to_string(), now_ms().to_string()),
            ];
            let text = tokio::select! {
                _ = stop.wait() => return,
                _ = task_stop.wait() => return,
                r = session.get_text(INIT_VIDEO_URL, &query, HIKE_HEADERS) => match r { Ok(t) => t, Err(_) => return },
            };
            // 暂停时等恢复而非丢弃任务：避免整个视频期间只有上报、没有视频流请求
            tokio::select! {
                _ = stop.wait() => return,
                _ = task_stop.wait() => return,
                _ = pause.wait_resume() => {}
            }
            let Some(inner) = text
                .strip_prefix("result(")
                .and_then(|t| t.strip_suffix(')'))
            else {
                return;
            };
            let Ok(v) = serde_json::from_str::<Value>(inner) else {
                return;
            };
            let Some(line_url) = v
                .pointer("/result/lines/0/lineUrl")
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
            else {
                return;
            };
            tokio::select! {
                _ = stop.wait() => {}
                _ = task_stop.wait() => {}
                _ = session.get_text_prefix(&line_url, 64 * 1024, HIKE_HEADERS) => {}
            }
        });
    }
}
