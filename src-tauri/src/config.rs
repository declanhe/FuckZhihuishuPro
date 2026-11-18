//! 应用配置（倍速 / 完成阈值 / 时限），持久化到用户数据目录。

use std::path::Path;

use serde::{Deserialize, Serialize};

/// 应用配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 播放倍速：0 = 平台最高（共享课 1.5 / 校内课 1.25，与 fuckZHS 默认一致）。
    /// UI 上最高只提供 1.5x，更高倍速有风控风险。
    pub speed: f64,
    /// 完成阈值（对应 fuckZHS end_thre）：播放进度超过该比例视为完成。
    /// 默认 1.0（100%，刷满整个视频）；0.91 可更快但与官方完成线一致。
    pub end_threshold: f64,
    /// 单门课程学习时限（分钟，0 = 不限）
    pub limit_minutes: u64,
    /// 关闭窗口时最小化到托盘继续刷课（默认开启）
    pub minimize_to_tray: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            speed: 0.0,
            end_threshold: 1.0,
            limit_minutes: 0,
            minimize_to_tray: true,
        }
    }
}

impl AppConfig {
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("config.json");
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        let path = data_dir.join("config.json");
        let json = serde_json::to_string_pretty(self).unwrap_or_default();
        std::fs::write(path, json)
    }
}
