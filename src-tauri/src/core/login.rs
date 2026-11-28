//! 登录模块 —— 对应 fuckZHS `fucker.py::login` 与 `_qrlogin`。
//!
//! 提供两种方式：
//! - 密码登录（部分账号可能要求短信验证，需要先用浏览器登录过一次）
//! - 二维码登录（知到 APP 扫码，推荐）

use serde_json::{json, Value};

use super::error::{Result, ZhsError};
use super::session::{b64_encode_uri, now_ms, ZhsSession};

const LOGIN_PAGE: &str =
    "https://passport.zhihuishu.com/login?service=https://onlineservice-api.zhihuishu.com/login/gologin";
const VALID_URL: &str = "https://passport.zhihuishu.com/user/validateAccountAndPassword";
const CHECK_URL: &str = "https://appcomm-user.zhihuishu.com/app-commserv-user/userInfo/checkNeedAuth";
const QR_IMG_URL: &str = "https://passport.zhihuishu.com/qrCodeLogin/getLoginQrImg";
const QR_QUERY_URL: &str = "https://passport.zhihuishu.com/qrCodeLogin/getLoginQrInfo";

const LOGIN_HEADERS: &[(&str, &str)] = &[
    ("Origin", "https://passport.zhihuishu.com"),
    // 对齐原版 fucker.py:161-164：完整 login_page 作为 Referer
    ("Referer", "https://passport.zhihuishu.com/login?service=https://onlineservice-api.zhihuishu.com/login/gologin"),
];

/// 二维码轮询状态。
#[derive(Debug)]
pub enum QrStatus {
    /// 尚未扫码
    Waiting,
    /// 已扫码，等待手机确认
    Scanned(String),
    /// 登录成功
    Success,
}

/// 网页版登录协议（2024+ 版本，对照 passport 页面 sign_up.js）：
/// 1. POST /user/validateAccountAndPassword，表单只有 secretStr 字段：
///    secretStr = btoa(encodeURI(JSON.stringify({account, password, validate})))
///    （aes_util.js 的 Encrypt 已弃用 AES，退化为 encodeURI + Base64）
/// 2. 成功返回 {status:1, uuid, pwd}，再用 pwd 跳转登录页种 cookie
/// 3. 状态码：-1 参数异常 -2 程序异常 -3 用户不存在 -4 密码错误 -11 连错5次锁定
/// 账号密码登录。
pub async fn login_password(session: &ZhsSession, username: &str, password: &str) -> Result<()> {
    // 访问登录页，建立会话
    session
        .api_query(LOGIN_PAGE, None, reqwest::Method::GET, LOGIN_HEADERS)
        .await
        .ok(); // 登录页可能不是 JSON，忽略解析错误

    // secretStr = btoa(encodeURI(json))，对应网页版 Encrypt()
    let payload = json!({
        "account": username,
        "password": password,
        "validate": 0,
    });
    let secret_str = b64_encode_uri(&payload.to_string());
    let form = vec![("secretStr".to_string(), secret_str)];
    let user_info = session
        .post_form(VALID_URL, &form, LOGIN_HEADERS)
        .await?;

    // 状态码语义来自网页版 JS（-3 用户不存在、-11 连错锁定等）
    let status = user_info
        .get("status")
        .and_then(|s| s.as_i64())
        .unwrap_or(-999);
    match status {
        1 => {}
        -1 | -2 => return Err(ZhsError::BadCredentials),
        -3 => return Err(ZhsError::msg("该账号未注册智慧树")),
        // -4 现在通常意味着需要滑块验证（网页版强制滑块），纯接口无法通过
        -4 => {
            return Err(ZhsError::msg(
                "该账号需要滑块验证，本程序暂不支持；请改用扫码登录（推荐）",
            ))
        }
        -9 => return Err(ZhsError::NeedSmsAuth),
        -11 => {
            return Err(ZhsError::msg(
                "密码错误次数过多，账号已被锁定 5 分钟，请稍后再试或改用扫码登录",
            ))
        }
        other => {
            let m = user_info
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let suffix = if m.is_empty() {
                String::new()
            } else {
                format!("：{m}")
            };
            return Err(ZhsError::msg(format!(
                "登录失败（服务器状态 {other}{suffix}），请改用扫码登录"
            )));
        }
    }

    // uuid / pwd 可能是字符串或数字（不同接口版本行为不一），都要兼容
    let uuid = match user_info.get("uuid") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => return Err(ZhsError::msg("登录响应缺少 uuid，请改用扫码登录")),
    };

    let need_auth = session
        .api_query(
            CHECK_URL,
            Some(&json!({ "uuid": uuid })),
            reqwest::Method::POST,
            LOGIN_HEADERS,
        )
        .await?
        .pointer("/rt/needAuth")
        .and_then(|n| n.as_bool())
        .unwrap_or(false);
    if need_auth {
        return Err(ZhsError::NeedSmsAuth);
    }

    // 携带 pwd 完成登录跳转，获取 cookies
    let pwd = match user_info.get("pwd") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let query = vec![
        ("account".to_string(), username.to_string()),
        ("pwd".to_string(), pwd),
        ("validate".to_string(), "0".to_string()),
    ];
    session
        .get_text(LOGIN_PAGE, &query, LOGIN_HEADERS)
        .await
        .map_err(|e| ZhsError::msg(format!("登录跳转失败: {e}")))?;

    session.finalize_login()?;
    Ok(())
}

/// 获取登录二维码（对应 `_qrlogin` 的第一步）。
/// 返回 (qrToken, base64 图片数据)。
pub async fn qr_start(session: &ZhsSession) -> Result<(String, String)> {
    // 原版 _qrlogin 只 _sessionReady()（基础头，无 Origin/Referer）
    let r = session
        .api_query(QR_IMG_URL, None, reqwest::Method::GET, &[])
        .await?;
    let token = r
        .get("qrToken")
        .and_then(|t| t.as_str())
        .ok_or_else(|| ZhsError::msg("二维码获取失败：响应缺少 qrToken"))?
        .to_string();
    let img = r
        .get("img")
        .and_then(|i| i.as_str())
        .ok_or_else(|| ZhsError::msg("二维码获取失败：响应缺少图片"))?
        .to_string();
    Ok((token, img))
}

/// 轮询二维码状态（对应 `_qrlogin` 的循环体）。
/// 登录成功时内部会自动完成跨站登录并设置 cookies。
pub async fn qr_poll(session: &ZhsSession, qr_token: &str) -> Result<QrStatus> {
    let msg = session
        .api_query(
            QR_QUERY_URL,
            Some(&json!({ "qrToken": qr_token })),
            reqwest::Method::GET,
            // 对齐原版：无 Origin/Referer
            &[],
        )
        .await?;
    match msg.get("status").and_then(|s| s.as_i64()) {
        Some(-1) | None => Ok(QrStatus::Waiting),
        Some(0) => Ok(QrStatus::Scanned(
            msg.get("msg").and_then(|m| m.as_str()).unwrap_or("").into(),
        )),
        Some(1) => {
            // 关键：一次性密码在 oncePassword 字段（msg 只是提示文案）
            // 对照 fuckZHS fucker.py: params={"pwd": msg.oncePassword}
            let once_pwd = msg
                .get("oncePassword")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            if once_pwd.is_empty() {
                return Err(ZhsError::msg(
                    "服务器未返回一次性密码，请刷新二维码重试",
                ));
            }
            let query = vec![("pwd".to_string(), once_pwd)];
            session
                .get_text(LOGIN_PAGE, &query, &[])
                .await
                .map_err(|e| ZhsError::msg(format!("二维码登录跳转失败: {e}")))?;
            session.finalize_login()?;
            Ok(QrStatus::Success)
        }
        Some(2) => Err(ZhsError::QrExpired),
        Some(3) => Err(ZhsError::QrCancelled),
        Some(other) => Err(ZhsError::msg(format!("二维码登录未知状态: {other}"))),
    }
}

/// 恢复已保存的登录态（cookies.json）。
/// 恢复后用真实接口验证 cookies 是否仍然有效——避免"看起来已登录，
/// 开刷才发现全部失败"。网络故障等临时问题不误伤（保持登录态）。
pub async fn restore_session(session: &ZhsSession, cookies: Value) -> Result<()> {
    session.import_cookies(cookies)?;
    if session.uuid().is_none() {
        return Err(ZhsError::NotLoggedIn);
    }
    match check_alive(session).await {
        Ok(()) => Ok(()),
        // 服务器明确返回未登录：清掉登录态，让用户重新扫码
        Err(ZhsError::NotLoggedIn) => {
            session.logout();
            Err(ZhsError::NotLoggedIn)
        }
        // 网络抖动/超时等：无法判定，保持登录态，开刷失败再提示
        Err(_) => Ok(()),
    }
}

/// 用当前登录态验证 cookies 是否仍然有效：请求课程列表接口。
pub async fn check_alive(session: &ZhsSession) -> Result<()> {
    super::zhidao::get_zhidao_list(session).await.map(|_| ())
}

/// 时间戳工具（登录轮询用不到，供外部测试）。
pub fn _ts() -> i64 {
    now_ms()
}
