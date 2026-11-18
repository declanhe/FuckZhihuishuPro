//! 课程任务调度引擎：启动/停止刷课任务，转发进度事件到前端。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::json;
use tauri::{AppHandle, Emitter as _};

use super::error::{Result, ZhsError};
use super::events::{CourseEvent, Emitter, StopHandle, StopToken};
use super::hike::HikeRunner;
use super::session::ZhsSession;
use super::zhidao::ZhidaoRunner;

/// 单个刷课任务的运行配置。
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// 倍速（<= 0 = 平台最高）
    pub speed: f64,
    /// 完成阈值
    pub end_thre: f64,
    /// 单课时限（分钟，0 = 不限）
    pub limit_minutes: u64,
}

/// 引擎状态：管理所有运行中的任务。
#[derive(Default)]
pub struct Engine {
    running: Arc<Mutex<HashMap<String, Arc<StopHandle>>>>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self, course_id: &str) -> bool {
        self.running.lock().unwrap().contains_key(course_id)
    }

    pub fn running_ids(&self) -> Vec<String> {
        self.running.lock().unwrap().keys().cloned().collect()
    }

    /// 启动一门课程的刷课任务（自动识别共享课/校内课）。
    /// 课程 ID 含字母 -> 知到共享课（recruitAndCourseId）；纯数字 -> 校内课（courseId）。
    pub fn start(
        &self,
        app: AppHandle,
        session: Arc<ZhsSession>,
        course_id: String,
        course_name: String,
        cfg: TaskConfig,
    ) -> Result<()> {
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(&course_id) {
                return Err(ZhsError::msg("该课程已在刷课中"));
            }
            let stop = Arc::new(StopHandle::new());
            running.insert(course_id.clone(), stop);
        }

        let running_map = self.running.clone();
        let stop_handle = self
            .running
            .lock()
            .unwrap()
            .get(&course_id)
            .cloned()
            .expect("just inserted");
        let token: StopToken = stop_handle.token();
        let cid = course_id.clone();
        let cname = course_name.clone();

        tauri::async_runtime::spawn(async move {
            let emit_app = app.clone();
            let emit_id = cid.clone();
            let emit_name = cname.clone();
            let emitter = Emitter::new(Arc::new(move |ev: CourseEvent| {
                let _ = emit_app.emit(
                    "course-event",
                    json!({
                        "courseId": emit_id,
                        "courseName": emit_name,
                        "event": ev,
                    }),
                );
            }));

            emitter.emit(CourseEvent::log("任务开始"));
            let is_zhidao = cid.chars().any(|c| c.is_alphabetic());
            let result = if is_zhidao {
                let mut runner = ZhidaoRunner::new(
                    session.clone(),
                    cfg.speed,
                    cfg.end_thre,
                    cfg.limit_minutes,
                    token.clone(),
                );
                runner.run_course(&cid, &emitter).await
            } else {
                let mut runner = HikeRunner::new(
                    session.clone(),
                    cfg.speed,
                    cfg.end_thre,
                    cfg.limit_minutes,
                    token.clone(),
                );
                runner.run_course(&cid, &emitter).await
            };

            match result {
                Ok(()) => {
                    emitter.emit(CourseEvent::log("任务结束"));
                    emitter.emit(CourseEvent::Done {
                        msg: "任务结束".into(),
                    });
                }
                Err(ZhsError::Stopped) => {
                    emitter.emit(CourseEvent::log("任务已停止"));
                    emitter.emit(CourseEvent::Done {
                        msg: "任务已停止".into(),
                    });
                }
                Err(ZhsError::Captcha) => {
                    // 账号限制（code -12）：runner 已 emit CaptchaRequired（前端展示恢复引导）。
                    // 这里不重复发事件，也不走 Failed/异常文案——这是账号保护，不是软件故障。
                    emitter.emit(CourseEvent::Done {
                        msg: "任务已暂停：账号被智慧树临时限制（见上方提示）".into(),
                    });
                }
                Err(e) => {
                    emitter.emit(CourseEvent::Failed { msg: e.to_string() });
                    emitter.emit(CourseEvent::Done {
                        msg: format!("任务异常结束: {e}"),
                    });
                }
            }
            running_map.lock().unwrap().remove(&cid);
        });
        Ok(())
    }

    /// 停止指定课程任务。
    pub fn stop(&self, course_id: &str) -> bool {
        let running = self.running.lock().unwrap();
        if let Some(handle) = running.get(course_id) {
            handle.stop();
            true
        } else {
            false
        }
    }

    /// 停止全部任务。
    pub fn stop_all(&self) {
        for handle in self.running.lock().unwrap().values() {
            handle.stop();
        }
    }
}
