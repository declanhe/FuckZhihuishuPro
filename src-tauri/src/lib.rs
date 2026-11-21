//! FuckZhihuishuPro 应用入口。
//!
//! 特性：
//! - 关闭窗口默认最小化到托盘，刷课任务在后台继续（对应需求"留在后台刷课"）
//! - 托盘菜单：显示主窗口 / 退出
//! - 数据目录 fallback：部分安全软件（如华硕 TuringShield、联想 PC Manager）
//!   会拦截未签名程序在 %APPDATA%/%LOCALAPPDATA% 创建目录，此时自动降级到
//!   exe 同目录存储，并把 WebView2 的用户数据目录一并迁移，保证"双击即用"。

mod commands;
mod config;
mod core;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{menu::{Menu, MenuItem}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, Manager, WebviewWindowBuilder};

use commands::AppState;
use config::AppConfig;
use core::engine::Engine;
use core::session::ZhsSession;

/// 测试某目录是否可以由当前进程创建并写入（安全软件可能只拦截特定进程）。
fn test_writable(dir: &PathBuf) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-test");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // ---- 数据目录决策（含安全软件 fallback）----
            // 候选按优先级排列，第一个可写的生效：
            // 1. %APPDATA%（标准位置，安装版首选）
            // 2. exe 旁（便携模式，如 D 盘绿色目录）
            // 3. %USERPROFILE%\FuckZhihuishuPro-Data（AppData 被安全软件拦截、
            //    exe 又在 Program Files 时的兜底）
            // 4. %TEMP%（绝对兜底）
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."));
            let roaming = app.path().app_data_dir()?;
            let local = app.path().app_local_data_dir()?;
            let temp = std::env::temp_dir();

            let mut data_candidates = vec![roaming.clone(), exe_dir.join("FuckZhihuishuPro-Data")];
            if let Ok(home) = std::env::var("USERPROFILE") {
                data_candidates.push(PathBuf::from(home).join("FuckZhihuishuPro-Data"));
            }
            data_candidates.push(temp.join("FuckZhihuishuPro-Data"));

            let data_dir = data_candidates
                .into_iter()
                .find(|d| test_writable(d))
                .unwrap_or_else(|| roaming.clone());

            // WebView2 用户数据目录：Local 可写用 Local，否则跟随 data_dir
            let webview_data_dir = if test_writable(&local) {
                local
            } else {
                data_dir.join("webview")
            };
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&webview_data_dir)?;
            eprintln!(
                "[FZHS] data_dir = {}\n[FZHS] webview_data_dir = {}",
                data_dir.display(),
                webview_data_dir.display()
            );

            let session = Arc::new(ZhsSession::new().expect("HTTP 客户端初始化失败"));
            // 启动时尝试恢复上次登录的 cookies（短时间免登录）
            let cookie_path = data_dir.join("cookies.json");
            if let Ok(raw) = std::fs::read_to_string(&cookie_path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    let _ = session.import_cookies(v);
                }
            }

            let config = AppConfig::load(&data_dir);
            app.manage(AppState {
                session,
                config: Mutex::new(config),
                engine: Engine::new(),
                data_dir,
                webview_data_dir: Some(webview_data_dir.clone()),
            });

            // ---- 主窗口（create:false，由这里创建以注入 webview data_directory）----
            let win_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .expect("missing main window config");
            WebviewWindowBuilder::from_config(app, &win_config)?
                .data_directory(webview_data_dir)
                .build()?;

            // ---- 系统托盘 ----
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出程序", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .expect("缺少应用图标");
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("FuckZhihuishuPro - 正在后台守护")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
                    "quit" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            state.engine.stop_all(app);
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // 「最小化到托盘」开启：关窗 -> 隐藏到托盘，后台继续刷课；
            // 关闭：点 × 直接退出（与托盘"退出程序"一致：停任务 + exit）
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "browser-login" {
                    return; // 登录窗口允许直接关闭
                }
                let app = window.app_handle();
                let minimize = app
                    .try_state::<AppState>()
                    .map(|s| s.config.lock().unwrap().minimize_to_tray)
                    .unwrap_or(true);
                if minimize {
                    let _ = window.hide();
                    api.prevent_close();
                } else {
                    if let Some(state) = app.try_state::<AppState>() {
                        state.engine.stop_all(app);
                    }
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_login_state,
            commands::login_with_password,
            commands::open_browser_login,
            commands::qr_login_start,
            commands::qr_login_poll,
            commands::logout,
            commands::restore_login,
            commands::get_courses,
            commands::start_courses,
            commands::stop_course,
            commands::stop_all_courses,
            commands::get_running,
            commands::get_queue_state,
            commands::set_queue_paused,
            commands::clear_queue,
            commands::get_config,
            commands::save_config,
            commands::self_test,
            commands::get_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
