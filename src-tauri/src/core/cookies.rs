//! 自实现的 Cookie 存储层。
//!
//! 为什么不用 reqwest 内置的 `cookie::Jar`：内置 Jar 无法遍历/导出 cookies，
//! 而客户端需要把登录会话持久化到磁盘以实现"短时间免登录"（对应 fuckZHS 的 cookies.json）。

use std::sync::Mutex;

use reqwest::cookie::CookieStore;
use reqwest::header::HeaderValue;
use reqwest::Url;
use serde::{Deserialize, Serialize};

/// 单条 cookie 的可序列化记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    /// 归属域名（小写，不带前导点）
    pub domain: String,
    /// 是否仅限精确 host（Set-Cookie 未带 Domain 属性时）
    pub host_only: bool,
    pub path: String,
    pub secure: bool,
    /// 过期时间戳（秒）；None = 会话 cookie。仅记录，存储层不做过期淘汰。
    #[serde(default)]
    #[allow(dead_code)]
    pub expires: Option<i64>,
}

#[derive(Debug, Default)]
pub struct MemoryCookieStore {
    inner: Mutex<Vec<StoredCookie>>,
}

impl Clone for MemoryCookieStore {
    fn clone(&self) -> Self {
        Self {
            inner: Mutex::new(self.inner.lock().unwrap().clone()),
        }
    }
}

impl MemoryCookieStore {
    /// 新建空存储。
    pub fn new() -> Self {
        Self::default()
    }

    /// 导出全部 cookie（用于持久化）。
    pub fn export(&self) -> Vec<StoredCookie> {
        self.inner.lock().unwrap().clone()
    }

    /// 导入 cookie（覆盖同名项）。
    pub fn import(&self, cookies: Vec<StoredCookie>) {
        let mut inner = self.inner.lock().unwrap();
        for c in cookies {
            match inner.iter_mut().find(|x| {
                x.name == c.name && x.domain == c.domain && x.path == c.path
            }) {
                Some(existing) => *existing = c,
                None => inner.push(c),
            }
        }
    }

    /// 以编程方式设置一条 cookie（对应 fuckZHS 的
    /// `self._cookies[f"exitRecod_{uuid}"] = "2"`）。
    pub fn set_raw(&self, name: &str, value: &str, domain: &str) {
        let mut inner = self.inner.lock().unwrap();
        let cookie = StoredCookie {
            name: name.into(),
            value: value.into(),
            domain: domain.trim_start_matches('.').to_lowercase(),
            host_only: false,
            path: "/".into(),
            secure: false,
            expires: None,
        };
        match inner
            .iter_mut()
            .find(|x| x.name == cookie.name && x.domain == cookie.domain && x.path == "/")
        {
            Some(existing) => *existing = cookie,
            None => inner.push(cookie),
        }
    }

    /// 按名称读取（优先匹配请求域名），对应 `cookies["CASLOGC"]`。
    pub fn get(&self, url: &Url, name: &str) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        let host = url.host_str()?.to_lowercase();
        inner
            .iter()
            .find(|c| {
                c.name == name && domain_matches(&host, c)
            })
            .map(|c| c.value.clone())
    }

    fn matches(&self, url: &Url, c: &StoredCookie) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        let host = host.to_lowercase();
        if !domain_matches(&host, c) {
            return false;
        }
        if !url.path().starts_with(&c.path) {
            return false;
        }
        if c.secure && url.scheme() != "https" {
            return false;
        }
        true
    }
}

fn domain_matches(host: &str, c: &StoredCookie) -> bool {
    if c.host_only {
        host == c.domain
    } else {
        host == c.domain || host.ends_with(&format!(".{}", c.domain))
    }
}

impl CookieStore for MemoryCookieStore {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &HeaderValue>,
        url: &Url,
    ) {
        let Some(host) = url.host_str().map(|h| h.to_lowercase()) else {
            return;
        };
        let mut inner = self.inner.lock().unwrap();
        for hv in cookie_headers {
            let Ok(raw) = hv.to_str() else { continue };
            let mut parts = raw.split(';');
            let Some(first) = parts.next() else { continue };
            let Some((name, value)) = first.split_once('=') else { continue };
            let (name, value) = (name.trim(), value.trim());
            if name.is_empty() {
                continue;
            }
            let mut cookie = StoredCookie {
                name: name.to_string(),
                value: value.to_string(),
                domain: host.clone(),
                host_only: true,
                path: "/".into(),
                secure: false,
                expires: None,
            };
            for attr in parts {
                let attr = attr.trim();
                let (key, val) = match attr.split_once('=') {
                    Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim().to_string()),
                    None => (attr.to_ascii_lowercase(), String::new()),
                };
                match key.as_str() {
                    "domain" if !val.is_empty() => {
                        cookie.domain = val.trim_start_matches('.').to_lowercase();
                        cookie.host_only = false;
                    }
                    "path" if val.starts_with('/') => cookie.path = val,
                    "secure" => cookie.secure = true,
                    _ => {}
                }
            }
            // upsert: 同 (name, domain, path) 覆盖
            match inner.iter_mut().find(|x| {
                x.name == cookie.name && x.domain == cookie.domain && x.path == cookie.path
            }) {
                Some(existing) => *existing = cookie,
                None => inner.push(cookie),
            }
        }
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let inner = self.inner.lock().unwrap();
        let pairs: Vec<String> = inner
            .iter()
            .filter(|c| self.matches(url, c))
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();
        if pairs.is_empty() {
            return None;
        }
        HeaderValue::from_str(&pairs.join("; ")).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let store = MemoryCookieStore::new();
        let url: Url = "https://passport.zhihuishu.com/login".parse().unwrap();
        let hv = HeaderValue::from_str("CASLOGC=%7B%22uuid%22%3A%22123%22%7D; Path=/").unwrap();
        store.set_cookies(&mut [&hv].into_iter(), &url);
        assert_eq!(
            store.get(&url, "CASLOGC").unwrap(),
            "%7B%22uuid%22%3A%22123%22%7D"
        );
        // 其他域名读不到
        let other: Url = "https://hike.zhihuishu.com/x".parse().unwrap();
        assert!(store.get(&other, "CASLOGC").is_none());
        // 通配域名
        let hv = HeaderValue::from_str("a=1; Domain=.zhihuishu.com; Path=/").unwrap();
        store.set_cookies(&mut [&hv].into_iter(), &url);
        assert_eq!(store.get(&other, "a").unwrap(), "1");
    }
}
