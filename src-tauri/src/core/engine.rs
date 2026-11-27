//! 课程任务调度引擎：课程串行队列（一次只刷一门，防风控），转发进度事件到前端。
//!
//! 智慧树平台一次只允许刷一门课：`start_courses` 只是入队，监督任务逐门
//! 执行——当前课程刷完（或被跳过）后才自动开始队列里的下一门。
//!
//! 健壮性：监督任务跑在内层 task，外层兜底——内层 panic 时清理残留状态、
//! 复位 alive 标志并给那门课发终态事件，绝不让整个队列卡死。

use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter as _, Manager as _};

use super::error::{Result, ZhsError};
use super::events::{CourseEvent, Emitter, PauseFlag, PauseToken, StopHandle, StopToken};
use super::hike::HikeRunner;
use super::session::ZhsSession;
use super::zhidao::ZhidaoRunner;
use crate::commands::AppState;

/// 队列中的一门课程。
#[derive(Debug, Clone, Serialize)]
pub struct QueuedCourse {
    pub id: String,
    pub name: String,
}

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

/// 队列快照（发给前端 / 供命令查询）。
#[derive(Debug, Clone, Serialize)]
pub struct QueueState {
    pub current: Option<QueuedCourse>,
    pub pending: Vec<QueuedCourse>,
    pub paused: bool,
    /// 一次性通知码（如 captcha_cleared），通常为 null
    pub notice: Option<String>,
}

/// 引擎共享状态的引用克隆（监督任务持有）。
#[derive(Clone)]
struct QueueRefs {
    current: Arc<Mutex<Option<(QueuedCourse, Arc<StopHandle>)>>>,
    queue: Arc<Mutex<Vec<QueuedCourse>>>,
    paused: PauseFlag,
}

impl QueueRefs {
    fn snapshot(&self, notice: Option<String>) -> QueueState {
        let current = lock_ok(&self.current).as_ref().map(|(c, _)| c.clone());
        let pending = lock_ok(&self.queue).clone();
        QueueState {
            current,
            pending,
            paused: self.paused.is_paused(),
            notice,
        }
    }

    fn emit(&self, app: &AppHandle, notice: Option<String>) {
        let _ = app.emit("queue-state", self.snapshot(notice));
    }
}

/// 引擎状态：串行课程队列。
pub struct Engine {
    refs: QueueRefs,
    supervisor_alive: Arc<Mutex<bool>>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// 毒化容忍锁：某任务 panic 持锁后被毒化的 Mutex 也能拿回数据，
/// 避免连锁 unwrap panic 卡死整个调度。
fn lock_ok<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// 以 course-event 的统一形状发给前端。
fn emit_course_event(app: &AppHandle, course_id: &str, course_name: &str, ev: CourseEvent) {
    let _ = app.emit(
        "course-event",
        json!({
            "courseId": course_id,
            "courseName": course_name,
            "event": ev,
        }),
    );
}

impl Engine {
    pub fn new() -> Self {
        Self {
            refs: QueueRefs {
                current: Arc::new(Mutex::new(None)),
                queue: Arc::new(Mutex::new(Vec::new())),
                paused: PauseFlag::new(),
            },
            supervisor_alive: Arc::new(Mutex::new(false)),
        }
    }

    pub fn queue_state(&self) -> QueueState {
        self.refs.snapshot(None)
    }

    /// 课程是否正在刷或已在队列中。
    pub fn is_running(&self, course_id: &str) -> bool {
        if lock_ok(&self.refs.current)
            .as_ref()
            .is_some_and(|(c, _)| c.id == course_id)
        {
            return true;
        }
        lock_ok(&self.refs.queue).iter().any(|c| c.id == course_id)
    }

    /// 正在刷的课程 ID（不含排队中的）。
    pub fn running_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if let Some((c, _)) = lock_ok(&self.refs.current).as_ref() {
            ids.push(c.id.clone());
        }
        ids
    }

    /// 课程入队（按传入顺序串行执行）。返回入队失败的课程 ID（重复）。
    pub fn enqueue(
        &self,
        app: AppHandle,
        session: Arc<ZhsSession>,
        courses: Vec<(String, String)>,
        cfg: TaskConfig,
    ) -> Vec<String> {
        let mut failed = Vec::new();
        {
            let mut queue = lock_ok(&self.refs.queue);
            let cur_id = lock_ok(&self.refs.current).as_ref().map(|(c, _)| c.id.clone());
            for (id, name) in courses {
                if cur_id.as_deref() == Some(id.as_str()) || queue.iter().any(|c| c.id == id) {
                    failed.push(id);
                    continue;
                }
                queue.push(QueuedCourse { id, name });
            }
        }
        self.refs.emit(&app, None);
        self.spawn_supervisor(app, session, cfg);
        failed
    }

    /// 停止指定课程：正在刷 -> 停止并跳到下一门；排队中 -> 移出队列。
    pub fn stop(&self, app: &AppHandle, course_id: &str) -> bool {
        {
            let cur = lock_ok(&self.refs.current);
            if let Some((c, handle)) = cur.as_ref() {
                if c.id == course_id {
                    handle.stop();
                    return true;
                }
            }
        }
        let removed = {
            let mut queue = lock_ok(&self.refs.queue);
            let before = queue.len();
            queue.retain(|c| c.id != course_id);
            queue.len() != before
        };
        if removed {
            self.refs.emit(app, None);
        }
        removed
    }

    /// 停止全部：清空队列 + 停止当前课程，并解除暂停。
    pub fn stop_all(&self, app: &AppHandle) {
        lock_ok(&self.refs.queue).clear();
        self.refs.paused.set(false);
        if let Some((_, handle)) = lock_ok(&self.refs.current).as_ref() {
            handle.stop();
        }
        self.refs.emit(app, None);
    }

    /// 暂停/继续：暂停时当前视频冻结进度，队列不开下一门。
    pub fn set_paused(&self, app: &AppHandle, paused: bool) {
        self.refs.paused.set(paused);
        self.refs.emit(app, None);
    }

    /// 清空排队中的课程（不影响正在刷的）。
    pub fn clear_queue(&self, app: &AppHandle) {
        lock_ok(&self.refs.queue).clear();
        self.refs.emit(app, None);
    }

    /// 启动监督任务：逐门取出队列课程执行；没有课程或遇到账号限制时结束。
    fn spawn_supervisor(&self, app: AppHandle, session: Arc<ZhsSession>, cfg: TaskConfig) {
        spawn_supervisor_inner(
            app,
            session,
            cfg,
            self.refs.clone(),
            self.supervisor_alive.clone(),
        );
    }
}

/// 真正启动监督任务（自由函数，便于 cleanup 里重启）。
///
/// 【竞态修复】alive 标志、队列状态、current 的复位/检查必须在同一个临界区内完成：
/// 旧 supervisor 结束时若队列里恰好有新入队的课，必须自己拉起新 supervisor，
/// 否则新 enqueue 看到 alive=true 直接返回、旧 supervisor 又已 break → 任务永久滞留。
fn spawn_supervisor_inner(
    app: AppHandle,
    session: Arc<ZhsSession>,
    cfg: TaskConfig,
    refs: QueueRefs,
    alive: Arc<Mutex<bool>>,
) {
    {
        let mut a = lock_ok(&alive);
        if *a {
            return;
        }
        *a = true;
    }
    tauri::async_runtime::spawn(async move {
        // 内层跑调度循环；外层兜底：内层 panic（内部 bug）也不许卡死队列——
        // 清掉残留的 current、复位 alive 标志，并给那门课发终态事件。
        let inner = {
            let refs = refs.clone();
            let app = app.clone();
            let session = session.clone();
            let cfg = cfg.clone();
            tauri::async_runtime::spawn(async move {
                supervisor_loop(app, refs, session, cfg).await;
            })
        };
        let _ = inner.await; // Err = panic，照样走下面的清理
        let stale = lock_ok(&refs.current).take();
        // 【原子交接】复位 alive 与检查队列在同一个 alive 锁的临界区内：
        // 若队列非空，直接重新拉起监督（此时 alive 已从 true 置 true，不会重复）。
        let need_restart = {
            let mut a = lock_ok(&alive);
            *a = false;
            !lock_ok(&refs.queue).is_empty()
        };
        if let Some((course, _)) = stale {
            emit_course_event(
                &app,
                &course.id,
                &course.name,
                CourseEvent::done("error", "任务异常终止（内部错误），调度已恢复"),
            );
        }
        refs.emit(&app, None);
        if need_restart {
            // 队列里还有课（正常 supervisor_loop 退出只在队列空或 captcha 时，
            // captcha 会清空队列，所以这里是竞态/异常路径），重新拉起
            spawn_supervisor_inner(app, session, cfg, refs, alive);
        }
    });
}

/// 调度主循环：取课程 -> 注册为当前 -> 跑完 -> 出清，循环到队列空或遇到账号限制。
async fn supervisor_loop(app: AppHandle, refs: QueueRefs, session: Arc<ZhsSession>, cfg: TaskConfig) {
    loop {
        // 暂停中不开新课程（等恢复）
        refs.paused.token().wait_resume().await;
        let next = lock_ok(&refs.queue).pop();
        let Some(course) = next else {
            break;
        };
        let stop_handle = Arc::new(StopHandle::new());
        {
            let mut cur = lock_ok(&refs.current);
            if cur.is_some() {
                // 不应发生：上一个任务尚未清理。放回队列，退出监督。
                lock_ok(&refs.queue).insert(0, course);
                break;
            }
            *cur = Some((course.clone(), stop_handle.clone()));
        }
        refs.emit(&app, None);

        // 每门课开始前重读配置：改倍速等设置对下一门课即时生效
        let cfg_now = app
            .try_state::<AppState>()
            .map(|s| s.task_config())
            .unwrap_or_else(|| cfg.clone());

        let result = run_one_course(
            app.clone(),
            session.clone(),
            course.clone(),
            cfg_now,
            stop_handle.token(),
            refs.paused.token(),
        )
        .await;

        *lock_ok(&refs.current) = None;
        match result {
            // 账号受限（code -12）：清空排队课程并停止调度，等用户人工处理。
            // 必须复位 paused——否则用户处理完验证码后若之前处于暂停态，
            // 重新入队的课程会卡在 wait_resume 永不开始。
            Err(ZhsError::Captcha) => {
                lock_ok(&refs.queue).clear();
                refs.paused.set(false);
                refs.emit(&app, Some("captcha_cleared".to_string()));
                break;
            }
            _ => {
                refs.emit(&app, None);
            }
        }
    }
}

/// 执行一门课程（阻塞到课程结束），返回课程最终结果供监督任务决策。
async fn run_one_course(
    app: AppHandle,
    session: Arc<ZhsSession>,
    course: QueuedCourse,
    cfg: TaskConfig,
    stop: StopToken,
    pause: PauseToken,
) -> Result<()> {
    let emit_app = app.clone();
    let emit_id = course.id.clone();
    let emit_name = course.name.clone();
    let emitter = Emitter::new(Arc::new(move |ev: CourseEvent| {
        emit_course_event(&emit_app, &emit_id, &emit_name, ev);
    }));

    emitter.emit(CourseEvent::logc("task_start", "任务开始", ""));
    // 课程 ID 含字母 -> 知到共享课（recruitAndCourseId）；纯数字 -> 校内课（courseId）
    let is_zhidao = course.id.chars().any(|c| c.is_alphabetic());
    let result = if is_zhidao {
        let mut runner = ZhidaoRunner::new(
            session.clone(),
            cfg.speed,
            cfg.end_thre,
            cfg.limit_minutes,
            stop.clone(),
            pause.clone(),
        );
        runner.run_course(&course.id, &emitter).await
    } else {
        let mut runner = HikeRunner::new(
            session.clone(),
            cfg.speed,
            cfg.end_thre,
            cfg.limit_minutes,
            stop.clone(),
            pause.clone(),
        );
        runner.run_course(&course.id, &emitter).await
    };

    match &result {
        Ok(()) => {
            emitter.emit(CourseEvent::done("finished", "任务结束"));
        }
        Err(ZhsError::Stopped) => {
            emitter.emit(CourseEvent::done("stopped", "任务已停止"));
        }
        Err(ZhsError::Captcha) => {
            // 账号限制（code -12）：runner 已 emit CaptchaRequired（前端展示恢复引导）。
            // 这里不重复发事件，也不走 Failed/异常文案——这是账号保护，不是软件故障。
            emitter.emit(CourseEvent::done(
                "captcha",
                "任务已暂停：账号被智慧树临时限制（见上方提示）",
            ));
        }
        Err(e) => {
            emitter.emit(CourseEvent::Failed { msg: e.to_string() });
            emitter.emit(CourseEvent::done("error", format!("任务异常结束: {e}")));
        }
    }
    result
}
