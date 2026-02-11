#![allow(unsafe_op_in_unsafe_fn)]

use super::{Randomness, RawToken, UserId};
use crate::common::utils::{from_base64, to_base64};
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::{
    alloc::Layout,
    hash::Hasher,
    marker::PhantomData,
    mem::SizedTypeProperties as _,
    ptr::{NonNull, copy_nonoverlapping},
    slice::from_raw_parts,
    str::from_utf8_unchecked,
    sync::atomic::{AtomicUsize, Ordering},
};
use manually_init::ManuallyInit;
use scc::HashMap;

/// Unique identification key for token
///
/// Composed of user ID and randomness, used to find corresponding token in global cache
#[derive(
    Debug, PartialEq, Eq, Hash, Clone, Copy, ::rkyv::Archive, ::rkyv::Serialize, ::rkyv::Deserialize,
)]
#[rkyv(derive(PartialEq, Eq, Hash))]
pub struct TokenKey {
    /// User unique identifier
    pub user_id: UserId,
    /// Randomness part, used to ensure token uniqueness
    pub randomness: Randomness,
}

impl TokenKey {
    /// Serialize TokenKey to base64 string
    ///
    /// Format: 24 bytes (16 bytes user_id + 8 bytes randomness) encoded to 32 character base64
    #[allow(clippy::inherent_to_string)]
    #[inline]
    pub fn to_string(self) -> String {
        let mut bytes = [0u8; 24];
        unsafe {
            copy_nonoverlapping(self.user_id.to_bytes().as_ptr(), bytes.as_mut_ptr(), 16);
            copy_nonoverlapping(self.randomness.to_bytes().as_ptr(), bytes.as_mut_ptr().add(16), 8);
        }
        to_base64(&bytes)
    }

    /// Serialize TokenKey to readable string
    ///
    /// Format: `<user_id>-<randomness>`
    #[inline]
    pub fn to_string2(self) -> String {
        let mut buffer = itoa::Buffer::new();
        let mut string = String::with_capacity(60);
        string.push_str(buffer.format(self.user_id.as_u128()));
        string.push('-');
        string.push_str(buffer.format(self.randomness.as_u64()));
        string
    }

    /// Parse TokenKey from string
    ///
    /// Supports two formats:
    /// 1. 32-character base64 encoding
    /// 2. `<user_id>-<randomness>` format
    pub fn from_string(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();

        if bytes.len() > 60 {
            return None;
        }

        // base64 format
        if bytes.len() == 32 {
            let decoded: [u8; 24] = __unwrap!(from_base64(s)?.try_into());
            let user_id = UserId::from_bytes(__unwrap!(decoded[0..16].try_into()));
            let randomness = Randomness::from_bytes(__unwrap!(decoded[16..24].try_into()));
            return Some(Self { user_id, randomness });
        }

        // Separator format
        let mut iter = bytes.iter().enumerate();
        let mut first_num_end = None;
        let mut second_num_start = None;

        // First loop: find first non-digit character
        for (i, b) in iter.by_ref() {
            if !b.is_ascii_digit() {
                first_num_end = Some(i);
                break;
            }
        }

        let first_num_end = first_num_end?;

        // Second loop: continue from last stop position, find next digit character
        for (i, b) in iter {
            if b.is_ascii_digit() {
                second_num_start = Some(i);
                break;
            }
        }

        let second_num_start = second_num_start?;

        let first_part = unsafe { from_utf8_unchecked(bytes.get_unchecked(..first_num_end)) };
        let second_part = unsafe { from_utf8_unchecked(bytes.get_unchecked(second_num_start..)) };

        let user_id_val = first_part.parse().ok()?;
        let randomness_val = second_part.parse().ok()?;

        Some(Self {
            user_id: UserId::from_u128(user_id_val),
            randomness: Randomness::from_u64(randomness_val),
        })
    }
}

/// Internal representation of Token
///
/// # Memory Layout
/// ```text
/// +----------------------+
/// | raw: RawToken        | Raw token data
/// | count: AtomicUsize   | Reference count
/// | string_len: usize    | String length
/// +----------------------+
/// | string data...       | UTF-8 string representation
/// +----------------------+
/// ```
struct TokenInner {
    /// Raw token data
    raw: RawToken,
    /// Atomic reference count
    count: AtomicUsize,
    /// Length of string representation
    string_len: usize,
}

impl TokenInner {
    const STRING_MAX_LEN: usize = {
        let layout = Self::LAYOUT;
        isize::MAX as usize + 1 - layout.align() - layout.size()
    };

    /// Get starting address of string data
    #[inline(always)]
    const unsafe fn string_ptr(&self) -> *const u8 { (self as *const Self).add(1) as *const u8 }

    /// Get string slice
    #[inline(always)]
    const unsafe fn as_str(&self) -> &str {
        let ptr = self.string_ptr();
        let slice = from_raw_parts(ptr, self.string_len);
        from_utf8_unchecked(slice)
    }

    /// Calculate memory layout required to store string of specified length
    fn layout_for_string(string_len: usize) -> Layout {
        if string_len > Self::STRING_MAX_LEN {
            __cold_path!();
            panic!("string is too long");
        }
        unsafe {
            Layout::new::<Self>()
                .extend(Layout::from_size_align_unchecked(string_len, 1))
                .unwrap_unchecked()
                .0
                .pad_to_align()
        }
    }

    /// Write struct and string data at specified memory location
    unsafe fn write_with_string(ptr: NonNull<Self>, raw: RawToken, string: &str) {
        let inner = ptr.as_ptr();

        // Write struct fields
        (*inner).raw = raw;
        (*inner).count = AtomicUsize::new(1);
        (*inner).string_len = string.len();

        // Copy string data
        let string_ptr = (*inner).string_ptr() as *mut u8;
        copy_nonoverlapping(string.as_ptr(), string_ptr, string.len());
    }
}

/// Reference-counted Token, supports global cache reuse
///
/// Token is immutable, thread-safe, and automatically manages cache.
/// Same TokenKey will reuse the same underlying instance.
#[repr(transparent)]
pub struct Token {
    ptr: NonNull<TokenInner>,
    _pd: PhantomData<TokenInner>,
}

// Safety: Token uses atomic reference counting, can be safely passed between threads
unsafe impl Send for Token {}
unsafe impl Sync for Token {}

impl Clone for Token {
    #[inline]
    fn clone(&self) -> Self {
        unsafe {
            let count = self.ptr.as_ref().count.fetch_add(1, Ordering::Relaxed);
            if count > isize::MAX as usize {
                __cold_path!();
                std::process::abort();
            }
        }

        Self { ptr: self.ptr, _pd: PhantomData }
    }
}

/// Thread-safe internal pointer wrapper
#[derive(Clone, Copy)]
#[repr(transparent)]
struct ThreadSafePtr(NonNull<TokenInner>);

unsafe impl Send for ThreadSafePtr {}
unsafe impl Sync for ThreadSafePtr {}

/// Global Token cache pool
static TOKEN_MAP: ManuallyInit<HashMap<TokenKey, ThreadSafePtr, ahash::RandomState>> =
    ManuallyInit::new();

#[inline(always)]
pub fn __init() { TOKEN_MAP.init(HashMap::with_capacity_and_hasher(64, ahash::RandomState::new())) }

impl Token {
    /// Create or reuse Token instance
    ///
    /// If cache already contains same TokenKey and RawToken is same, reuse;
    /// otherwise create new instance (may overwrite old one).
    ///
    /// # Concurrency safety
    /// - Uses read-write lock to protect global cache
    /// - Fast path (read lock): try to reuse existing instance
    /// - Slow path (write lock): create new instance after double check, prevent race condition
    pub fn new(raw: RawToken, string: Option<String>) -> Self {
        use scc::hash_map::RawEntry;

        let key = raw.key();
        let hash;

        // Fast path: try to find in cache and increment reference count
        {
            let cache = TOKEN_MAP.get();
            let builder = cache.raw_entry();
            hash = builder.hash(&key);
            if let RawEntry::Occupied(entry) = builder.from_key_hashed_nocheck_sync(hash, &key) {
                let &ThreadSafePtr(ptr) = entry.get();
                unsafe {
                    let inner = ptr.as_ref();
                    // Verify RawToken whether completely matches (same key does not mean same raw)
                    if inner.raw == raw {
                        let count = inner.count.fetch_add(1, Ordering::Relaxed);
                        // Prevent reference count overflow (theoretically impossible, but as safety check)
                        if count > isize::MAX as usize {
                            __cold_path!();
                            std::process::abort();
                        }
                        return Self { ptr, _pd: PhantomData };
                    } else {
                        __cold_path!();
                        crate::debug!("{} != {}", inner.raw, raw);
                    }
                }
            }
        }

        // Slow path: create new instance (need exclusive access to cache)
        let cache = TOKEN_MAP.get();

        match cache.raw_entry().from_key_hashed_nocheck_sync(hash, &key) {
            RawEntry::Occupied(entry) => {
                // Double check: prevent other threads from creating same token before acquiring write lock
                let &ThreadSafePtr(ptr) = entry.get();
                unsafe {
                    let inner = ptr.as_ref();
                    if inner.raw == raw {
                        let count = inner.count.fetch_add(1, Ordering::Relaxed);
                        if count > isize::MAX as usize {
                            __cold_path!();
                            std::process::abort();
                        }
                        return Self { ptr, _pd: PhantomData };
                    } else {
                        __cold_path!();
                        crate::debug!("{} != {}", inner.raw, raw);
                    }
                }

                Self { ptr, _pd: PhantomData }
            }
            RawEntry::Vacant(entry) => {
                // Allocate and initialize new instance (using custom DST layout)
                let ptr = unsafe {
                    // Prepare string representation (before heap allocation)
                    let string = string.unwrap_or_else(|| raw.to_string());
                    let layout = TokenInner::layout_for_string(string.len());

                    let alloc = alloc(layout) as *mut TokenInner;
                    if alloc.is_null() {
                        handle_alloc_error(layout);
                    }
                    let ptr = NonNull::new_unchecked(alloc);
                    TokenInner::write_with_string(ptr, raw, &string);

                    ptr
                };

                // Insert new instance into cache (holding write lock, ensures thread safety)
                entry.insert(key, ThreadSafePtr(ptr));

                Self { ptr, _pd: PhantomData }
            }
        }
    }

    /// Get raw token data
    #[inline(always)]
    pub const fn raw(&self) -> &RawToken { unsafe { &self.ptr.as_ref().raw } }

    /// Get string representation
    #[inline(always)]
    pub const fn as_str(&self) -> &str { unsafe { self.ptr.as_ref().as_str() } }

    /// Get token key
    #[inline(always)]
    pub const fn key(&self) -> TokenKey { self.raw().key() }

    /// Check whether it is web token
    #[inline(always)]
    pub const fn is_web(&self) -> bool { self.raw().is_web() }

    /// Check whether it is session token
    #[inline(always)]
    pub const fn is_session(&self) -> bool { self.raw().is_session() }
}

impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            let inner = self.ptr.as_ref();

            // Decrement reference count, use Release ordering to ensure all previous modifications are visible to subsequent operations
            if inner.count.fetch_sub(1, Ordering::Release) != 1 {
                // Not the last reference, return directly
                return;
            }

            // Last reference: need to clean up resources
            // Get write lock to protect cache operations, and prevent concurrent new() operations from interfering
            let cache = TOKEN_MAP.get();

            let key = inner.raw.key();
            if let scc::hash_map::RawEntry::Occupied(e) = cache.raw_entry().from_key_sync(&key) {
                // Double check reference count: prevent other threads from incrementing count via new() while waiting for write lock
                // Example:
                //   Thread A: fetch_sub returns 1
                //   Thread B: finds this token in new(), fetch_add increments count
                //   Thread A: gets write lock
                // At this point must recheck, otherwise will incorrectly release memory in use
                if inner.count.load(Ordering::Relaxed) != 0 {
                    // New reference created, cancel release operation
                    return;
                }

                // Confirm this is the last reference, perform cleanup:
                // 1. Remove from cache (prevent subsequent new() from finding already released pointer)
                e.remove();

                // 2. Release heap memory (including TokenInner and inlined string data)
                let layout = TokenInner::layout_for_string(inner.string_len);
                dealloc(self.ptr.cast().as_ptr(), layout);
            }
        }
    }
}

// ===== Trait implementations =====

impl PartialEq for Token {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool { self.ptr == other.ptr }
}

impl Eq for Token {}

impl core::hash::Hash for Token {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) { self.key().hash(state); }
}

impl core::fmt::Display for Token {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.write_str(self.as_str()) }
}

// ===== Serde implementation =====

mod serde_impls {
    use super::*;
    use ::serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for Token {
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
            self.as_str().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Token {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
            let s = String::deserialize(deserializer)?;
            let raw_token = s.parse().map_err(::serde::de::Error::custom)?;
            Ok(Token::new(raw_token, Some(s)))
        }
    }
}
