//! 统一错误类型。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZhsError {
    #[error("网络请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Message(String),

    #[error("账号被智慧树临时限制，请稍后再试")]
    Captcha,

    #[error("账号或密码错误")]
    BadCredentials,

    #[error("密码错误次数过多，需要验证码，请改用扫码登录")]
    NeedCaptcha,

    #[error("该账号需要短信验证，请先在浏览器上手动登录一次再试")]
    NeedSmsAuth,

    #[error("二维码已过期，请点击刷新重新获取")]
    QrExpired,

    #[error("二维码登录已取消")]
    QrCancelled,

    #[error("未登录或登录已失效，请重新登录")]
    NotLoggedIn,

    #[error("达到单课时限")]
    TimeLimit,

    #[error("任务已被用户停止")]
    Stopped,
}

// WebView2 是 Windows 专属，该转换只在 Windows 上编译
#[cfg(windows)]
impl From<windows::core::Error> for ZhsError {
    fn from(e: windows::core::Error) -> Self {
        Self::msg(format!("WebView2 操作失败: {e}"))
    }
}

impl ZhsError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }

    /// 返回给前端的简短分类标识
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Captcha => "captcha",
            Self::BadCredentials => "bad_credentials",
            Self::NeedCaptcha => "need_captcha",
            Self::NeedSmsAuth => "need_sms",
            Self::QrExpired => "qr_expired",
            Self::QrCancelled => "qr_cancelled",
            Self::NotLoggedIn => "not_logged_in",
            Self::TimeLimit => "time_limit",
            Self::Stopped => "stopped",
            _ => "error",
        }
    }
}

pub type Result<T> = std::result::Result<T, ZhsError>;
