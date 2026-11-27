//! 知到共享学分课 —— 对应 fuckZHS `fucker.py` 的 zhidao 方法组。
//!
//! 核心流程（对应 `getZhidaoContext` + `fuckZhidaoCourse` + `fuckZhidaoVideo`）：
//! 1. 跨站登录（gologin）获取学习接口会话
//! 2. 拉取章节/视频清单与观看状态
//! 3. 逐视频模拟观看：每秒推进 `speed` 秒，每 30 秒向服务器上报进度
//! 4. 遇到弹题自动拉题作答（答案在题目数据中，无需 AI）

use std::collections::HashMap;
use std::sync::Arc;

use rand::Rng;
use serde_json::{json, Value};

use super::crypto::{self, VIDEO_KEY, HOME_KEY};
use super::error::{Result, ZhsError};
use super::events::{sleep_or_stop, CourseEvent, Emitter, PauseToken, StopHandle, StopToken};
use super::session::{b64_encode, ev_encode, now_ms, ZhsSession};

const LIST_URL: &str =
    "https://onlineservice-api.zhihuishu.com/gateway/t/v1/student/course/share/queryShareCourseInfo";
const GOLOGIN_URL: &str = "https://studyservice-api.zhihuishu.com/login/gologin";
const QUERY_COURSE_URL: &str = "https://studyservice-api.zhihuishu.com/gateway/t/v1/learning/queryCourse";
const VIDEO_LIST_URL: &str = "https://studyservice-api.zhihuishu.com/gateway/t/v1/learning/videolist";
const STUDY_INFO_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/learning/queryStuyInfo"; // 服务器拼写如此
const LAST_VIDEO_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/learning/queryUserRecruitIdLastVideoId";
const PRELEARNING_NOTE_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/learning/prelearningNote";
const VIDEO_POINTER_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/popupAnswer/loadVideoPointerInfo";
const POPUP_EXAM_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/popupAnswer/lessonPopupExam";
const SAVE_EXAM_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/popupAnswer/saveLessonPopupExamSaveAnswer";
const SAVE_DB_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/learning/saveDatabaseIntervalTimeV2";
const COURSEWARE_URL: &str =
    "https://studyservice-api.zhihuishu.com/gateway/t/v1/course/threeDimensionalCourseWare";
const INIT_VIDEO_URL: &str = "https://newbase.zhihuishu.com/video/initVideo";

const STUDY_HEADERS: &[(&str, &str)] = &[
    ("Origin", "https://studyh5.zhihuishu.com"),
    ("Referer", "https://studyh5.zhihuishu.com/"),
];

/// 视频小节信息（对应 fuckZHS 的 `videos[videoId]`）。
#[derive(Debug, Clone)]
pub struct ZVideo {
    /// 服务器视频 ID（`videoId`）
    pub video_id: String,
    /// 小节视频 ID（`videoSmallLessons[].id`，即 lessonVideoId）
    pub small_id: String,
    /// 所属小节（lesson）ID
    pub lesson_id: String,
    /// 所属章节 ID
    pub chapter_id: String,
    pub chapter_name: String,
    pub lesson_name: String,
    pub name: String,
    /// 视频总时长（秒）
    pub video_sec: f64,
    /// 观看状态（1 = 已看完）
    pub watch_state: f64,
    /// 已学时长（秒）
    pub study_total_time: f64,
}

/// 共享课上下文（对应 `getZhidaoContext`）。
#[derive(Debug, Clone)]
pub struct ZhidaoContext {
    pub rac_id: String,
    pub recruit_id: String,
    /// 视频清单里的 courseId（videolist.courseId）——prelearningNote/弹题用
    pub course_id: String,
    /// queryCourse 的 courseInfo.courseId——saveDatabaseIntervalTimeV2 明文 courseId 用。
    /// 原版这两个来源分开（见 fucker.py prelearningNote vs saveDatabaseIntervalTimeV2），
    /// 若服务端两者不同，混用会导致进度上报的 courseId 错误。
    pub study_course_id: String,
    pub course_name: String,
    pub videos: Vec<ZVideo>,
}

/// 获取全部共享课列表（对应 `getZhidaoList`），返回 (课程名, recruitAndCourseId)。
pub async fn get_zhidao_list(session: &ZhsSession) -> Result<Vec<(String, String)>> {
    let mut courses = Vec::new();
    let mut page = 1u64;
    let mut total = 0u64;
    loop {
        let data = json!({"status": 0, "pageNo": page, "pageSize": 5});
        let resp = session
            .zhidao_query(LIST_URL, &data, HOME_KEY, Some(200), reqwest::Method::POST, &[])
            .await?;
        let result = &resp["result"];
        if page == 1 {
            total = result["totalCount"].as_u64().unwrap_or(0);
        }
        if let Some(list) = result["courseOpenDtos"].as_array() {
            for c in list {
                let name = c["courseName"].as_str().unwrap_or("").to_string();
                let id = c["secret"].as_str().unwrap_or("").to_string();
                if !id.is_empty() {
                    courses.push((name, id));
                }
            }
        }
        let pages = total.div_ceil(5).max(1);
        if page >= pages {
            break;
        }
        page += 1;
    }
    Ok(courses)
}

/// 从 videolist 响应解析视频清单（对应 getZhidaoContext 的展开逻辑）。
fn parse_videos(chapters: &Value) -> Vec<ZVideo> {
    let mut videos: Vec<ZVideo> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for chapter in chapters["videoChapterDtos"].as_array().unwrap_or(&vec![]).iter() {
        let chapter_id = json_str(&chapter["id"]);
        let chapter_name = json_str(&chapter["name"]);
    for lesson in chapter["videoLessons"].as_array().unwrap_or(&vec![]).iter() {
        let lesson_name = json_str(&lesson["name"]);
            // 单视频小节：lesson 自带 videoId，构造一个 small lesson
            let mut smalls: Vec<Value> = Vec::new();
            if lesson.get("videoId").is_some_and(|v| !v.is_null()) {
                let mut v = lesson.clone();
                v["lessonId"] = lesson["id"].clone();
                v["id"] = json!(0);
                smalls.push(v);
            }
            if let Some(arr) = lesson["videoSmallLessons"].as_array() {
                for v in arr {
                    smalls.push(v.clone());
                }
            }
            for mut v in smalls {
                v["chapterId"] = json!(chapter_id);
                let video_id = json_str(&v["videoId"]);
                if video_id.is_empty() {
                    continue;
                }
                let zv = ZVideo {
                    video_id: video_id.clone(),
                    small_id: json_str(&v["id"]),
                    lesson_id: json_str(&v["lessonId"]),
                    chapter_id: json_str(&v["chapterId"]),
                    chapter_name: chapter_name.clone(),
                    lesson_name: lesson_name.clone(),
                    name: json_str(&v["name"]),
                    video_sec: v["videoSec"].as_f64().unwrap_or(0.0),
                    watch_state: 0.0,
                    study_total_time: 0.0,
                };
                // 与 Python dict 一致：同 videoId 覆盖
                if let Some(&i) = index.get(&video_id) {
                    videos[i] = zv;
                } else {
                    index.insert(video_id, videos.len());
                    videos.push(zv);
                }
            }
        }
    }
    videos
}

fn json_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// 知到刷课执行器（对应 `Fucker` 的 zhidao 部分方法 + context 缓存）。
pub struct ZhidaoRunner {
    pub session: Arc<ZhsSession>,
    /// 播放倍速（<= 0 时取平台最高 1.5）
    pub speed: f64,
    /// 完成阈值（对应 end_thre，默认 0.91）
    pub end_thre: f64,
    /// 单课时限（秒，0 = 不限）
    pub limit_seconds: u64,
    pub stop: StopToken,
    /// 全局暂停（引擎级）：暂停时视频循环冻结进度
    pub pause: PauseToken,
    ctx: Option<ZhidaoContext>,
    /// 本轮累计学习秒数（时限用）
    fucked_time: u64,
    /// 伪装观看后台任务的终止句柄（切视频/停止时掐断旧任务，防残留请求）
    fake_watch_guard: std::sync::Mutex<Option<Arc<StopHandle>>>,
    /// 每个视频已成功上报的最高进度（秒）。
    /// 【防 -8】会话自愈重载后 queryStudyInfo 可能返回比实际已上报更小的旧进度，
    /// 若直接采信会上报倒退值 → 服务器报 -8"学习总时长下降了"。用本表取 max 杜绝倒退。
    reported_progress: HashMap<String, f64>,
}

impl ZhidaoRunner {
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
            speed: if speed > 0.0 { speed } else { 1.5 },
            end_thre,
            limit_seconds: limit_minutes * 60,
            stop,
            pause,
            ctx: None,
            fucked_time: 0,
            fake_watch_guard: std::sync::Mutex::new(None),
            reported_progress: HashMap::new(),
        }
    }

    fn speed(&self) -> f64 {
        self.speed
    }

    /// 跨站登录（对应 `gologin`）：建立 studyservice 学习会话。
    /// 会话失效（NotLoggedIn）时重新调用即可自愈，无需用户重新扫码。
    async fn gologin(&self, rac_id: &str) -> Result<()> {
        let fromurl = format!(
            "https://studyh5.zhihuishu.com/videoStudy.html#/studyVideo?recruitAndCourseId={rac_id}"
        );
        self.session
            .http()
            .get(GOLOGIN_URL)
            .query(&[("fromurl", fromurl)])
            .headers(parse_headers(STUDY_HEADERS))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// 获取（或缓存）课程上下文。
    pub async fn context(&mut self, rac_id: &str, emit: &Emitter) -> Result<ZhidaoContext> {
        if let Some(ctx) = &self.ctx {
            if ctx.rac_id == rac_id {
                return Ok(ctx.clone());
            }
        }
        emit.emit(CourseEvent::logc(
            "loading_course",
            format!("正在加载课程 {rac_id} …"),
            rac_id,
        ));

        // 跨站登录（studyservice 学习会话；过期后重新调用即可恢复）
        self.gologin(rac_id).await?;

        // 课程信息
        let course = self
            .session
            .zhidao_query(
                QUERY_COURSE_URL,
                &json!({"recruitAndCourseId": rac_id}),
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?["data"]
            .clone();
        let recruit_id = json_str(&course["recruitId"]);

        // 章节/视频清单
        let chapters = self
            .session
            .zhidao_query(
                VIDEO_LIST_URL,
                &json!({"recruitAndCourseId": rac_id}),
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?["data"]
            .clone();
        let course_id = json_str(&chapters["courseId"]);
        // 原版 V2 上报用 queryCourse 的 courseInfo.courseId，与 videolist 的 courseId 分开取；
        // 若该字段缺失，回退到 videolist 的 courseId（实测两者常相同）。
        let study_course_id = {
            let s = json_str(&course["courseInfo"]["courseId"]);
            if s.is_empty() {
                course_id.clone()
            } else {
                s
            }
        };
        let mut videos = parse_videos(&chapters);

        // 观看状态（lessonIds/videoIds 去重）
        let lesson_ids: Vec<Value> = videos
            .iter()
            .map(|v| v.lesson_id.clone())
            .collect::<std::collections::HashSet<String>>()
            .into_iter()
            .map(Value::String)
            .collect();
        let video_ids: Vec<Value> = videos
            .iter()
            .filter(|v| !v.small_id.is_empty() && v.small_id != "0")
            .map(|v| v.small_id.clone())
            .collect::<std::collections::HashSet<String>>()
            .into_iter()
            .map(Value::String)
            .collect();
        let states = self
            .session
            .zhidao_query(
                STUDY_INFO_URL,
                &json!({
                    "lessonIds": lesson_ids,
                    "lessonVideoIds": video_ids,
                    "recruitId": recruit_id,
                }),
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?["data"]
            .clone();

        for v in videos.iter_mut() {
            // 与 Python 一致：优先 lv[small_id]，缺失则 lesson[lesson_id]
            let state = states
                .pointer(&format!("/lv/{}", v.small_id))
                .filter(|s| s.is_object())
                .or_else(|| states.pointer(&format!("/lesson/{}", v.lesson_id)))
                .filter(|s| s.is_object());
            if let Some(state) = state {
                v.watch_state = state["watchState"].as_f64().unwrap_or(0.0);
                v.study_total_time = state["studyTotalTime"].as_f64().unwrap_or(0.0);
            }
        }

        // 最近观看记录（拉一下保持行为一致，忽略返回）
        let _ = self
            .session
            .zhidao_query(
                LAST_VIDEO_URL,
                &json!({"recruitId": recruit_id}),
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await;

        let course_name = course
            .pointer("/courseInfo/name")
            .and_then(|n| n.as_str())
            .or_else(|| course.pointer("/courseInfo/enName").and_then(|n| n.as_str()))
            .unwrap_or(rac_id)
            .to_string();

        let ctx = ZhidaoContext {
            rac_id: rac_id.to_string(),
            recruit_id,
            course_id,
            study_course_id,
            course_name,
            videos,
        };
        self.ctx = Some(ctx.clone());
        Ok(ctx)
    }

    /// 刷整门课程（对应 `fuckZhidaoCourse`）。
    pub async fn run_course(&mut self, rac_id: &str, emit: &Emitter) -> Result<()> {
        let ctx = self.context(rac_id, emit).await?;
        emit.emit(CourseEvent::logc(
            "start_course",
            format!("开始刷课：{}", ctx.course_name),
            &ctx.course_name,
        ));

        for video in ctx.videos.clone().iter() {
            emit.emit(CourseEvent::Lesson {
                name: video.lesson_name.clone(),
            });
            match self.run_video(rac_id, &video.video_id, emit).await {
                Ok(()) => {}
                Err(ZhsError::Stopped) => return Err(ZhsError::Stopped),
                Err(ZhsError::Captcha) => {
                    emit.emit(CourseEvent::CaptchaRequired);
                    return Err(ZhsError::Captcha);
                }
                Err(ZhsError::TimeLimit) => {
                    emit.emit(CourseEvent::LimitReached);
                    return Ok(());
                }
                Err(e) => {
                    emit.emit(CourseEvent::Failed {
                        msg: format!("视频 {} 失败: {e}", video.name),
                    });
                }
            }
        }
        emit.emit(CourseEvent::done(
            "course_finished",
            format!("课程 {} 已完成", ctx.course_name),
        ));
        Ok(())
    }

    /// 刷单个视频（对应 `fuckZhidaoVideo`）。
    /// 学习会话失效（长时间挂机后 gologin 会话过期）时自动恢复并重试一次。
    pub async fn run_video(&mut self, rac_id: &str, video_id: &str, emit: &Emitter) -> Result<()> {
        match self.run_video_inner(rac_id, video_id, emit).await {
            Err(ZhsError::NotLoggedIn) => {
                emit.emit(CourseEvent::log("学习会话失效，正在自动恢复…"));
                self.ctx = None;
                self.context(rac_id, emit).await?;
                self.run_video_inner(rac_id, video_id, emit).await
            }
            other => other,
        }
    }

    async fn run_video_inner(&mut self, rac_id: &str, video_id: &str, emit: &Emitter) -> Result<()> {
        // 全局暂停：等恢复后再进入下一个视频
        self.pause.wait_resume().await;
        self.check_limit()?;
        let ctx = self.context(rac_id, emit).await?;
        let Some(video) = ctx.videos.iter().find(|v| v.video_id == video_id).cloned() else {
            return Err(ZhsError::msg(format!("视频 {video_id} 不存在")));
        };
        // 【防 -8】起始进度取"服务器报告值"与"本轮已成功上报的最高值"的较大者，
        // 会话自愈重载后 queryStudyInfo 若返回旧值也不会倒退。
        let mut played_time = self
            .reported_progress
            .get(&video.video_id)
            .copied()
            .unwrap_or(0.0)
            .max(video.study_total_time);
        if video.watch_state == 1.0 && self.end_thre <= 1.0 {
            emit.emit(CourseEvent::Skipped {
                name: video.name.clone(),
                reason: "watched".into(),
            });
            return Ok(());
        }

        // 获取学习令牌
        let token_id = self.prelearning_note(&ctx, &video).await?;
        let token_id = b64_encode(&token_id);

        // 弹题列表（按时间倒序）
        let mut questions: Vec<Value> = self
            .session
            .zhidao_query(
                VIDEO_POINTER_URL,
                &json!({
                    "lessonId": video.lesson_id,
                    "lessonVideoId": video.small_id,
                    "recruitId": ctx.recruit_id,
                    "courseId": ctx.course_id,
                }),
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?["data"]["questionPoint"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        questions.sort_by(|a, b| {
            let ta = a["timeSec"].as_f64().unwrap_or(0.0);
            let tb = b["timeSec"].as_f64().unwrap_or(0.0);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });
        while let Some(last) = questions.last() {
            if last["timeSec"].as_f64().unwrap_or(0.0) <= played_time {
                questions.pop();
            } else {
                break;
            }
        }

        let end_time = {
            let base = (video.video_sec * self.end_thre).max(1.0);
            match questions.first().and_then(|q| q["timeSec"].as_f64()) {
                Some(qt) => base.max(qt),
                None => base,
            }
        };

        // 伪装真实观看（异步，不阻塞）
        self.watch_video_fake(&video.video_id);
        // 三维课件上报
        let _ = self
            .session
            .zhidao_query(
                COURSEWARE_URL,
                &json!({"videoId": video.video_id}),
                VIDEO_KEY,
                Some(0),
                reqwest::Method::GET,
                STUDY_HEADERS,
            )
            .await;

        emit.emit(CourseEvent::VideoStart {
            video_id: video.video_id.clone(),
            name: video.name.clone(),
            played: played_time,
            total: video.video_sec,
        });

        // —— 主循环（对应 fuckZhidaoVideo 的事件循环）——
        let mut last_submit = played_time;
        let mut elapsed_time: u64 = 0;
        let db_interval: u64 = 30;
        let mut answer: Option<u32> = None;
        let mut question: Option<Value> = None;
        let mut report = false;
        let mut human_pause: i64 = 0; // 随机模拟人类暂停（区别于全局暂停）
        let mut wp = crypto::WatchPoint::new();

        while played_time < end_time {
            if sleep_or_stop(1.0, &self.stop).await {
                return Err(ZhsError::Stopped);
            }
            // 全局暂停（用户操作）：冻结进度，不上报、不计入时限
            if self.pause.is_paused() {
                emit.emit(CourseEvent::VideoProgress {
                    video_id: video.video_id.clone(),
                    played: played_time,
                    total: end_time,
                    paused: true,
                    quiz: answer.is_some(),
                });
                continue;
            }
            self.fucked_time += 1;
            self.check_limit()?;
            elapsed_time += 1;
            played_time = (played_time + self.speed()).min(end_time);
            // 随机暂停一分钟，模拟人类行为（对应 random() < 0.0025）
            if human_pause == 0 && rand::rng().random::<f64>() < 0.0025 {
                human_pause = 60;
            }
            if human_pause == 60 {
                report = true;
            }
            if human_pause > 0 {
                human_pause -= 1;
                played_time = last_submit;
            }
            if elapsed_time % 2 == 0 {
                wp.add(played_time as i64, None);
            }
            // 到达弹题时间点：拉题
            if let Some(last) = questions.last().cloned() {
                if played_time >= last["timeSec"].as_f64().unwrap_or(f64::MAX) {
                    questions.pop();
                    match self
                        .lesson_popup_exam(&ctx, &video, last["questionIds"].clone())
                        .await
                    {
                        Ok(q) => {
                            question = Some(q);
                            answer = Some(2); // 答题延迟
                            report = true;
                        }
                        Err(e) => {
                            emit.emit(CourseEvent::Failed {
                                msg: format!("获取弹题失败: {e}"),
                            });
                        }
                    }
                }
            }
            // 延迟后提交答案
            if let Some(a) = answer {
                if a == 0 {
                    answer = None;
                    if let Some(q) = &question {
                        let ans = answer_zhidao(q);
                        // 答题失败不终止刷课，记录后继续
                        if let Err(e) =
                            self.save_popup_exam_answer(&ctx, &video, q, &ans).await
                        {
                            emit.emit(CourseEvent::Failed {
                                msg: format!("提交弹题答案失败: {e}"),
                            });
                        } else {
                            emit.emit(CourseEvent::QuizAnswered {
                                video_id: video.video_id.clone(),
                                question: q["questionContent"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                            });
                        }
                    }
                } else {
                    if human_pause == 0 {
                        human_pause = 1;
                    }
                    answer = Some(a - 1);
                }
            }
            // 每 30 秒 / 弹题 / 暂停时上报数据库
            if elapsed_time % db_interval == 0
                || played_time >= end_time
                || report
            {
                report = false;
                wp.add(played_time as i64, None);
                let watch_point = wp.get();
                let (ok, captcha) = self
                    .submit_once_with_relogin(
                        rac_id, &ctx, &video, played_time, last_submit, &watch_point,
                        &token_id, true, emit,
                    )
                    .await;
                if captcha {
                    return Err(ZhsError::Captcha);
                }
                if ok {
                    last_submit = played_time;
                    wp.reset(played_time as i64);
                } else {
                    // 【风控】失败也必须 reset watch point，防止跨周期累积膨胀。
                    // 原版无论成功与否都 wp.reset，本周期的点已在上面 add 过，
                    // 下个周期从当前 played_time 重新采样即可。
                    wp.reset(played_time as i64);
                    emit.emit(CourseEvent::Failed {
                        msg: "进度上报失败（将在下个周期重试）".into(),
                    });
                }
            }
            emit.emit(CourseEvent::VideoProgress {
                video_id: video.video_id.clone(),
                played: played_time,
                total: end_time,
                paused: human_pause > 0,
                quiz: answer.is_some(),
            });
        }
        // 【风控】终点补偿：循环内最后一次上报若失败，last_submit 会落后于
        // played_time。此时绝不能发 VideoDone——否则服务器停留在旧进度，
        // 下一轮重刷同一段 = 进度反复上报 = -12。这里强制同步补报一次。
        if last_submit < played_time {
            wp.add(played_time as i64, None);
            let watch_point = wp.get();
            let (ok, captcha) = self
                .submit_once_with_relogin(
                    rac_id, &ctx, &video, played_time, last_submit, &watch_point,
                    &token_id, true, emit,
                )
                .await;
            if captcha {
                return Err(ZhsError::Captcha);
            }
            if !ok {
                emit.emit(CourseEvent::Failed {
                    msg: "终点进度上报失败，本视频将不计入完成（下轮重刷）".into(),
                });
                // 不返回 Stopped/TimeLimit，让外层把它当作普通失败记录，
                // 继续下一个视频；这个视频服务器没记录，下轮会重新刷。
                return Ok(());
            }
            last_submit = played_time;
        }
        let tail_sleep = rand::rng().random::<f64>() + 1.0;
        if sleep_or_stop(tail_sleep, &self.stop).await {
            return Err(ZhsError::Stopped);
        }
        emit.emit(CourseEvent::VideoDone {
            video_id: video.video_id.clone(),
            name: video.name.clone(),
        });
        Ok(())
    }

    /// 上报一次进度；若学习会话失效（NotLoggedIn）则重新 gologin 后**原地重试一次**。
    ///
    /// 【风控】不能用 continue 丢弃本次上报再等下个 30 秒周期——那样客户端进度
    /// 已推进而服务器无记录，下次 playTimes 会包含 60 秒、watch point 却只覆盖部分，
    /// 数据不自洽正是 -12 的诱因。原地重试保证每个周期的数据闭环。
    /// 返回 (是否成功, 是否遇到验证码)。
    async fn submit_once_with_relogin(
        &mut self,
        rac_id: &str,
        ctx: &ZhidaoContext,
        video: &ZVideo,
        played_time: f64,
        last_submit: f64,
        watch_point: &str,
        token_id: &str,
        allow_relogin: bool,
        emit: &Emitter,
    ) -> (bool, bool) {
        let mut relogin = allow_relogin;
        loop {
            match self
                .save_database_interval(ctx, video, played_time, last_submit, watch_point, token_id)
                .await
            {
                Ok(()) => {
                    // 【防 -8】记录成功上报的最高进度，后续（含会话自愈重载）绝不倒退
                    let entry = self.reported_progress.entry(video.video_id.clone()).or_insert(0.0);
                    if played_time > *entry {
                        *entry = played_time;
                    }
                    return (true, false);
                }
                Err(ZhsError::Captcha) => {
                    emit.emit(CourseEvent::CaptchaRequired);
                    return (false, true);
                }
                Err(ZhsError::NotLoggedIn) if relogin => {
                    relogin = false; // 每个上报点最多自愈一次，防止无限循环
                    emit.emit(CourseEvent::log("学习会话失效，正在自动恢复…"));
                    if self.gologin(rac_id).await.is_ok() {
                        continue; // 原地重试本次上报（不是 while 的 continue）
                    }
                    return (false, false);
                }
                Err(_) => return (false, false),
            }
        }
    }

    fn check_limit(&self) -> Result<()> {
        if self.limit_seconds > 0 && self.fucked_time >= self.limit_seconds {
            return Err(ZhsError::TimeLimit);
        }
        Ok(())
    }

    /// 获取学习令牌（对应 `prelearningNote`）。
    async fn prelearning_note(&self, ctx: &ZhidaoContext, video: &ZVideo) -> Result<String> {
        let data = json!({
            "ccCourseId": ctx.course_id,
            "chapterId": video.chapter_id,
            "isApply": 1,
            "lessonId": video.lesson_id,
            "lessonVideoId": video.small_id,
            "recruitId": ctx.recruit_id,
            "videoId": video.video_id,
        });
        let resp = self
            .session
            .zhidao_query(
                PRELEARNING_NOTE_URL,
                &data,
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?;
        let id = resp
            .pointer("/data/studiedLessonDto/id")
            .map(json_str)
            .ok_or_else(|| ZhsError::msg("prelearningNote 响应缺少 studiedLessonDto.id"))?;
        Ok(id)
    }

    /// 拉取弹题（对应 `lessonPopoupExam`）。
    async fn lesson_popup_exam(
        &self,
        _ctx: &ZhidaoContext,
        video: &ZVideo,
        question_ids: Value,
    ) -> Result<Value> {
        let data = json!({
            "lessonId": video.lesson_id,
            "lessonVideoId": video.small_id,
            "questionIds": question_ids,
        });
        let resp = self
            .session
            .zhidao_query(
                POPUP_EXAM_URL,
                &data,
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?;
        let q = resp
            .pointer("/data/lessonTestQuestionUseInterfaceDtos/0/testQuestion")
            .cloned()
            .ok_or_else(|| ZhsError::msg("弹题响应缺少 testQuestion"))?;
        Ok(q)
    }

    /// 提交弹题答案（对应 `saveLessonPopupExamSaveAnswer`）。
    async fn save_popup_exam_answer(
        &self,
        ctx: &ZhidaoContext,
        video: &ZVideo,
        question: &Value,
        answer_ids: &str,
    ) -> Result<()> {
        let data = json!({
            "courseId": ctx.course_id,
            "recruitId": ctx.recruit_id,
            "testQuestionId": question["questionId"],
            "isCurrent": "1",
            "lessonId": video.lesson_id,
            "lessonVideoId": video.small_id,
            "answer": answer_ids,
            "testType": 0,
        });
        self.session
            .zhidao_query(
                SAVE_EXAM_URL,
                &data,
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?;
        Ok(())
    }

    /// 上报学习进度（对应 `saveDatabaseIntervalTimeV2`）。
    async fn save_database_interval(
        &self,
        ctx: &ZhidaoContext,
        video: &ZVideo,
        played_time: f64,
        last_submit: f64,
        watch_point: &str,
        token_id: &str,
    ) -> Result<()> {
        let uuid = self
            .session
            .uuid()
            .unwrap_or_default();
        let raw_ev = vec![
            ctx.recruit_id.clone(),
            video.lesson_id.clone(),
            video.small_id.clone(),
            video.video_id.clone(),
            video.chapter_id.clone(),
            "0".to_string(),
            ((played_time - last_submit).max(0.0) as i64).to_string(),
            (played_time as i64).to_string(),
            crypto::hms(video.video_sec.min(played_time) as i64),
            format!("{uuid}zhs"),
        ];
        let data = json!({
            "ewssw": watch_point,
            "sdsew": ev_encode(&raw_ev),
            "zwsds": token_id,
            "courseId": ctx.study_course_id,
        });
        self.session
            .zhidao_query(
                SAVE_DB_URL,
                &data,
                VIDEO_KEY,
                Some(0),
                reqwest::Method::POST,
                STUDY_HEADERS,
            )
            .await?;
        Ok(())
    }

    /// 伪装视频观看请求（对应 `watchVideo`，异步执行不阻塞）。
    ///
    /// 【风控】后台任务必须绑定停止令牌：暂停/跳过/清空后若旧任务仍在后台
    /// 请求视频流，会形成"任务已停但服务器仍收到请求"的异常模式。
    /// 同时每个视频最多一个伪装任务，切视频时旧的立即中断。
    fn watch_video_fake(&self, video_id: &str) {
        let session = self.session.clone();
        let video_id = video_id.to_string();
        let stop = self.stop.clone();
        let pause = self.pause.clone();
        // 复用上一任伪装任务的"终止开关"：切换视频/停止时立刻掐断旧任务
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
            // 用 select! 让请求可被停止/暂停打断
            let text = tokio::select! {
                _ = stop.wait() => return,
                _ = task_stop.wait() => return,
                r = session.get_text(INIT_VIDEO_URL, &query) => match r { Ok(t) => t, Err(_) => return },
            };
            // 暂停时不要丢弃这个伪装观看任务——等恢复后继续请求视频流。
            // 若直接 return，整个视频期间只有进度上报、没有视频流请求，是异常模式。
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
            // 视频流可能很大：只读前 64KB 即可模拟"开始加载"，避免长时间占用连接
            tokio::select! {
                _ = stop.wait() => {}
                _ = task_stop.wait() => {}
                _ = session.get_text_prefix(&line_url, 64 * 1024) => {}
            }
        });
    }
}

/// 弹题自动作答（对应 `answerZhidao`）：选项数据里 result == "1" 的即为正确答案。
fn answer_zhidao(q: &Value) -> String {
    let ids: Vec<String> = q["questionOptions"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|opt| opt["result"].as_str() == Some("1"))
        .map(|opt| json_str(&opt["id"]))
        .collect();
    ids.join(",")
}

fn parse_headers(headers: &[(&str, &str)]) -> reqwest::header::HeaderMap {
    let mut map = reqwest::header::HeaderMap::new();
    for (k, v) in headers {
        if let (Ok(k), Ok(v)) = (
            reqwest::header::HeaderName::from_bytes(k.as_bytes()),
            reqwest::header::HeaderValue::from_str(v),
        ) {
            map.insert(k, v);
        }
    }
    map
}
