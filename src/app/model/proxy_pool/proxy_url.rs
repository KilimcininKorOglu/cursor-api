use core::{fmt, str::FromStr};
use interned::Str;
use reqwest::Proxy;
use rkyv::{Archive, Deserialize, Serialize};

/// A serializable proxy URL wrapper
///
/// Used to store proxy URLs in scenarios where serialization/deserialization of proxy configuration is needed.
/// Internally stores a validated URL string, ensuring it can be safely converted to `reqwest::Proxy`.
#[derive(Clone, Archive, Deserialize, Serialize)]
#[rkyv(compare(PartialEq))]
#[repr(transparent)]
pub struct ProxyUrl(Str);

impl ProxyUrl {
    /// Convert ProxyUrl to reqwest::Proxy
    ///
    /// # Safety
    /// Using `unwrap_unchecked` here is safe because:
    /// - ProxyUrl can only be constructed via `FromStr::from_str`
    /// - `from_str` already validates URL validity through `Proxy::all(s)?`
    /// - Once constructed, the internal URL string is immutable
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

    /// Parse ProxyUrl from string
    ///
    /// Will pre-validate whether the URL can create a valid `Proxy`,
    /// ensuring the safety of subsequent `to_proxy` method calls.
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Verify URL validity
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
