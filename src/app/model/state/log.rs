use crate::app::{
    lazy::LOGS_FILE_PATH,
    model::{ExtToken, ExtTokenHelper, RequestLog, TokenKey, log::RequestLogHelper},
};
use alloc::collections::VecDeque;
use memmap2::{MmapMut, MmapOptions};
use rkyv::{
    Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize, rancor::Error as RkyvError,
};
use tokio::fs::OpenOptions;

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;

/// Request log limit enumeration
#[derive(Debug, Clone, Copy)]
pub enum RequestLogsLimit {
    /// Logging disabled
    Disabled,
    /// Unlimited logging
    Unlimited,
    /// Logging with limit, parameter is maximum log count
    Limited(usize),
}

impl RequestLogsLimit {
    /// Create RequestLogsLimit from usize
    #[inline]
    pub fn from_usize(limit: usize) -> Self {
        const MAX_LIMIT: usize = 1000000;
        match limit {
            0 => Self::Disabled,
            n if n >= MAX_LIMIT => Self::Unlimited,
            n => Self::Limited(n),
        }
    }

    /// Check whether need to save logs
    #[inline(always)]
    pub fn should_log(&self) -> bool { !matches!(self, Self::Disabled) }

    /// Get log limit
    #[inline(always)]
    pub fn get_limit(&self) -> Option<usize> {
        match self {
            Self::Disabled => Some(0),
            Self::Unlimited => None,
            Self::Limited(limit) => Some(*limit),
        }
    }
}

/// Helper structure for rkyv serialization
#[derive(Archive, RkyvDeserialize, RkyvSerialize)]
struct LogManagerHelper {
    logs: Vec<RequestLogHelper>,
    tokens: HashMap<TokenKey, ExtTokenHelper>,
}

/// Log manager, responsible for centralized management of logs and tokens
pub struct LogManager {
    logs: VecDeque<RequestLog>,
    tokens: HashMap<TokenKey, ExtToken>,
    token_ref_counts: HashMap<TokenKey, usize>, // token reference count
    logs_limit: RequestLogsLimit,
}

impl LogManager {
    /// Create new log manager
    #[inline]
    pub fn new(logs_limit: RequestLogsLimit) -> Self {
        Self {
            logs: match logs_limit {
                RequestLogsLimit::Disabled => VecDeque::new(),
                RequestLogsLimit::Limited(limit) if limit < 128 => VecDeque::with_capacity(limit),
                _ => VecDeque::with_capacity(32),
            },
            tokens: HashMap::default(),
            token_ref_counts: HashMap::default(),
            logs_limit,
        }
    }

    /// Load logs from storage
    #[inline(never)]
    pub async fn load() -> Result<Self, Box<dyn core::error::Error + Send + Sync + 'static>> {
        let logs_limit = RequestLogsLimit::from_usize(crate::common::utils::parse_from_env(
            "REQUEST_LOGS_LIMIT",
            100usize,
        ));

        // If logging disabled, return empty manager
        if !logs_limit.should_log() {
            return Ok(Self::new(logs_limit));
        }

        let (logs, tokens) = {
            let file = match OpenOptions::new().read(true).open(&*LOGS_FILE_PATH).await {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Self::new(logs_limit));
                }
                Err(e) => return Err(Box::new(e)),
            };

            if file.metadata().await?.len() > usize::MAX as u64 {
                return Err("Log file too large".into());
            }

            let mmap = unsafe { MmapOptions::new().map(&file)? };
            let helper =
                unsafe { ::rkyv::from_bytes_unchecked::<LogManagerHelper, RkyvError>(&mmap) }?;

            let logs = helper.logs.into_iter().map(RequestLogHelper::into_request_log).collect();

            let tokens = helper.tokens.into_iter().map(|(k, v)| (k, v.extract())).collect();

            (logs, tokens)
        };
        let mut manager = Self { logs, tokens, token_ref_counts: HashMap::default(), logs_limit };

        // Rebuild token reference count
        manager.rebuild_token_ref_counts();

        Ok(manager)
    }

    /// Rebuild token reference count
    #[inline(never)]
    fn rebuild_token_ref_counts(&mut self) {
        self.token_ref_counts.clear();

        // Count how many logs reference each token
        for log in &self.logs {
            let token_key = log.token_key();
            *self.token_ref_counts.entry(token_key).or_insert(0) += 1;
        }

        // Remove tokens that are not referenced
        self.tokens.retain(|key, _| self.token_ref_counts.contains_key(key));
    }

    /// Increment token reference count
    #[inline]
    fn increment_token_ref(&mut self, token_key: TokenKey) {
        *self.token_ref_counts.entry(token_key).or_insert(0) += 1;
    }

    /// Decrement token reference count, if count reaches 0 then clean up token
    #[inline]
    fn decrement_token_ref(&mut self, token_key: TokenKey) {
        if let Some(count) = self.token_ref_counts.get_mut(&token_key) {
            *count -= 1;
            if *count == 0 {
                self.token_ref_counts.remove(&token_key);
                self.tokens.remove(&token_key);
            }
        }
    }

    /// Internal method: add or update token (only call when needed)
    #[inline]
    fn insert_token(&mut self, key: TokenKey, token: ExtToken) { self.tokens.insert(key, token); }

    /// Public method: update related token when adding log
    #[inline(never)]
    pub fn push_log_with_token(&mut self, log: RequestLog, ext_token: ExtToken) {
        // If logging disabled, return directly
        if !self.logs_limit.should_log() {
            return;
        }

        let log_token_key = log.token_key();

        // Manage log queue based on limit strategy
        if let Some(limit) = self.logs_limit.get_limit() {
            while self.logs.len() >= limit {
                if let Some(removed_log) = self.logs.pop_front() {
                    // Decrement token reference count of removed log
                    let removed_token_key = removed_log.token_key();
                    self.decrement_token_ref(removed_token_key);
                }
            }
        }

        // Add new token (if provided and not exists)
        // debug_assert_eq!(token_key, log_token_key, "token key does not match in log");
        self.insert_token(log_token_key, ext_token);

        // Increment token reference count of new log
        self.increment_token_ref(log_token_key);

        // Add log
        self.logs.push_back(log);
    }

    /// Save data to file
    #[inline(never)]
    pub async fn save(&self) -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        // If logging disabled, skip saving
        if !self.logs_limit.should_log() {
            return Ok(());
        }

        let helper = LogManagerHelper {
            logs: self.logs.iter().map(RequestLogHelper::from).collect(),
            tokens: self.tokens.iter().map(|(k, v)| (*k, ExtTokenHelper::new(v))).collect(),
        };

        let bytes = rkyv::to_bytes::<RkyvError>(&helper)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*LOGS_FILE_PATH)
            .await?;

        if bytes.len() > usize::MAX >> 1 {
            return Err("Log data too large".into());
        }

        file.set_len(bytes.len() as u64).await?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }

    /// Get read-only reference to logs
    #[inline]
    pub fn logs(&self) -> &VecDeque<RequestLog> { &self.logs }

    // /// Get mutable reference to logs
    // #[inline]
    // pub fn logs_mut(&mut self) -> &mut VecDeque<RequestLog> {
    //     &mut self.logs
    // }

    /// Get read-only reference to tokens
    #[inline]
    pub fn tokens(&self) -> &HashMap<TokenKey, ExtToken> { &self.tokens }

    // /// Compatibility method: add new log (not recommended, use push_log_with_token instead)
    // #[inline]
    // pub fn push_log(&mut self, log: RequestLog) {
    //     self.push_log_with_token(log, None);
    // }

    /// Get token
    #[inline]
    pub fn get_token(&self, key: &TokenKey) -> Option<&ExtToken> { self.tokens.get(key) }

    /// Get next log ID
    #[inline]
    pub fn next_log_id(&self) -> u64 { self.logs.back().map_or(1, |log| log.id + 1) }

    /// Find and update log with specified ID
    #[inline]
    pub fn update_log<F>(&mut self, id: u64, f: F)
    where F: FnOnce(&mut RequestLog) {
        if let Some(log) = self.logs.iter_mut().rev().find(|log| log.id == id) {
            f(log)
        }
    }

    // /// Remove log with specified ID
    // #[inline]
    // pub fn remove_log(&mut self, id: u64) -> bool {
    //     if let Some(pos) = self.logs.iter().position(|log| log.id == id) {
    //         if let Some(removed_log) = self.logs.remove(pos) {
    //             // Decrement token reference count of removed log
    //             let token_key = removed_log.token_key();
    //             self.decrement_token_ref(token_key);
    //             return true;
    //         }
    //     }
    //     false
    // }

    /// Check whether logging is enabled
    #[inline]
    pub fn is_enabled(&self) -> bool { self.logs_limit.should_log() }

    /// Get error log count
    #[inline]
    pub fn error_count(&self) -> u64 {
        self.logs.iter().filter(|log| log.status as u8 != 1).count() as u64
    }

    /// Get total log count
    #[inline]
    pub fn total_count(&self) -> u64 { self.logs.len() as u64 }

    // /// Get total token count
    // #[inline]
    // pub fn token_count(&self) -> u64 {
    //     self.tokens.len() as u64
    // }

    // /// Get token reference count statistics
    // #[inline]
    // pub fn token_ref_stats(&self) -> Vec<(TokenKey, usize)> {
    //     self.token_ref_counts
    //         .iter()
    //         .map(|(&k, &v)| (k, v))
    //         .collect()
    // }

    // /// Clear all logs and tokens
    // #[inline]
    // pub fn clear(&mut self) {
    //     self.logs.clear();
    //     self.tokens.clear();
    //     self.token_ref_counts.clear();
    // }

    // /// Clear logs, automatically clean up unused tokens
    // #[inline]
    // pub fn clear_logs(&mut self) {
    //     self.logs.clear();
    //     self.tokens.clear();
    //     self.token_ref_counts.clear();
    // }

    // /// Manually clean up unused tokens
    // #[inline(never)]
    // pub fn cleanup_unused_tokens(&mut self) {
    //     self.rebuild_token_ref_counts();
    // }

    /// Find log by ID
    #[inline]
    pub fn find_log(&self, id: u64) -> Option<&RequestLog> {
        self.logs.iter().rev().find(|log| log.id == id)
    }

    // /// Find mutable log by ID
    // #[inline]
    // pub fn find_log_mut(&mut self, id: u64) -> Option<&mut RequestLog> {
    //     self.logs.iter_mut().rev().find(|log| log.id == id)
    // }

    // /// Iterator over logs and corresponding tokens
    // #[inline]
    // pub fn logs_with_tokens(&self) -> impl Iterator<Item = (&RequestLog, &ExtToken)> {
    //     self.logs.iter().filter_map(|log| {
    //         self.get_token(&log.token_info.key)
    //             .map(|token| (log, token))
    //     })
    // }
}
