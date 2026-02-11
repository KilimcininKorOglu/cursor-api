mod token;

use super::{
    log::{LogManager, create_task},
    proxy_pool::Proxies,
};
use core::sync::atomic::{AtomicU64, Ordering};
pub use token::{QueueType, TokenError, TokenHealth, TokenManager, TokenWriter};
use tokio::sync::RwLock;

pub struct AppState {
    pub token_manager: RwLock<TokenManager>,
    pub total_requests: AtomicU64,
    pub active_requests: AtomicU64,
    pub error_requests: AtomicU64,
}

impl AppState {
    pub async fn load() -> Result<Self, Box<dyn core::error::Error + Send + Sync + 'static>> {
        // Load logs, tokens and proxies in parallel
        let (log_manager_result, token_manager_result, proxies_result) =
            tokio::join!(LogManager::load(), TokenManager::load(), Proxies::load());

        // Get results, handle errors
        let log_manager = log_manager_result?;
        let token_manager = token_manager_result?;

        // Handle proxies
        let proxies = proxies_result.unwrap_or_default();
        proxies.init();

        // Calculate initial statistics information
        let error_count = log_manager.error_count();
        let total_count = log_manager.total_count();

        create_task(log_manager);

        Ok(Self {
            token_manager: RwLock::new(token_manager),
            total_requests: AtomicU64::new(total_count),
            active_requests: AtomicU64::new(0),
            error_requests: AtomicU64::new(error_count),
        })
    }

    /// Increment total request count
    #[inline(always)]
    pub fn increment_total(&self) { self.total_requests.fetch_add(1, Ordering::Relaxed); }

    /// Increment active request count
    #[inline(always)]
    pub fn increment_active(&self) { self.active_requests.fetch_add(1, Ordering::Relaxed); }

    /// Decrement active request count
    #[inline(always)]
    pub fn decrement_active(&self) { self.active_requests.fetch_sub(1, Ordering::Relaxed); }

    /// Increment error request count
    #[inline(always)]
    pub fn increment_error(&self) { self.error_requests.fetch_add(1, Ordering::Relaxed); }

    /// Get read lock of token manager
    #[inline]
    pub async fn token_manager_read(&self) -> tokio::sync::RwLockReadGuard<'_, TokenManager> {
        self.token_manager.read().await
    }

    /// Get write lock of token manager
    #[inline]
    pub async fn token_manager_write(&self) -> tokio::sync::RwLockWriteGuard<'_, TokenManager> {
        self.token_manager.write().await
    }

    pub async fn save(&self) -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        // Save logs, tokens and proxies in parallel
        let (log_result, tokens_result, proxies_result) =
            tokio::join!(LogManager::save(), self.save_tokens(), Proxies::save());

        log_result?;
        tokens_result?;
        proxies_result?;
        Ok(())
    }

    async fn save_tokens(&self) -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        self.token_manager.read().await.save().await
    }

    /// Update client key in token manager
    pub async fn update_client_key(&self) { self.token_manager.write().await.update_client_key() }
}
