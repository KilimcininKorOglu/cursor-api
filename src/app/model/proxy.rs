use super::{
    ApiStatus, DeleteResponseExpectation,
    proxy_pool::{Proxies, SingleProxy},
};
use interned::Str;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, sync::Arc};

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;
type HashSet<K> = hashbrown::HashSet<K, ahash::RandomState>;

// Proxy information response
#[derive(Serialize)]
pub struct ProxyInfoResponse {
    pub status: ApiStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxies: Option<Arc<HashMap<Str, SingleProxy>>>,
    pub proxies_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub general_proxy: Option<Arc<Str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Cow<'static, str>>,
}

// Update proxy configuration request
pub type ProxyUpdateRequest = Proxies;

// Add proxy request
#[derive(Deserialize)]
pub struct ProxyAddRequest {
    pub proxies: HashMap<String, SingleProxy>,
}

// Delete proxy request
#[derive(Deserialize)]
pub struct ProxiesDeleteRequest {
    #[serde(default)]
    pub names: HashSet<String>,
    #[serde(default)]
    pub expectation: DeleteResponseExpectation,
}

// Delete proxy response
#[derive(Serialize)]
pub struct ProxiesDeleteResponse {
    pub status: ApiStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_proxies: Option<Arc<HashMap<Str, SingleProxy>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_names: Option<Vec<String>>,
}

// Set general proxy request
#[derive(Deserialize)]
pub struct SetGeneralProxyRequest {
    pub name: String,
}

// Set fetch image proxy request
// #[derive(Deserialize)]
// pub struct SetFetchImageProxyRequest {
//     pub name: Option<String>,
// }
