use core::{fmt, str::FromStr};
use interned::Str;
use reqwest::Proxy;
use rkyv::{Archive, Deserialize, Serialize};

/// 一个可序列化的代理URL包装器
///
/// 用于在Need序列化/反序列化代理配置的场景中存储代理URL。
/// 内部存储经过验证的URL字符串，Ensure可以安全地ConvertTo `reqwest::Proxy`。
#[derive(Clone, Archive, Deserialize, Serialize)]
#[rkyv(compare(PartialEq))]
#[repr(transparent)]
pub struct ProxyUrl(Str);

impl ProxyUrl {
    /// 将 ProxyUrl ConvertTo reqwest::Proxy
    ///
    /// # Safety
    /// 这里Use `unwrap_unchecked` 是安全的，因To：
    /// - ProxyUrl 只能通过 `FromStr::from_str` 构造
    /// - `from_str` 中Already经通过 `Proxy::all(s)?` 验证了URL的Have效性
    /// - 一旦构造成功，内部的URL字符串就是不可变的
    #[inline]
    pub fn to_proxy(&self) -> Proxy { unsafe { Proxy::all(self.0.as_str()).unwrap_unchecked() } }
}

impl From<ProxyUrl> for Proxy {
    fn from(url: ProxyUrl) -> Self { url.to_proxy() }
}

impl fmt::Display for ProxyUrl {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

impl core::ops::Deref for ProxyUrl {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl FromStr for ProxyUrl {
    type Err = reqwest::Error;

    /// 从字符串解析 ProxyUrl
    ///
    /// 会预先验证URLWhether可以创建Have效的 `Proxy`，
    /// 这保证了后续 `to_proxy` 方法的安全性。
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // VerificationURL的Have效性
        Proxy::all(s)?;
        Ok(Self(Str::new(s)))
    }
}

impl PartialEq for ProxyUrl {
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}

impl Eq for ProxyUrl {}

impl core::hash::Hash for ProxyUrl {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) { self.0.hash(state); }
}
