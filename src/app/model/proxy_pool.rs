use crate::app::lazy::{
    HTTP2_ADAPTIVE_WINDOW, HTTP2_KEEP_ALIVE_INTERVAL, HTTP2_KEEP_ALIVE_TIMEOUT,
    HTTP2_KEEP_ALIVE_WHILE_IDLE, PROXIES_FILE_PATH, SERVICE_TIMEOUT, TCP_KEEPALIVE,
    TCP_KEEPALIVE_INTERVAL, TCP_KEEPALIVE_RETRIES,
};
use alloc::sync::Arc;
use arc_swap::{ArcSwap, ArcSwapAny};
use core::{str::FromStr, time::Duration};
use interned::Str;
use manually_init::ManuallyInit;
use memmap2::{MmapMut, MmapOptions};
use reqwest::Client;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
mod proxy_url;
use proxy_url::ProxyUrl;

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;
type HashSet<K> = hashbrown::HashSet<K, ahash::RandomState>;

// Proxy value constants
const NON_PROXY: &str = "non";
const SYS_PROXY: &str = "sys";

/// Create default proxy configuration
///
/// Contains a system proxy configuration
#[inline]
pub fn default_proxies() -> HashMap<Str, SingleProxy> {
    HashMap::from_iter([(SYS_PROXY.into(), SingleProxy::Sys)])
}

/// Name to proxy configuration mapping
static PROXIES: ManuallyInit<ArcSwap<HashMap<Str, SingleProxy>>> = ManuallyInit::new();

/// General proxy name
static GENERAL_NAME: ManuallyInit<ArcSwap<Str>> = ManuallyInit::new();

// /// Get image proxy name
// static FETCH_IMAGE_NAME: ArcSwapOption<String> = ArcSwapOption::const_empty();

/// Proxy configuration to client instance mapping
///
/// Cache already created clients, avoid creating duplicate clients with same configuration
static CLIENTS: ManuallyInit<ArcSwap<HashMap<SingleProxy, Client>>> = ManuallyInit::new();

/// General client
///
/// Used for requests without specified proxy, points to client corresponding to GENERAL_NAME
static GENERAL_CLIENT: ManuallyInit<ArcSwapAny<Client>> = ManuallyInit::new();

// /// Get image client
// ///
// /// Used for HTTP image fetch requests, points to client corresponding to FETCH_IMAGE_NAME
// static FETCH_IMAGE_CLIENT: ArcSwapAny<Option<Client>> = unsafe {
//     core::intrinsics::transmute_unchecked::<ArcSwapOption<()>, _>(ArcSwapOption::const_empty())
// };

/// Proxy configuration manager
///
/// Responsible for managing all proxy configurations and their corresponding clients
#[derive(Clone, Deserialize, Serialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct Proxies {
    /// Name to proxy configuration mapping
    proxies: HashMap<Str, SingleProxy>,
    /// Default proxy name to use
    general: Str,
}

impl Default for Proxies {
    fn default() -> Self { Self { proxies: default_proxies(), general: SYS_PROXY.into() } }
}

impl Proxies {
    /// Initialize global proxy system
    ///
    /// Verify configuration completeness and create all necessary clients
    #[inline]
    pub fn init(mut self) {
        // Ensure at least default proxy exists
        if self.proxies.is_empty() {
            self.proxies = default_proxies();
            if self.general.as_str() != SYS_PROXY {
                self.general = SYS_PROXY.into();
            }
        } else if !self.proxies.contains_key(&self.general) {
            // General proxy name invalid, use first available proxy
            self.general = __unwrap!(self.proxies.keys().next()).clone();
        }

        // Collect all unique proxy configurations
        let proxies = self.proxies.values().collect::<HashSet<_>>();
        let mut clients =
            HashMap::with_capacity_and_hasher(proxies.len(), ::ahash::RandomState::new());

        // Create client for each proxy configuration
        for proxy in proxies {
            proxy.insert_to(&mut clients);
        }

        // Initialize global static variables
        // Safety: previous logic already ensures general exists in proxies,
        // and all proxies have corresponding clients
        GENERAL_CLIENT.init(ArcSwapAny::from(
            __unwrap!(clients.get(__unwrap!(self.proxies.get(&self.general)))).clone(),
        ));
        CLIENTS.init(ArcSwap::from_pointee(clients));
        PROXIES.init(ArcSwap::from_pointee(self.proxies));
        GENERAL_NAME.init(ArcSwap::from_pointee(self.general));
    }

    /// Update global proxy configuration (do not update client pool)
    #[inline]
    pub fn update_global(self) {
        proxies().store(Arc::new(self.proxies));
        general_name().store(Arc::new(self.general));
    }

    /// Update global proxy pool
    ///
    /// Smart update client pool:
    /// - Remove clients no longer in use
    /// - Create clients for new proxy configurations
    /// - Keep clients still in use
    fn update_global_pool() {
        let proxies = proxies().load();
        let mut general_name = general_name().load_full();
        let mut clients = (*clients().load_full()).clone();

        // Ensure configuration validity
        if proxies.is_empty() {
            self::proxies().store(Arc::new(default_proxies()));
            if general_name.as_str() != SYS_PROXY {
                general_name = Arc::new(SYS_PROXY.into());
            }
        } else if !proxies.contains_key(&*general_name) {
            // General proxy name invalid, select first available
            general_name = Arc::new(__unwrap!(proxies.keys().next()).clone());
        }

        // Collect all unique proxies in current configuration
        let current_proxies: HashSet<&SingleProxy> = proxies.values().collect();

        // Remove clients no longer in use
        let to_remove: Vec<SingleProxy> =
            clients.keys().filter(|proxy| !current_proxies.contains(proxy)).cloned().collect();

        for proxy in to_remove {
            clients.remove(&proxy);
        }

        // Create clients for new proxy configurations
        for proxy in current_proxies {
            if !clients.contains_key(proxy) {
                proxy.insert_to(&mut clients);
            }
        }

        // Update global state
        self::clients().store(Arc::new(clients));
        self::general_name().store(general_name);
        set_general();
    }

    /// Save proxy configuration to file
    pub async fn save() -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&Self {
            proxies: (*proxies().load_full()).clone(),
            general: (*general_name().load_full()).clone(),
        })?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*PROXIES_FILE_PATH)
            .await?;

        // Prevent file from being too large
        if bytes.len() > usize::MAX >> 1 {
            return Err("Proxy data too large".into());
        }

        file.set_len(bytes.len() as u64).await?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }

    /// Load proxy configuration from file
    pub async fn load() -> Result<Self, Box<dyn core::error::Error + Send + Sync + 'static>> {
        let file = match OpenOptions::new().read(true).open(&*PROXIES_FILE_PATH).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => return Err(Box::new(e)),
        };

        if file.metadata().await?.len() > usize::MAX as u64 {
            return Err("Proxy file too large".into());
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Safety: file content is controlled by us, format is guaranteed correct
        unsafe {
            ::rkyv::from_bytes_unchecked::<Self, ::rkyv::rancor::Error>(&mmap)
                .map_err(|_| "Load proxies failed".into())
        }
    }

    /// Update global proxy pool and save configuration
    #[inline]
    pub async fn update_and_save() -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>>
    {
        Self::update_global_pool();
        Self::save().await
    }
}

/// Single proxy configuration
#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize, PartialEq, Eq, Hash)]
#[rkyv(compare(PartialEq))]
pub enum SingleProxy {
    /// Do not use proxy
    Non,
    /// Use system proxy
    Sys,
    /// Use proxy with specified URL
    Url(ProxyUrl),
}

impl SingleProxy {
    /// Create corresponding client based on proxy configuration and insert into mapping
    #[inline]
    fn insert_to(&self, clients: &mut HashMap<SingleProxy, Client>) {
        let builder = Client::builder()
            .https_only(true)
            .tcp_keepalive(TCP_KEEPALIVE.to_duration())
            .tcp_keepalive_interval(TCP_KEEPALIVE_INTERVAL.to_duration())
            .tcp_keepalive_retries(TCP_KEEPALIVE_RETRIES.to_count())
            .http2_adaptive_window(*HTTP2_ADAPTIVE_WINDOW)
            .http2_keep_alive_interval(HTTP2_KEEP_ALIVE_INTERVAL.to_duration())
            .http2_keep_alive_timeout(HTTP2_KEEP_ALIVE_TIMEOUT.to_duration_or_default())
            .http2_keep_alive_while_idle(*HTTP2_KEEP_ALIVE_WHILE_IDLE)
            .connect_timeout(Duration::from_secs(*SERVICE_TIMEOUT as _))
            .webpki_roots_only();
        let client = match self {
            SingleProxy::Non => builder.no_proxy().build().expect("Create no-proxy client failed"),
            SingleProxy::Sys => builder.build().expect("Create default client failed"),
            SingleProxy::Url(url) => {
                builder.proxy(url.to_proxy()).build().expect("Create proxy client failed")
            }
        };

        clients.insert(self.clone(), client);
    }
}

impl Serialize for SingleProxy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self {
            Self::Non => serializer.serialize_str(NON_PROXY),
            Self::Sys => serializer.serialize_str(SYS_PROXY),
            Self::Url(url) => serializer.serialize_str(&*url),
        }
    }
}

impl<'de> Deserialize<'de> for SingleProxy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        struct SingleProxyVisitor;

        impl serde::de::Visitor<'_> for SingleProxyVisitor {
            type Value = SingleProxy;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a string representing 'non', 'sys', or a valid URL")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where E: serde::de::Error {
                match value {
                    NON_PROXY => Ok(Self::Value::Non),
                    SYS_PROXY => Ok(Self::Value::Sys),
                    url_str => Ok(Self::Value::Url(
                        ProxyUrl::from_str(url_str)
                            .map_err(|e| E::custom(format_args!("Invalid URL: {e}")))?,
                    )),
                }
            }
        }

        deserializer.deserialize_str(SingleProxyVisitor)
    }
}

impl core::fmt::Display for SingleProxy {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Non => f.write_str(NON_PROXY),
            Self::Sys => f.write_str(SYS_PROXY),
            Self::Url(url) => f.write_str(url),
        }
    }
}

impl FromStr for SingleProxy {
    type Err = reqwest::Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            NON_PROXY => Ok(Self::Non),
            SYS_PROXY => Ok(Self::Sys),
            url_str => Ok(Self::Url(ProxyUrl::from_str(url_str)?)),
        }
    }
}

/// Get corresponding client by name
///
/// If specified proxy name not found, return general client
#[inline]
pub fn get_client(name: &str) -> Client {
    // First find proxy configuration by name
    if let Some(proxy) = proxies().load().get(name) {
        // Then find client by proxy configuration
        if let Some(client) = clients().load().get(proxy) {
            return client.clone();
        }
    }

    // Return general client
    get_general_client()
}

/// Get general client
#[inline]
pub fn get_general_client() -> Client { general_client().load_full() }

/// Get client by optional name
#[inline]
pub fn get_client_or_general(name: Option<&str>) -> Client {
    match name {
        Some(name) => get_client(name),
        None => get_general_client(),
    }
}

/// Get fetch image client
#[inline]
pub fn get_fetch_image_client() -> Client {
    // fetch_image_client().load_full().unwrap_or_else(get_general_client)
    get_general_client()
}

/// Update general client reference
///
/// Precondition: general_name must exist in proxies,
/// and corresponding proxy must exist in clients
#[inline]
fn set_general() {
    general_client().store(unsafe {
        clients()
            .load()
            .get(proxies().load().get(&*general_name().load_full()).unwrap_unchecked())
            .unwrap_unchecked()
            .clone()
    });
}

// Accessor functions
#[inline]
pub fn proxies() -> &'static ArcSwap<HashMap<Str, SingleProxy>> { PROXIES.get() }

#[inline]
pub fn general_name() -> &'static ArcSwap<Str> { GENERAL_NAME.get() }

// #[inline]
// pub fn fetch_image_name() -> &'static ArcSwapOption<String> { &FETCH_IMAGE_NAME }

#[inline]
fn clients() -> &'static ArcSwap<HashMap<SingleProxy, Client>> { CLIENTS.get() }

#[inline]
fn general_client() -> &'static ArcSwapAny<Client> { GENERAL_CLIENT.get() }

// #[inline]
// fn fetch_image_client() -> &'static ArcSwapAny<Option<Client>> { &FETCH_IMAGE_CLIENT }
