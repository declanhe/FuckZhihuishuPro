//! FuckZhihuishuPro 核心引擎 —— fuckZHS 的 Rust 移植。
//!
//! 模块对应关系：
//! - [`crypto`]:  zd_utils.py + sign.py（AES-CBC / MD5 / ev 编码 / WatchPoint）
//! - [`cookies`]: 自实现可导出的 Cookie 存储（requests.Session 的替代）
//! - [`session`]: HTTP 会话与三类 API 查询封装（apiQuery / zhidaoQuery / hikeQuery）
//! - [`login`]:   密码登录 + 扫码登录（login / _qrlogin）
//! - [`zhidao`]:  知到共享学分课（getZhidaoList / getZhidaoContext / fuckZhidaoVideo 等）
//! - [`hike`]:    校内学分课（getHikeList / fuckHikeCourse / fuckHikeVideo 等）
//! - [`engine`]:  任务调度（启动/停止，事件转发）

pub mod cookies;
pub mod crypto;
pub mod engine;
pub mod error;
pub mod events;
pub mod hike;
pub mod login;
pub mod session;
pub mod zhidao;

// 内嵌浏览器登录依赖 WebView2 CookieManager，仅 Windows 可用；
// macOS 上二维码登录完全够用（见 commands.rs 的 open_browser_login 桩）。
#[cfg(windows)]
pub mod browser_login;
