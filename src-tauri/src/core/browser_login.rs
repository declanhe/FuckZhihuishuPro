//! 内嵌浏览器登录（Autovisor 思路：浏览器只负责登录，刷课走 API）。
//!
//! v0.3.2 方案：**直接把主窗口导航到智慧树登录页**，不在新窗口打开。
//! 原因：独立子窗口的 WebView2 环境在这台机器上反复白屏（环境创建
//! 兼容性问题），而主窗口环境已被证实完全正常。
//!
//! 流程：前端点"浏览器登录" → 主窗口整体导航到登录页 → 用户完成账密/
//! 滑块/短信验证 → 检测到 URL 跳离登录页 → CookieManager 抓全部
//! cookie → 写入引擎会话 → 主窗口导航回应用（tauri://localhost）→
//! 前端监听事件切换到已登录界面。
//!
//! 前端如何回来：主窗口被导航走后 Vue 应用销毁，回来时重新加载
//! index.html，从"loading"状态开始，restore_login 恢复登录态即可。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use webview2_com::GetCookiesCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{HSTRING, Interface, PCWSTR, PWSTR};

use super::cookies::StoredCookie;
use super::error::{Result, ZhsError};

const LOGIN_URL: &str =
    "https://passport.zhihuishu.com/login?service=https://onlineservice-api.zhihuishu.com/login/gologin";

/// 判断 URL 是否为"已登录"状态（跳离 passport 登录页）。
fn url_is_logged_in(url: &str) -> bool {
    url.contains("zhihuishu.com")
        && !url.contains("passport.zhihuishu.com")
        && !url.contains("StuLogin")
        && !url.contains("service=")
}

/// 把主窗口导航到智慧树登录页，并启动成功检测循环。
pub fn start_browser_login(app: &tauri::AppHandle, _udf: PathBuf) -> Result<()> {
    let Some(win) = app.get_webview_window("main") else {
        return Err(ZhsError::msg("主窗口不存在"));
    };
    // 记录当前应用地址（dev 为 http://localhost:1420，生产为 tauri://localhost）
    let app_url = win
        .url()
        .ok()
        .map(|u| u.to_string())
        .unwrap_or_else(|| "http://localhost:1420/".to_string());
    win.navigate(LOGIN_URL.parse().unwrap())
        .map_err(|e| ZhsError::msg(format!("导航到登录页失败: {e}")))?;

    // 轮询 URL（纯 async，不碰 UI 线程）
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        // 最多轮询 10 分钟
        for _ in 0..1200 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let Some(w) = app2.get_webview_window("main") else {
                return;
            };
            let Ok(url) = w.url() else { continue };
            if url_is_logged_in(url.as_str()) {
                let app3 = app2.clone();
                let r = tokio::task::spawn_blocking(move || capture_cookies_blocking(&app3))
                    .await
                    .unwrap_or_else(|e| Err(ZhsError::msg(format!("内部错误: {e}"))));
                if let Err(e) = r {
                    eprintln!("[browser-login] capture failed: {e}");
                }
                // 无论抓取成败都回应用界面（成功事件已在前端触发跳转）
                if let Some(w) = app2.get_webview_window("main") {
                    let _ = w.navigate(app_url.parse().unwrap());
                }
                return;
            }
        }
    });
    Ok(())
}

/// 从主窗口的 CookieManager 抓全部域的 cookie，写入引擎会话。
fn capture_cookies_blocking(app: &tauri::AppHandle) -> Result<()> {
    let Some(win) = app.get_webview_window("main") else {
        return Ok(());
    };

    let cookies: Arc<Mutex<Vec<StoredCookie>>> = Arc::new(Mutex::new(Vec::new()));
    let cookies_clone = cookies.clone();
    let (tx, rx) = std::sync::mpsc::channel::<Result<()>>();
    let tx2 = tx.clone();

    // GetCookies(uri) 按 URI 过滤；关键 cookie 分散在多个子域，
    // host-only cookie 只在精确匹配时返回，所以逐域抓取。
    const COOKIE_URIS: &[&str] = &[
        "https://zhihuishu.com",
        "https://passport.zhihuishu.com",
        "https://hikeservice.zhihuishu.com",
        "https://studyresources.zhihuishu.com",
        "https://hike-teaching.zhihuishu.com",
        "https://studyservice-api.zhihuishu.com",
        "https://onlineservice-api.zhihuishu.com",
        "https://studyh5.zhihuishu.com",
        "https://hike.zhihuishu.com",
        "https://newbase.zhihuishu.com",
        "https://appcomm-user.zhihuishu.com",
    ];

    // 在 WebView2 UI 线程上逐域发起 GetCookies（异步、立即返回）
    let _ = win.with_webview(move |platform| {
        let result = (|| {
            let controller = &platform.controller();
            unsafe {
                let core = controller.CoreWebView2()?;
                let core2: ICoreWebView2_2 = core.cast()?;
                let manager = core2.CookieManager()?;
                let handler = GetCookiesCompletedHandler::create(Box::new(
                    move |error_code, cookie_list| {
                        let res = (|| {
                            error_code?;
                            let Some(list) = cookie_list else { return Ok(()) };
                            let mut count = 0u32;
                            list.Count(&mut count)?;
                            for i in 0..count {
                                let c = list.GetValueAtIndex(i)?;
                                let mut name_pw = PWSTR::null();
                                let mut value_pw = PWSTR::null();
                                let mut domain_pw = PWSTR::null();
                                let mut path_pw = PWSTR::null();
                                let mut secure = false.into();
                                let mut is_session = false.into();
                                c.Name(&mut name_pw)?;
                                c.Value(&mut value_pw)?;
                                c.Domain(&mut domain_pw)?;
                                c.Path(&mut path_pw)?;
                                c.IsSecure(&mut secure)?;
                                c.IsSession(&mut is_session)?;
                                let name = take_pw(name_pw);
                                let value = take_pw(value_pw);
                                let domain = take_pw(domain_pw);
                                let path = take_pw(path_pw);
                                cookies_clone.lock().unwrap().push(StoredCookie {
                                    name,
                                    value,
                                    domain: domain.trim_start_matches('.').to_lowercase(),
                                    host_only: false,
                                    path: if path.is_empty() { "/".into() } else { path },
                                    secure: secure.as_bool(),
                                    expires: if is_session.as_bool() {
                                        None
                                    } else {
                                        Some(4102444800)
                                    },
                                });
                            }
                            Ok(())
                        })();
                        let _ = tx.send(res);
                        Ok(())
                    },
                ));
                for uri in COOKIE_URIS {
                    manager.GetCookies(
                        PCWSTR::from_raw(HSTRING::from(*uri).as_ptr()),
                        &handler,
                    )?;
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            let _ = tx2.send(Err(e));
        }
    });

    // 已运行在阻塞线程，可安全等待；等所有域的回调完成（首次成功后
    // 再给其余回调一点时间），然后统一收结果
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut first_err: Option<ZhsError> = None;
    loop {
        let remain = deadline.saturating_duration_since(std::time::Instant::now());
        if remain.is_zero() {
            break;
        }
        match rx.recv_timeout(remain) {
            Ok(Ok(())) => {
                // 收到一次成功后短暂等待让其余回调落盘，然后尝试排空
                std::thread::sleep(std::time::Duration::from_millis(600));
                let mut last_err = None;
                while let Ok(res) = rx.try_recv() {
                    if let Err(e) = res {
                        last_err = Some(e);
                    }
                }
                if let Some(e) = last_err {
                    first_err = Some(e);
                }
                break;
            }
            Ok(Err(e)) => {
                first_err = Some(e);
                break;
            }
            Err(_) => break, // 超时：用已有的部分结果
        }
    }

    let list = cookies.lock().unwrap().clone();
    if list.is_empty() {
        return Err(first_err.unwrap_or_else(|| ZhsError::msg("未获取到登录 cookie，请重试")));
    }

    // 写入引擎会话并持久化
    if let Some(state) = app.try_state::<crate::commands::AppState>() {
        let value = serde_json::to_value(&list).unwrap_or(serde_json::Value::Array(vec![]));
        state.session.import_cookies(value)?;
        state.save_cookies();
    }

    // 通知前端登录成功
    use tauri::Emitter;
    let _ = app.emit("browser-login-success", ());
    Ok(())
}

/// 取走 WebView2 out-param 字符串并释放其内存（CoTaskMemAlloc 分配）。
fn take_pw(pw: PWSTR) -> String {
    unsafe {
        let s = pw.to_string().unwrap_or_default();
        if !pw.is_null() {
            windows::Win32::System::Com::CoTaskMemFree(Some(pw.as_ptr() as *const _));
        }
        s
    }
}
