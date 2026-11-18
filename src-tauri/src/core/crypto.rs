//! 加密与混淆工具 —— 移植自 fuckZHS 的 `zd_utils.py` 与 `sign.py`。
//!
//! - [`aes_encrypt`] / [`aes_decrypt`]: 知到 API 的 AES-128-CBC secretStr
//! - [`get_ev`]: 学习进度上报的 ev 混淆编码
//! - [`sign_study_record`]: hike（校内课）API 的 MD5 签名
//! - [`WatchPoint`]: 观看点模拟
//! - [`hms`]: 与 Python `timedelta` 字符串一致的时长格式

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use md5::{Digest, Md5};

use super::error::{Result, ZhsError};

/// 知到 API AES-CBC 固定 IV
pub const IV: &[u8; 16] = b"1g3qqdh4jvbskb9x";
/// 课程列表等"首页"接口密钥
pub const HOME_KEY: &[u8; 16] = b"7q9oko0vqb3la20r";
/// 视频/学习进度接口密钥
pub const VIDEO_KEY: &[u8; 16] = b"azp53h0kft7qi78q";
/// hike API 签名盐值（sign.py）
pub const SIGN_SALT: &str = "o6xpt3b#Qy$Z";
/// ev 编码默认 key（d26666 对应）
pub const EV_KEY: &str = "zzpttjd";

type AesCbcEnc = cbc::Encryptor<aes::Aes128>;
type AesCbcDec = cbc::Decryptor<aes::Aes128>;

/// AES-128-CBC + PKCS7 + Base64，对应 `zd_utils.Cipher.encrypt`。
pub fn aes_encrypt(key: &[u8; 16], plaintext: &str) -> String {
    let enc = AesCbcEnc::new(key.into(), IV.into());
    let ct = enc.encrypt_padded_vec_mut::<Pkcs7>(plaintext.as_bytes());
    B64.encode(ct)
}

/// AES-128-CBC 解密 + Base64，对应 `zd_utils.Cipher.decrypt`。
pub fn aes_decrypt(key: &[u8; 16], ciphertext_b64: &str) -> Result<String> {
    let ct = B64
        .decode(ciphertext_b64)
        .map_err(|e| ZhsError::msg(format!("Base64 解码失败: {e}")))?;
    let dec = AesCbcDec::new(key.into(), IV.into());
    let pt = dec
        .decrypt_padded_vec_mut::<Pkcs7>(&ct)
        .map_err(|e| ZhsError::msg(format!("AES 解密失败: {e}")))?;
    String::from_utf8(pt).map_err(|e| ZhsError::msg(format!("UTF-8 解码失败: {e}")))
}

/// MD5 摘要（小写 hex），对应 `sign.py` 中的 `md5(raw.encode()).hexdigest()`。
pub fn md5_hex(data: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

/// hike API 学习记录签名（对应 `sign.py::sign`）。
/// 字段顺序: SALT uuid courseId fileId studyTotalTime startDate endDate endWatchTime startWatchTime uuid
pub fn sign_study_record(
    uuid: &str,
    course_id: &str,
    file_id: &str,
    study_total_time: &str,
    start_date: &str,
    end_date: &str,
    end_watch_time: &str,
    start_watch_time: &str,
) -> String {
    let raw = format!(
        "{SIGN_SALT}{uuid}{course_id}{file_id}{study_total_time}{start_date}{end_date}{end_watch_time}{start_watch_time}{uuid}"
    );
    md5_hex(&raw)
}

/// ev 混淆编码（对应 `zd_utils.getEv`）。
/// 将各字段以 `;` 连接后逐字符与 key 循环异或，输出每字符最多 4 位 hex。
pub fn get_ev(items: &[String], key: &str) -> String {
    let key_chars: Vec<u32> = key.chars().map(|c| c as u32).collect();
    let joined = items.join(";");
    let mut out = String::with_capacity(joined.len() * 2);
    for (i, ch) in joined.chars().enumerate() {
        let x = ch as u32 ^ key_chars[i % key_chars.len()];
        let mut h = format!("{x:x}");
        if h.len() < 2 {
            h.insert(0, '0');
        }
        // 与 Python 保持一致：取末 4 位（ASCII 情况下即 2 位）
        let start = h.len().saturating_sub(4);
        out.push_str(&h[start..]);
    }
    out
}

/// 时长格式化，与 Python `str(timedelta(seconds=n))` 一致。
/// 例如 125 -> "0:02:05"，3661 -> "1:01:01"，90000 -> "1 day, 1:00:00"。
pub fn hms(seconds: i64) -> String {
    let s = seconds.max(0);
    let days = s / 86400;
    let rem = s % 86400;
    let (h, m, sec) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    if days > 0 {
        let plural = if days > 1 { "s" } else { "" };
        format!("{days} day{plural}, {h}:{m:02}:{sec:02}")
    } else {
        format!("{h}:{m:02}:{sec:02}")
    }
}

/// 观看点模拟（对应 `zd_utils.WatchPoint`）。
/// 服务器通过观看点判断是否真的"看"过视频，每 2 秒记录一个点。
#[derive(Debug)]
pub struct WatchPoint {
    points: Vec<u64>,
    last: u64,
}

impl Default for WatchPoint {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchPoint {
    pub fn new() -> Self {
        Self {
            points: vec![0, 1],
            last: 1,
        }
    }

    /// 记录 [start, end] 区间内的观看点（步长 2 秒），`start` 缺省用上次位置。
    pub fn add(&mut self, end: i64, start: Option<i64>) {
        const INTERVAL: i64 = 2;
        let start = start.unwrap_or(self.last as i64);
        self.last = end.max(0) as u64;
        let mut i = start;
        while i <= end {
            self.points.push(Self::point_at(i));
            i += INTERVAL;
        }
    }

    /// 导出为逗号分隔字符串。
    pub fn get(&self) -> String {
        self.points
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// 重置观看点，对应 `reset(init)`。
    pub fn reset(&mut self, init: i64) {
        self.points = vec![0, 1];
        self.last = if init == 0 { 1 } else { init as u64 };
    }

    fn point_at(time: i64) -> u64 {
        (time.max(0) as u64) / 5 + 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hms() {
        assert_eq!(hms(125), "0:02:05");
        assert_eq!(hms(3661), "1:01:01");
        assert_eq!(hms(0), "0:00:00");
        assert_eq!(hms(90000), "1 day, 1:00:00");
    }

    #[test]
    fn test_watch_point() {
        let mut wp = WatchPoint::new();
        assert_eq!(wp.get(), "0,1");
        wp.add(5, None); // start=1, points at 1,3,5 -> gen: 1/5+2, 3/5+2, 5/5+2
        assert_eq!(wp.get(), "0,1,2,2,3");
        wp.reset(0);
        assert_eq!(wp.get(), "0,1");
    }

    #[test]
    fn test_get_ev_ascii() {
        // "12;3" 与 key "ab" 异或: '1'^'a'=0x50, '2'^'b'=0x50, ';'^'a'=0x5a, '3'^'b'=0x51
        let ev = get_ev(&["12".into(), "3".into()], "ab");
        assert_eq!(ev, "50505a51");
    }

    #[test]
    fn test_aes_roundtrip() {
        let plain = r#"{"hello":"world","n":42}"#;
        let ct = aes_encrypt(VIDEO_KEY, plain);
        let pt = aes_decrypt(VIDEO_KEY, &ct).unwrap();
        assert_eq!(pt, plain);
    }
}

#[cfg(test)]
mod cross_tests {
    use super::*;

    /// 与原版 Python getEv 的交叉验证用例（由 py -3.14 生成参考值）
    #[test]
    fn test_get_ev_cross_reference() {
        assert_eq!(get_ev(&["12".into(), "3".into()], "ab"), "50505a51");
        assert_eq!(
            get_ev(
                &[
                    "4e5f5a59425b4859".into(),
                    "12345".into(),
                    "0".into(),
                    "0".into(),
                    "0".into(),
                    "0".into(),
                    "30".into(),
                    "30".into(),
                    "0:00:30".into(),
                    "uuid123zhs".into(),
                ],
                EV_KEY
            ),
            "4e1f4512410b51434e4241165e5c4f434b454659504f41404f445154414a4b474451574a41404e445a5e494a4b010103004b48430e1c19"
        );
        assert_eq!(get_ev(&["中文".into()], EV_KEY), "4e5765fd");
    }
}
