mod queue;

use crate::app::{
    constant::{UNNAMED, UNNAMED_PATTERN},
    lazy::TOKENS_FILE_PATH,
    model::{Alias, ExtToken, TokenInfo, TokenInfoHelper, TokenKey},
};
use alloc::{borrow::Cow, collections::VecDeque};
use memmap2::{Mmap, MmapMut};
pub use queue::{QueueType, TokenHealth, TokenQueue};
use tokio::fs::OpenOptions;

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;

#[derive(Debug)]
pub enum TokenError {
    AliasExists,
    InvalidId,
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TokenError::AliasExists => "Alias already exists",
            TokenError::InvalidId => "Invalid Token ID",
        })
    }
}

impl core::error::Error for TokenError {}

/// High-performance Token manager
///
/// Design features:
/// - **Zero-copy query**: All query methods return references, avoid clone
/// - **Compact storage**: Vec<Option<T>> dense layout, cache friendly
/// - **O(1) operations**: Constant time CRUD through HashMap+Vec
/// - **ID reuse**: FIFO queue manages free IDs, reduce memory fragmentation
///   - Prioritize reusing earliest released IDs, improve cache locality
///   - Vec won't grow infinitely, deleted slots will be reused by new tokens
/// - **Multi-index**: Support ID/alias/TokenKey three query methods
/// - **Lock-free design**: Single-threaded optimization, avoid synchronization overhead
///
/// Data structure invariants:
/// - `tokens`, `id_to_alias` always have same length
/// - id values in `id_map`, `alias_map` are always < tokens.len()
/// - id in `id_map`, `alias_map` pointing to `tokens[id]` must be Some
/// - id in `free_ids` must be < tokens.len() and `tokens[id]` is None
///
/// Performance critical paths use unsafe to eliminate bounds checking
pub struct TokenManager {
    /// Main storage: ID -> TokenInfo, use Option to support empty slots after deletion
    tokens: Vec<Option<TokenInfo>>,
    /// TokenKey -> ID mapping, for lookup by token content
    id_map: HashMap<TokenKey, usize>,
    /// Alias -> ID mapping, for user-friendly lookup
    alias_map: HashMap<Alias, usize>,
    /// ID -> Alias reverse index, maintained in sync with tokens
    id_to_alias: Vec<Option<Alias>>,
    /// Reusable ID queue (FIFO), prioritize reusing earliest released IDs
    free_ids: VecDeque<usize>,
    /// Round-robin token selection queue
    queue: TokenQueue,
}

impl TokenManager {
    #[inline]
    pub fn new(capacity: usize) -> Self {
        let r = ahash::RandomState::new();
        Self {
            tokens: Vec::with_capacity(capacity),
            id_map: HashMap::with_capacity_and_hasher(capacity, r.clone()),
            alias_map: HashMap::with_capacity_and_hasher(capacity, r),
            id_to_alias: Vec::with_capacity(capacity),
            free_ids: VecDeque::with_capacity(capacity / 10), // Assume 10% deletion rate
            queue: TokenQueue::with_capacity(capacity),
        }
    }

    #[inline(never)]
    pub fn add<'a, S: Into<Cow<'a, str>>>(
        &mut self,
        token_info: TokenInfo,
        alias: S,
    ) -> Result<usize, TokenError> {
        // Handle unnamed or conflicting aliases, auto-generate unique alias
        let mut alias: Cow<'_, str> = alias.into();
        if alias == UNNAMED || alias.starts_with(UNNAMED_PATTERN) {
            let id = self.free_ids.front().copied().unwrap_or(self.tokens.len());
            alias = Cow::Owned(generate_unnamed_alias(id));
        }

        if self.alias_map.contains_key(&*alias) {
            return Err(TokenError::AliasExists);
        }

        // ID allocation strategy: prioritize reusing free IDs (FIFO order), otherwise extend vec
        let id = if let Some(reused_id) = self.free_ids.pop_front() {
            reused_id
        } else {
            let new_id = self.tokens.len();
            self.tokens.push(None);
            self.id_to_alias.push(None);
            new_id
        };

        let key = token_info.bundle.primary_token.key();
        self.id_map.insert(key, id);
        self.queue.push(key, id);

        // SAFETY: id is either reused_id (from free_ids, must be <len), or new index just pushed
        unsafe { *self.tokens.get_unchecked_mut(id) = Some(token_info) };

        let alias = Alias::new(alias);
        self.alias_map.insert(alias.clone(), id);

        // SAFETY: same as above, id is valid and id_to_alias syncs length with tokens
        unsafe { *self.id_to_alias.get_unchecked_mut(id) = Some(alias) };

        Ok(id)
    }

    #[cfg(not(feature = "horizon"))]
    /// Hot path: query Token by ID
    #[inline]
    pub fn get_by_id(&self, id: usize) -> Option<&TokenInfo> {
        self.tokens.get(id).and_then(|o| o.as_ref())
    }

    /// Hot path: query Token by alias
    #[inline]
    pub fn get_by_alias(&self, alias: &str) -> Option<&TokenInfo> {
        let &id = self.alias_map.get(alias)?;
        // SAFETY: id in alias_map is maintained by add/remove, guaranteed <tokens.len() and corresponding Some
        Some(unsafe { self.tokens.get_unchecked(id).as_ref().unwrap_unchecked() })
    }

    #[inline(never)]
    pub fn remove(&mut self, id: usize) -> Option<TokenInfo> {
        let token_info = self.tokens.get_mut(id)?.take()?;

        // Clean up all indexes
        let key = token_info.bundle.primary_token.key();
        self.id_map.remove(&key);
        #[cfg(not(feature = "horizon"))]
        self.queue.remove(&queue);
        #[cfg(feature = "horizon")]
        self.queue.remove(&key, &self.tokens);

        // SAFETY: reaching here means id<len and Some, id_to_alias syncs length, must have corresponding alias
        unsafe {
            let alias = self.id_to_alias.get_unchecked_mut(id).take().unwrap_unchecked();
            self.alias_map.remove(&alias);
        }

        // Add ID to end of free queue, waiting for reuse
        self.free_ids.push_back(id);
        Some(token_info)
    }

    #[inline(never)]
    pub fn set_alias<'a, S: Into<Cow<'a, str>>>(
        &mut self,
        id: usize,
        alias: S,
    ) -> Result<(), TokenError> {
        if self.tokens.get(id).is_none_or(Option::is_none) {
            return Err(TokenError::InvalidId);
        }

        let mut alias: Cow<'_, str> = alias.into();
        if alias == UNNAMED || alias.starts_with(UNNAMED_PATTERN) {
            alias = Cow::Owned(generate_unnamed_alias(id));
        }
        if self.alias_map.contains_key(&*alias) {
            return Err(TokenError::AliasExists);
        }

        // SAFETY: already checked id is valid and Some above
        unsafe {
            let old_alias = self.id_to_alias.get_unchecked_mut(id).take().unwrap_unchecked();
            self.alias_map.remove(&old_alias);
        }

        let alias = Alias::new(alias);
        self.alias_map.insert(alias.clone(), id);

        // SAFETY: id is still valid
        unsafe { *self.id_to_alias.get_unchecked_mut(id) = Some(alias) };

        Ok(())
    }

    pub fn tokens(&self) -> &Vec<Option<TokenInfo>> { &self.tokens }

    pub fn tokens_mut(&mut self) -> TokensWriter<'_> {
        TokensWriter { tokens: &mut self.tokens, id_map: &mut self.id_map, queue: &mut self.queue }
    }

    pub fn id_map(&self) -> &HashMap<TokenKey, usize> { &self.id_map }

    pub fn alias_map(&self) -> &HashMap<Alias, usize> { &self.alias_map }

    pub fn id_to_alias(&self) -> &Vec<Option<Alias>> { &self.id_to_alias }

    pub fn select(&self, queue_type: QueueType) -> Option<ExtToken> {
        self.queue.select(queue_type, self)
    }

    #[inline(never)]
    pub fn list(&self) -> Vec<(usize, Alias, TokenInfo)> {
        // SAFETY: enumerate guarantees id<len, filter_map only handles Some branch, id_to_alias maintained in sync
        unsafe {
            self.tokens
                .iter()
                .enumerate()
                .filter_map(|(id, token_opt)| {
                    token_opt.as_ref().map(|token| {
                        let alias = self.id_to_alias.get_unchecked(id).as_ref().unwrap_unchecked();
                        (id, alias.clone(), token.clone())
                    })
                })
                .collect()
        }
    }

    /// Update client key of all tokens, for security refresh
    #[inline(always)]
    pub fn update_client_key(&mut self) {
        for token_info in self.tokens.iter_mut().flatten() {
            token_info.bundle.client_key = super::super::Hash::random();
            token_info.bundle.session_id = uuid::Uuid::new_v4();
        }
    }

    #[inline(never)]
    pub async fn save(&self) -> Result<(), Box<dyn core::error::Error + Send + Sync + 'static>> {
        // SAFETY: enumerate guarantees id<len, filter_map only handles Some, id_to_alias maintained in sync
        let helpers: Vec<TokenInfoHelper> = unsafe {
            self.tokens
                .iter()
                .enumerate()
                .filter_map(|(id, token_opt)| {
                    token_opt.as_ref().map(|token_info| {
                        let alias = self
                            .id_to_alias
                            .get_unchecked(id)
                            .as_ref()
                            .map(|a| a.to_string())
                            .unwrap_unchecked();

                        TokenInfoHelper::new(token_info, alias)
                    })
                })
                .collect()
        };

        let bytes = ::rkyv::to_bytes::<::rkyv::rancor::Error>(&helpers)?;
        if bytes.len() > usize::MAX >> 1 {
            return Err("Token data too large".into());
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*TOKENS_FILE_PATH)
            .await?;
        file.set_len(bytes.len() as u64).await?;

        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }

    #[inline(never)]
    pub async fn load() -> Result<Self, Box<dyn core::error::Error + Send + Sync + 'static>> {
        let file = match OpenOptions::new().read(true).open(&*TOKENS_FILE_PATH).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new(0));
            }
            Err(e) => return Err(Box::new(e)),
        };

        if file.metadata().await?.len() > usize::MAX as u64 {
            return Err("Token file too large".into());
        }

        let mmap = unsafe { Mmap::map(&file)? };
        let helpers = unsafe {
            ::rkyv::from_bytes_unchecked::<Vec<TokenInfoHelper>, ::rkyv::rancor::Error>(&mmap)
        }
        .map_err(|_| "Load tokens failed")?;
        let mut manager = Self::new(helpers.len());

        for helper in helpers {
            let (token_info, alias) = helper.extract();
            let _ = manager.add(token_info, alias)?;
        }

        Ok(manager)
    }
}

pub struct TokensWriter<'w> {
    tokens: &'w mut Vec<Option<TokenInfo>>,
    id_map: &'w mut HashMap<TokenKey, usize>,
    queue: &'w mut TokenQueue,
}

impl<'w> TokensWriter<'w> {
    // SAFETY: caller must guarantee id < tokens.len() and tokens[id].is_some()
    #[inline]
    pub unsafe fn get_unchecked_mut(self, id: usize) -> &'w mut TokenInfo {
        unsafe { self.tokens.get_unchecked_mut(id).as_mut().unwrap_unchecked() }
    }

    // SAFETY: caller must guarantee id < tokens.len() and tokens[id].is_some()
    #[inline]
    pub unsafe fn into_token_writer(self, id: usize) -> TokenWriter<'w> {
        let token =
            unsafe { &mut self.tokens.get_unchecked_mut(id).as_mut().unwrap_unchecked().bundle };
        TokenWriter {
            key: token.primary_token.key(),
            token,
            id_map: self.id_map,
            queue: self.queue,
        }
    }
}

/// Token writer, automatically syncs key changes through Drop
///
/// Use case: when need to modify token's key, use this type to ensure:
/// 1. Automatically update id_map after modification completes
/// 2. Automatically update key in queue after modification completes
/// 3. Prevent index inconsistency caused by forgetting manual sync
pub struct TokenWriter<'w> {
    pub key: TokenKey,
    token: &'w mut ExtToken,
    id_map: &'w mut HashMap<TokenKey, usize>,
    queue: &'w mut TokenQueue,
}

impl<'w> core::ops::Deref for TokenWriter<'w> {
    type Target = &'w mut ExtToken;
    fn deref(&self) -> &Self::Target { &self.token }
}

impl<'w> core::ops::DerefMut for TokenWriter<'w> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.token }
}

impl Drop for TokenWriter<'_> {
    fn drop(&mut self) {
        use core::hint::{assert_unchecked, unreachable_unchecked};
        let key = self.token.primary_token.key();

        // Detect whether key changed, if changed then update all indexes
        if key != self.key {
            // SAFETY: TokenWriter can only be created through into_token_writer, token must exist at that time
            // self.key is the token key at creation time, must be in id_map
            unsafe {
                let i = if let hashbrown::hash_map::EntryRef::Occupied(entry) = self.id_map.entry_ref(&self.key) {
                    entry.remove()
                } else {
                    unreachable_unchecked()
                };
                self.id_map.insert(key, i);
                assert_unchecked(self.queue.set_key(&self.key, key));
            }
        }
    }
}

/// Generate default alias for unnamed token
/// Format: UNNAMED_PATTERN + ID (e.g. "unnamed_42")
#[inline]
fn generate_unnamed_alias(id: usize) -> String {
    // Pre-allocate capacity: pattern length + 6 digits
    // 6 digits can represent 0-999999, covering million-level tokens
    // When exceeding million, String will auto-expand (one extra realloc)
    const CAPACITY: usize = UNNAMED_PATTERN.len() + 6;
    let mut s = String::with_capacity(CAPACITY);
    s.push_str(UNNAMED_PATTERN);

    if id == 0 {
        s.push('0');
    } else {
        let start = s.len();
        let mut n = id;
        // Push digits in reverse order, finally reverse
        while n > 0 {
            s.push((b'0' + (n % 10) as u8) as char);
            n /= 10;
        }
        // SAFETY: only pushed ASCII digits, UTF-8 valid
        unsafe { s[start..].as_bytes_mut().reverse() };
    }

    s
}
