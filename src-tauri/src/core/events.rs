//! 刷课过程中的事件通知与停止/暂停控制。

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;

/// 刷课进度事件（发送给前端渲染）。
///
/// 带 `code` 的事件，前端按 code 做多语言渲染；`msg` 仅作兜底展示。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CourseEvent {
    /// 普通日志。code 非空时前端可翻译（arg 为课程名等参数）。
    Log {
        level: String,
        msg: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        arg: Option<String>,
    },
    /// 进入章节
    Chapter { name: String },
    /// 进入小节
    Lesson { name: String },
    /// 开始刷某个视频
    VideoStart {
        video_id: String,
        name: String,
        played: f64,
        total: f64,
    },
    /// 视频观看进度（每秒一次）
    VideoProgress {
        video_id: String,
        played: f64,
        total: f64,
        paused: bool,
        quiz: bool,
    },
    /// 视频完成
    VideoDone { video_id: String, name: String },
    /// 已自动回答弹题
    QuizAnswered { video_id: String, question: String },
    /// 跳过某项。reason 为机器可读代码：watched / quiz / resource
    Skipped { name: String, reason: String },
    /// 整门课程完成。code：course_finished / finished / stopped / captcha
    Done {
        msg: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    /// 达到单课时限
    LimitReached,
    /// 需要验证码
    CaptchaRequired,
    /// 某项失败（不中断整体流程）。
    /// level：`warn` = 可自愈/可重试的小问题（黄字，不影响刷课）；
    ///        `error` = 该项确实失败（红字）。
    Failed {
        msg: String,
        #[serde(default = "default_error_level")]
        level: String,
    },
}

#[allow(dead_code)] // 供 serde 的 default 属性引用
fn default_error_level() -> String {
    "error".into()
}

impl CourseEvent {
    pub fn log(msg: impl Into<String>) -> Self {
        Self::Log {
            level: "info".into(),
            msg: msg.into(),
            code: None,
            arg: None,
        }
    }

    /// 带翻译代码的日志（arg 通常为课程名）。
    pub fn logc(code: &str, msg: impl Into<String>, arg: impl Into<String>) -> Self {
        Self::Log {
            level: "info".into(),
            msg: msg.into(),
            code: Some(code.into()),
            arg: Some(arg.into()),
        }
    }

    /// 可自愈的小问题（进度上报失败会重试等）——前端显示为黄色警告。
    pub fn warn(msg: impl Into<String>) -> Self {
        Self::Failed {
            msg: msg.into(),
            level: "warn".into(),
        }
    }

    /// 该项确实失败——前端显示为红色错误。
    pub fn fail(msg: impl Into<String>) -> Self {
        Self::Failed {
            msg: msg.into(),
            level: "error".into(),
        }
    }

    pub fn done(code: &str, msg: impl Into<String>) -> Self {
        Self::Done {
            msg: msg.into(),
            code: Some(code.into()),
        }
    }
}

/// 事件发送器。
#[derive(Clone)]
pub struct Emitter {
    f: EmitFn,
}

pub type EmitFn = Arc<dyn Fn(CourseEvent) + Send + Sync>;

impl Emitter {
    pub fn new(f: EmitFn) -> Self {
        Self { f }
    }

    pub fn emit(&self, ev: CourseEvent) {
        (self.f)(ev)
    }
}

/// 停止令牌（任务侧持有）。
#[derive(Clone)]
pub struct StopToken {
    rx: Arc<watch::Receiver<bool>>,
}

impl StopToken {
    pub fn is_stopped(&self) -> bool {
        *self.rx.borrow()
    }

    /// 等待停止信号。
    pub async fn wait(&self) {
        let mut rx = (*self.rx).clone();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return; // 发送端已丢弃，视为停止
            }
        }
    }
}

/// 标准停止控制句柄：flag 侧调用 `stop()`，任务侧持有 `token()`。
#[derive(Clone)]
pub struct StopHandle {
    tx: Arc<watch::Sender<bool>>,
}

impl StopHandle {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        Self { tx: Arc::new(tx) }
    }

    pub fn token(&self) -> StopToken {
        StopToken {
            rx: Arc::new(self.tx.subscribe()),
        }
    }

    pub fn stop(&self) {
        let _ = self.tx.send(true);
    }
}

impl Default for StopHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局暂停开关：引擎侧调用 `set()`，任务侧持有 `token()`。
/// 暂停时视频循环不推进进度（不上报、不计入时限），调度器不开下一门课。
#[derive(Clone)]
pub struct PauseFlag {
    tx: Arc<watch::Sender<bool>>,
}

impl PauseFlag {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        Self { tx: Arc::new(tx) }
    }

    pub fn set(&self, paused: bool) {
        let _ = self.tx.send(paused);
    }

    pub fn is_paused(&self) -> bool {
        *self.tx.borrow()
    }

    pub fn token(&self) -> PauseToken {
        PauseToken {
            rx: Arc::new(self.tx.subscribe()),
        }
    }
}

impl Default for PauseFlag {
    fn default() -> Self {
        Self::new()
    }
}

/// 暂停令牌（任务/调度器侧持有）。
#[derive(Clone)]
pub struct PauseToken {
    rx: Arc<watch::Receiver<bool>>,
}

impl PauseToken {
    pub fn is_paused(&self) -> bool {
        *self.rx.borrow()
    }

    /// 阻塞直到恢复（未暂停时立即返回）。
    pub async fn wait_resume(&self) {
        let mut rx = (*self.rx).clone();
        while *rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                return; // 发送端已丢弃，视为恢复
            }
        }
    }
}

/// 可被停止打断的 sleep，返回 `true` 表示收到了停止信号。
pub async fn sleep_or_stop(secs: f64, stop: &StopToken) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs_f64(secs)) => false,
        _ = stop.wait() => true,
    }
}
