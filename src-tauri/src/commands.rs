//! Tauri 命令层 —— 前端通过 `invoke()` 调用这些函数。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::config::AppConfig;
use crate::core::engine::{Engine, TaskConfig};
use crate::core::error::ZhsError;
use crate::core::login::{self, QrStatus};
use crate::core::session::ZhsSession;

/// 全局应用状态。
pub struct AppState {
    pub session: Arc<ZhsSession>,
    pub config: Mutex<AppConfig>,
    pub engine: Engine,
    pub data_dir: PathBuf,
    /// WebView2 用户数据目录（内嵌登录窗口与其共用，保证 cookie 互通）
    pub webview_data_dir: Option<PathBuf>,
}

impl AppState {
    fn cookies_path(&self) -> PathBuf {
        self.data_dir.join("cookies.json")
    }

    pub fn save_cookies(&self) {
        let path = self.cookies_path();
        let json = serde_json::to_string_pretty(&self.session.export_cookies()).unwrap_or_default();
        let _ = std::fs::write(path, json);
    }

    pub fn task_config(&self) -> TaskConfig {
        let cfg = self.config.lock().unwrap();
        TaskConfig {
            speed: cfg.speed,
            end_thre: cfg.end_threshold,
            limit_minutes: cfg.limit_minutes,
        }
    }
}

/// 登录状态。
#[derive(Serialize)]
pub struct LoginState {
    pub logged_in: bool,
    pub uuid: Option<String>,
}

/// 课程条目。
#[derive(Serialize, Deserialize)]
pub struct CourseItem {
    pub name: String,
    pub id: String,
}

/// 扫码状态。
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QrPoll {
    Waiting,
    Scanned { msg: String },
    Success,
    /// 二维码已过期，前端应自动刷新
    Expired,
}

fn err_to_string(e: ZhsError) -> String {
    serde_json::json!({ "kind": e.kind(), "message": e.to_string() }).to_string()
}

// ---------------- 登录 ----------------

#[tauri::command]
pub fn get_login_state(state: State<'_, AppState>) -> LoginState {
    LoginState {
        logged_in: state.session.is_logged_in(),
        uuid: state.session.uuid(),
    }
}

#[tauri::command]
pub async fn login_with_password(
    state: State<'_, AppState>,
    username: String,
    password: String,
) -> std::result::Result<(), String> {
    login::login_password(&state.session, &username, &password)
        .await
        .map_err(err_to_string)?;
    state.save_cookies();
    Ok(())
}

/// 主窗口直接导航到智慧树登录页（网页里完成账密/滑块/短信验证），
/// 成功后自动抓 cookie 并导航回应用；前端监听 browser-login-success 事件。
/// 依赖 WebView2 CookieManager，仅 Windows 实现；macOS 上前端直接隐藏该入口。
#[cfg(windows)]
#[tauri::command]
pub fn open_browser_login(app: tauri::AppHandle, state: State<'_, AppState>) -> std::result::Result<(), String> {
    let udf = state
        .webview_data_dir
        .clone()
        .ok_or_else(|| "内部错误：webview 数据目录未记录".to_string())?;
    crate::core::browser_login::start_browser_login(&app, udf).map_err(err_to_string)
}

/// macOS 桩：没有 WebView2。扫码登录完全覆盖需求，前端已隐藏该入口，兜底提示。
#[cfg(not(windows))]
#[tauri::command]
pub fn open_browser_login() -> std::result::Result<(), String> {
    Err("浏览器登录暂只支持 Windows，请用「扫码登录」～".to_string())
}

#[tauri::command]
pub async fn qr_login_start(state: State<'_, AppState>) -> std::result::Result<serde_json::Value, String> {
    let (token, img) = login::qr_start(&state.session).await.map_err(err_to_string)?;
    Ok(serde_json::json!({ "qrToken": token, "img": img }))
}

#[tauri::command]
pub async fn qr_login_poll(
    state: State<'_, AppState>,
    qr_token: String,
) -> std::result::Result<QrPoll, String> {
    match login::qr_poll(&state.session, &qr_token).await {
        Ok(QrStatus::Waiting) => Ok(QrPoll::Waiting),
        Ok(QrStatus::Scanned(msg)) => Ok(QrPoll::Scanned { msg }),
        Ok(QrStatus::Success) => {
            state.save_cookies();
            Ok(QrPoll::Success)
        }
        // 明确返回 expired，让前端自动刷新——此前返回 Waiting 导致前端
        // 的 qr_expired 分支永远不可达，过期后无限轮询失效二维码像"卡住"
        Err(ZhsError::QrExpired) => Ok(QrPoll::Expired),
        Err(e) => Err(err_to_string(e)),
    }
}

#[tauri::command]
pub fn logout(app: AppHandle, state: State<'_, AppState>) {
    state.session.logout();
    let _ = std::fs::remove_file(state.cookies_path());
    state.engine.stop_all(&app);
}

#[tauri::command]
pub async fn restore_login(state: State<'_, AppState>) -> std::result::Result<bool, String> {
    let path = state.cookies_path();
    if !path.exists() {
        return Ok(false);
    }
    let raw = std::fs::read_to_string(&path).unwrap_or_default();
    let Ok(cookies) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Ok(false);
    };
    match login::restore_session(&state.session, cookies).await {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

// ---------------- 课程 ----------------

#[tauri::command]
pub async fn get_courses(
    state: State<'_, AppState>,
) -> std::result::Result<serde_json::Value, String> {
    if !state.session.is_logged_in() {
        return Err(err_to_string(ZhsError::NotLoggedIn));
    }
    let session = state.session.clone();
    let (zhidao, hike) = tokio::join!(
        crate::core::zhidao::get_zhidao_list(&session),
        crate::core::hike::get_hike_list(&session),
    );
    let to_items = |list: std::result::Result<Vec<(String, String)>, ZhsError>| -> Vec<CourseItem> {
        match list {
            Ok(v) => v
                .into_iter()
                .map(|(name, id)| CourseItem { name, id })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    let zhidao_items = to_items(zhidao);
    let hike_items = to_items(hike);
    // 两边都失败说明登录已失效
    if zhidao_items.is_empty() && hike_items.is_empty() {
        // 不一定是失效，也可能真的没课；由前端提示
    }
    Ok(serde_json::json!({
        "zhidao": zhidao_items,
        "hike": hike_items,
    }))
}

/// 课程入队（串行刷课：按传入顺序一门一门刷）。返回入队失败的 ID（重复）。
#[tauri::command]
pub fn start_courses(
    app: AppHandle,
    state: State<'_, AppState>,
    courses: Vec<CourseItem>,
) -> std::result::Result<Vec<String>, String> {
    if !state.session.is_logged_in() {
        return Err(err_to_string(ZhsError::NotLoggedIn));
    }
    let cfg = state.task_config();
    let courses: Vec<(String, String)> = courses
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    Ok(state
        .engine
        .enqueue(app, state.session.clone(), courses, cfg))
}

/// 停止指定课程：正在刷的 -> 跳到下一门；排队中的 -> 移出队列。
#[tauri::command]
pub fn stop_course(app: AppHandle, state: State<'_, AppState>, course_id: String) -> bool {
    state.engine.stop(&app, &course_id)
}

/// 停止全部：清空队列 + 停止当前课程。
#[tauri::command]
pub fn stop_all_courses(app: AppHandle, state: State<'_, AppState>) {
    state.engine.stop_all(&app)
}

/// 正在刷的课程 ID。
#[tauri::command]
pub fn get_running(state: State<'_, AppState>) -> Vec<String> {
    state.engine.running_ids()
}

/// 队列状态：当前课程 + 排队课程 + 是否暂停。
#[tauri::command]
pub fn get_queue_state(state: State<'_, AppState>) -> crate::core::engine::QueueState {
    state.engine.queue_state()
}

/// 暂停/继续刷课（当前视频冻结进度，队列不开下一门）。
#[tauri::command]
pub fn set_queue_paused(app: AppHandle, state: State<'_, AppState>, paused: bool) {
    state.engine.set_paused(&app, paused)
}

/// 清空排队中的课程（不影响正在刷的）。
#[tauri::command]
pub fn clear_queue(app: AppHandle, state: State<'_, AppState>) {
    state.engine.clear_queue(&app)
}

// ---------------- 配置 ----------------

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, config: AppConfig) {
    *state.config.lock().unwrap() = config.clone();
    let _ = config.save(&state.data_dir);
}

// ---------------- 杂项 ----------------

/// 测试核心加密逻辑的自检（前端"关于"页可调用）。
#[tauri::command]
pub fn self_test() -> bool {
    // WatchPoint / ev / AES 往返
    let mut wp = crate::core::crypto::WatchPoint::new();
    wp.add(10, None);
    let ok_wp = !wp.get().is_empty();
    let ct = crate::core::crypto::aes_encrypt(crate::core::crypto::VIDEO_KEY, "test");
    let ok_aes = crate::core::crypto::aes_decrypt(crate::core::crypto::VIDEO_KEY, &ct)
        .map(|s| s == "test")
        .unwrap_or(false);
    ok_wp && ok_aes
}

/// 数据目录路径（前端展示"日志/数据保存位置"）。
#[tauri::command]
pub fn get_data_dir(state: State<'_, AppState>) -> String {
    state.data_dir.to_string_lossy().to_string()
}
