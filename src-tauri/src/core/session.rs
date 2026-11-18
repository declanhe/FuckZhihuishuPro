//! HTTP 会话层 —— 对应 fuckZHS `Fucker` 的 session/headers 与查询封装。
//!
//! - [`ZhsSession::api_query`]: 普通表单/查询请求（登录、hike 课程列表等）
//! - [`ZhsSession::zhidao_query`]: 知到 API（secretStr 加密 + 时间戳，对应 `zhidaoQuery`）
//! - [`ZhsSession::hike_query`]: hike API（`_` 时间戳 + MD5 签名，对应 `hikeQuery`）

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Method, Url};
use serde_json::{json, Value};

use super::cookies::MemoryCookieStore;
use super::crypto::{self, EV_KEY};
use super::error::{Result, ZhsError};

const UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36";

/// 当前毫秒时间戳。
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub struct ZhsSession {
    client: Client,
    store: Arc<MemoryCookieStore>,
    uuid: Mutex<Option<String>>,
}

impl Default for ZhsSession {
    fn default() -> Self {
        Self::new().expect("failed to create http client")
    }
}

impl ZhsSession {
    pub fn new() -> Result<Self> {
        let store = Arc::new(MemoryCookieStore::new());
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("*/*"));
        headers.insert(
            "sec-ch-ua",
            HeaderValue::from_static(
                "\" Not A;Brand\";v=\"99\", \"Chromium\";v=\"101\", \"Google Chrome\";v=\"101\"",
            ),
        );
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert(
            "sec-ch-ua-platform",
            HeaderValue::from_static("\"macOS\""),
        );
        headers.insert("Accept-Language", HeaderValue::from_static("en-GB,en;q=0.9"));
        let client = Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            .cookie_provider(store.clone())
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;
        Ok(Self {
            client,
            store,
            uuid: Mutex::new(None),
        })
    }

    /// HTTP 客户端（供 gologin 等特殊请求直接使用）。
    pub fn http(&self) -> &Client {
        &self.client
    }

    /// 当前登录 uuid（从 CASLOGC cookie 解析）。
    pub fn uuid(&self) -> Option<String> {
        self.uuid.lock().unwrap().clone()
    }

    /// 登录成功后调用：从 cookies 解析 uuid 并设置 `exitRecod_{uuid}` 标记。
    /// 对应 `Fucker.cookies` setter。
    pub fn finalize_login(&self) -> Result<()> {
        let cas_url: Url = "https://passport.zhihuishu.com/".parse().unwrap();
        let cas = self
            .store
            .get(&cas_url, "CASLOGC")
            .ok_or_else(|| ZhsError::msg("Cookies invalid: 缺少 CASLOGC"))?;
        // CASLOGC 是 URL 编码的 JSON，取其中 uuid 字段
        let decoded = percent_decode(&cas);
        let v: Value = serde_json::from_str(&decoded)
            .map_err(|e| ZhsError::msg(format!("CASLOGC 解析失败: {e}")))?;
        let uuid = v
            .get("uuid")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ZhsError::msg("CASLOGC 中没有 uuid"))?;
        // 设置 exitRecod_{uuid} = 2
        self.store.set_raw(&format!("exitRecod_{uuid}"), "2", "zhihuishu.com");
        *self.uuid.lock().unwrap() = Some(uuid);
        Ok(())
    }

    /// 导出全部 cookie（持久化用）。
    pub fn export_cookies(&self) -> Value {
        json!(self.store.export())
    }

    /// 导入 cookie（恢复登录态）。
    pub fn import_cookies(&self, cookies: Value) -> Result<()> {
        let list: Vec<super::cookies::StoredCookie> =
            serde_json::from_value(cookies).map_err(|e| ZhsError::msg(format!("cookies 格式错误: {e}")))?;
        self.store.import(list);
        // 尝试恢复 uuid，失败不报错（可能 cookie 已过期）
        let _ = self.finalize_login();
        Ok(())
    }

    /// 清空登录态。
    pub fn logout(&self) {
        self.store.import(Vec::new());
        *self.uuid.lock().unwrap() = None;
    }

    pub fn is_logged_in(&self) -> bool {
        self.uuid.lock().unwrap().is_some() && !self.store.export().is_empty()
    }

    /// 发起请求并解析 JSON 响应。
    async fn request(
        &self,
        method: Method,
        url: &str,
        extra_headers: &[(&str, &str)],
        query: Option<&[(String, String)]>,
        form: Option<&[(String, String)]>,
    ) -> Result<Value> {
        let mut req = self.client.request(method, url);
        for (k, v) in extra_headers {
            req = req.header(*k, *v);
        }
        if let Some(q) = query {
            req = req.query(q);
        }
        if let Some(f) = form {
            req = req.form(f);
        }
        let resp = req.send().await?;
        let resp = resp.error_for_status()?;
        // 服务器在登录失效时可能返回 HTML 跳转脚本而非 JSON
        let text = resp.text().await?;
        let trimmed = text.trim_start();
        if trimmed.starts_with('<') {
            return Err(ZhsError::NotLoggedIn);
        }
        Ok(serde_json::from_str::<Value>(&text).map_err(|e| {
            ZhsError::msg(format!("响应不是有效 JSON: {e}；内容开头: {}", {
                let head: String = text.chars().take(120).collect();
                head
            }))
        })?)
    }

    /// 直接提交表单对并解析 JSON 响应（用于 secretStr 等预构建字段）。
    pub async fn post_form(
        &self,
        url: &str,
        form: &[(String, String)],
        extra_headers: &[(&str, &str)],
    ) -> Result<Value> {
        self.request(Method::POST, url, extra_headers, None, Some(form))
            .await
    }

    /// 普通 API 请求（对应 `_apiQuery`）：GET 用 query 参数，POST 用表单。
    pub async fn api_query(
        &self,
        url: &str,
        data: Option<&Value>,
        method: Method,
        extra_headers: &[(&str, &str)],
    ) -> Result<Value> {
        match method {
            Method::POST => {
                let pairs = value_to_pairs(data.unwrap_or(&json!({})));
                self.request(method, url, extra_headers, None, Some(&pairs))
                    .await
            }
            _ => {
                let pairs = data.map(value_to_pairs);
                let q = pairs.as_deref();
                self.request(method, url, extra_headers, q, None).await
            }
        }
    }

    /// 知到 API（对应 `zhidaoQuery`）：
    /// 数据 JSON 化 -> AES-CBC 加密为 secretStr -> 表单提交，附带毫秒时间戳。
    /// `ok_code` 传 `None` 跳过状态码检查；code == -12 视为需要验证码。
    pub async fn zhidao_query(
        &self,
        url: &str,
        data: &Value,
        key: &[u8; 16],
        ok_code: Option<i64>,
        method: Method,
        extra_headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut payload = data.clone();
        let ts = now_ms() / 1000 * 1000; // 与 Python 一致：秒 * 1000，末尾恒为 000
        payload["dateFormate"] = json!(ts);
        let secret = crypto::aes_encrypt(key, &payload.to_string());
        let form = vec![
            ("secretStr".to_string(), secret),
            ("dateFormate".to_string(), ts.to_string()),
        ];
        let ret = self
            .request(method, url, extra_headers, None, Some(&form))
            .await?;
        if let Some(ok) = ok_code {
            let code = ret.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            if code != ok {
                return match code {
                    -12 => Err(ZhsError::Captcha),
                    _ => Err(ZhsError::msg(format!(
                        "code: {code} msg: {}",
                        ret.get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("（无消息）")
                    ))),
                };
            }
        }
        Ok(ret)
    }

    /// hike API（对应 `hikeQuery`）：附 `_` 时间戳；`sig` 时对所有参数做 MD5 签名。
    pub async fn hike_query(
        &self,
        url: &str,
        params: &Value,
        sig: bool,
        ok_code: Option<i64>,
        extra_headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut obj = params.clone();
        obj["_"] = json!(now_ms());
        let mut pairs = value_to_pairs(&obj);
        if sig {
            // 签名字段顺序固定，与 sign.py 一致；所有值先转字符串
            let s = |k: &str| {
                pairs
                    .iter()
                    .find(|(k2, _)| k2 == k)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            let signature = crypto::sign_study_record(
                &s("uuid"),
                &s("courseId"),
                &s("fileId"),
                &s("studyTotalTime"),
                &s("startDate"),
                &s("endDate"),
                &s("endWatchTime"),
                &s("startWatchTime"),
            );
            pairs.push(("signature".to_string(), signature));
        }
        let ret = self
            .request(Method::GET, url, extra_headers, Some(&pairs), None)
            .await?;
        if let Some(ok) = ok_code {
            // hike 接口正常返回 {"status":200,...}，业务错误可能返回
            // {"code":500,"message":...}（无 status 字段）——两种都要展示真实原因
            // 注意 status 可能是字符串（实测 queryResourceMenuTree 返回 "status":"200"）
            let status_field = ret.get("status").and_then(parse_status_i64);
            let code_field = ret.get("code").and_then(parse_status_i64);
            let msg = ret
                .get("message")
                .or_else(|| ret.get("msg")) // 两种字段名都见过
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let failed = match (status_field, code_field) {
                (Some(s), _) => s != ok,
                (None, Some(c)) => c != 200, // 业务错误响应
                (None, None) => true,        // 响应里没有任何状态字段
            };
            if failed {
                let detail = if msg.is_empty() {
                    serde_json::to_string(&ret).unwrap_or_default()
                } else {
                    msg
                };
                let shown = status_field.or(code_field).unwrap_or(-1);
                return Err(ZhsError::msg(format!("{shown} {detail}")));
            }
        }
        Ok(ret)
    }

    /// 获取文本响应（用于 JSONP 等）。
    pub async fn get_text(&self, url: &str, query: &[(String, String)]) -> Result<String> {
        let resp = self
            .client
            .get(url)
            .query(query)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.text().await?)
    }
}

/// 兼容数字与字符串两种状态的 status 解码（服务器两种都返回过）。
fn parse_status_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// 将 JSON 对象压平为 (key, value) 字符串对（值按 Python str() 语义字符串化）。
pub fn value_to_pairs(v: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Value::Object(map) = v {
        for (k, val) in map {
            out.push((k.clone(), json_value_to_string(val)));
        }
    }
    out
}

fn json_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => {
            if *b {
                "True".into() // 与 Python str(True) 保持一致
            } else {
                "False".into()
            }
        }
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 学习进度 ev 编码（zhidao V2 上报用）。
pub fn ev_encode(items: &[String]) -> String {
    crypto::get_ev(items, EV_KEY)
}

/// Base64 编码（标准字母表，带 padding），对应 Python `b64encode`。
pub fn b64_encode(data: &str) -> String {
    B64.encode(data.as_bytes())
}

/// 对应网页版登录 Encrypt()：btoa(encodeURI(word))。
/// 先按 JS encodeURIComponent 规则转义，再标准 Base64。
pub fn b64_encode_uri(data: &str) -> String {
    b64_encode(&encodeURIComponent(data))
}

/// JS encodeURIComponent 的 Rust 等价实现。
/// 不转义字符：A-Z a-z 0-9 - _ . ! ~ * ' ( )
pub fn encodeURIComponent(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 简易百分号解码（处理 %XX 与 '+'）。
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("%7B%22uuid%22%3A%22123%22%7D"), r#"{"uuid":"123"}"#);
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn test_value_to_pairs() {
        let v = json!({"a": "x", "b": 1, "c": true});
        let pairs = value_to_pairs(&v);
        assert!(pairs.contains(&("a".into(), "x".into())));
        assert!(pairs.contains(&("b".into(), "1".into())));
        assert!(pairs.contains(&("c".into(), "True".into())));
    }
}
