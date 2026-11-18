//! 刷课过程中的事件通知与停止控制。

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::watch;

/// 刷课进度事件（发送给前端渲染）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CourseEvent {
    /// 普通日志
    Log { level: String, msg: String },
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
    /// 跳过某项（已看完/不支持类型等）
    Skipped { name: String, reason: String },
    /// 整门课程完成
    Done { msg: String },
    /// 达到单课时限
    LimitReached,
    /// 需要验证码
    CaptchaRequired,
    /// 某项失败（不中断整体流程）
    Failed { msg: String },
}

impl CourseEvent {
    pub fn log(msg: impl Into<String>) -> Self {
        Self::Log {
            level: "info".into(),
            msg: msg.into(),
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

/// 可被停止打断的 sleep，返回 `true` 表示收到了停止信号。
pub async fn sleep_or_stop(secs: f64, stop: &StopToken) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs_f64(secs)) => false,
        _ = stop.wait() => true,
    }
}
