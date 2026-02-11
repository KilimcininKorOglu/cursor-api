//! Reference-counted immutable string with global string pool reuse
//!
//! # Core Design Philosophy
//!
//! `ArcStr` achieves memory deduplication through a global string pool, strings with same content share the same memory.
//! This significantly reduces memory usage in scenarios with many duplicate strings, while maintaining high-performance string operations.
//!
//! # Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        User API Layer                           │
//! │  ArcStr::new() │ as_str() │ clone() │ Drop │ PartialEq...       │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                      Global String Pool                         │
//! │               HashMap<ThreadSafePtr, ()>                        │
//! │               Double-checked locking + Atomic reference count   │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Underlying Memory Layout                     │
//! │  [hash:u64][count:AtomicUsize][len:usize][string_data...]       │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Performance Characteristics
//!
//! | Operation | Time Complexity | Description |
//! |-----------|-----------------|-------------|
//! | new() - first | O(1) + pool insert | Heap allocation + HashMap insertion |
//! | new() - hit | O(1) | HashMap lookup + atomic increment |
//! | clone() | O(1) | Only atomic increment |
//! | drop() | O(1) | Fast deletion using pre-stored hash |
//! | as_str() | O(1) | Direct memory access |

use core::{
    alloc::Layout,
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{BuildHasherDefault, Hash, Hasher},
    hint,
    marker::PhantomData,
    ptr::NonNull,
    str,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Relaxed, Release},
    },
};
use manually_init::ManuallyInit;
use scc::{Equivalent, HashMap};

// ═══════════════════════════════════════════════════════════════════════════
//                          Layer 1: Public API and Core Interface
// ═══════════════════════════════════════════════════════════════════════════

/// Reference-counted immutable string with global string pool reuse
///
/// # Design Goals
///
/// - **Memory deduplication**: Strings with same content share the same memory address
/// - **Zero-copy clone**: `clone()` only involves atomic increment operation
/// - **Thread-safe**: Supports safe use in multi-threaded environments
/// - **High-performance lookup**: Uses pre-computed hash values to optimize pool lookup
///
/// # Usage Example
///
/// ```rust
/// use interned::ArcStr;
///
/// let s1 = ArcStr::new("hello");
/// let s2 = ArcStr::new("hello");
///
/// // Strings with same content share the same memory
/// assert_eq!(s1.as_ptr(), s2.as_ptr());
/// assert_eq!(s1.ref_count(), 2);
///
/// // Zero-cost string access
/// println!("{}", s1.as_str()); // "hello"
/// ```
///
/// # Memory Safety
///
/// `ArcStr` internally uses atomic reference counting to ensure memory safety, no need to worry about dangling pointers or data races.
/// When the last reference is released, the string will be automatically removed from the global pool and memory freed.
#[repr(transparent)]
pub struct ArcStr {
    /// Non-null pointer to `ArcStrInner`
    ///
    /// # Invariants
    /// - Pointer is always valid, points to correctly initialized `ArcStrInner`
    /// - Reference count is at least 1 (before drop starts)
    /// - String data is always valid UTF-8
    ptr: NonNull<ArcStrInner>,

    /// Zero-sized marker, ensures `ArcStr` has ownership semantics
    _marker: PhantomData<ArcStrInner>,
}

// SAFETY: ArcStr uses atomic reference counting, can be safely passed and accessed across threads
unsafe impl Send for ArcStr {}
unsafe impl Sync for ArcStr {}

impl ArcStr {
    /// Create or reuse string instance
    ///
    /// If a string with the same content already exists in the global pool, reuses existing instance and increments reference count;
    /// otherwise creates new instance and adds to pool.
    ///
    /// # Concurrency Strategy
    ///
    /// Uses double-checked locking pattern to balance performance and correctness:
    /// 1. **Read lock fast path**: In most cases, only read lock is needed to find existing string
    /// 2. **Write lock creation path**: Only acquires write lock when actually need to create new string
    /// 3. **Double verification**: Check again after acquiring write lock to prevent concurrent creation of duplicates
    ///
    /// # Performance Characteristics
    ///
    /// - **Pool hit**: O(1) `HashMap` lookup + atomic increment
    /// - **Pool miss**: O(1) memory allocation + O(1) `HashMap` insertion
    /// - **Hash calculation**: Uses ahash's high-performance hash algorithm
    ///
    /// # Examples
    ///
    /// ```rust
    /// let s1 = ArcStr::new("shared_content");
    /// let s2 = ArcStr::new("shared_content"); // Reuses s1's memory
    /// assert_eq!(s1.as_ptr(), s2.as_ptr());
    /// ```
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        let string = s.as_ref();

        // Phase 0: Pre-compute content hash
        //
        // This hash value will be used multiple times throughout the lifecycle:
        // - As HashMap key during pool lookup
        // - Stored in ArcStrInner for subsequent drop optimization
        let hash = CONTENT_HASHER.hash_one(string);

        // ===== Phase 1: Read lock fast path =====
        // In most cases string is already in pool, this is the most common path
        {
            let pool = ARC_STR_POOL.get();
            if let Some(existing) = Self::try_find_existing(pool, hash, string) {
                return existing;
            }
            // Read lock automatically released
        }

        // ===== Phase 2: Write lock creation path =====
        // Entering here means need to create new string instance
        let pool = ARC_STR_POOL.get();

        match pool.raw_entry().from_key_hashed_nocheck_sync(hash, string) {
            scc::hash_map::RawEntry::Occupied(entry) => {
                // Double check: While acquiring write lock, other threads may have created the same string
                let ptr = entry.key().0;

                // Found matching string, increment its reference count
                // SAFETY: Pointers in pool are always valid, and reference count operations are atomic
                unsafe { ptr.as_ref().inc_strong() };

                Self { ptr, _marker: PhantomData }
            }
            scc::hash_map::RawEntry::Vacant(entry) => {
                // Confirm need to create new instance: allocate memory and initialize
                let layout = ArcStrInner::layout_for_string(string.len());

                // SAFETY: layout_for_string ensures layout is valid and size is reasonable
                let ptr = unsafe {
                    let alloc: *mut ArcStrInner = alloc::alloc::alloc(layout).cast();

                    if alloc.is_null() {
                        hint::cold_path();
                        alloc::alloc::handle_alloc_error(layout);
                    }

                    let ptr = NonNull::new_unchecked(alloc);
                    ArcStrInner::write_with_string(ptr, string, hash);
                    ptr
                };

                // Add newly created string to global pool
                // Use from_key_hashed_nocheck to avoid recalculating hash
                entry.insert(ThreadSafePtr(ptr), ());

                Self { ptr, _marker: PhantomData }
            }
        }
    }

    /// Get string slice (zero-cost operation)
    ///
    /// Directly access underlying string data, no extra overhead.
    ///
    /// # Performance
    ///
    /// This is a `const fn`, offset can be determined at compile time,
    /// only needs one memory dereference at runtime.
    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> &str {
        // SAFETY: ptr always points to valid ArcStrInner during ArcStr's lifetime,
        // and string data is guaranteed to be valid UTF-8
        unsafe { self.ptr.as_ref().as_str() }
    }

    /// Get byte slice of string
    ///
    /// Provides direct access to underlying byte data.
    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        // SAFETY: ptr always points to valid ArcStrInner
        unsafe { self.ptr.as_ref().as_bytes() }
    }

    /// Get string length (in bytes)
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        // SAFETY: ptr always points to valid ArcStrInner
        unsafe { self.ptr.as_ref().string_len }
    }

    /// Check if string is empty
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool { self.len() == 0 }

    /// Get current reference count
    ///
    /// Note: Due to concurrent access, returned value may change immediately after return.
    /// This method is mainly used for debugging and testing.
    #[must_use]
    #[inline]
    pub fn ref_count(&self) -> usize {
        // SAFETY: ptr always points to valid ArcStrInner
        unsafe { self.ptr.as_ref().strong_count() }
    }

    /// Get memory address of string data (for debugging and testing)
    ///
    /// Returns starting address of string content, can be used to verify if strings share memory.
    #[must_use]
    #[inline]
    pub const fn as_ptr(&self) -> *const u8 {
        // SAFETY: ptr always points to valid ArcStrInner
        unsafe { self.ptr.as_ref().string_ptr() }
    }

    /// Internal helper function: Find existing string in pool
    ///
    /// This function is extracted to eliminate duplicate code in read lock path and write lock path.
    /// Uses hashbrown's optimized API to avoid recalculating hash.
    ///
    /// # Parameters
    ///
    /// - `pool`: Reference to string pool
    /// - `hash`: Pre-computed string hash value
    /// - `string`: String content to find
    ///
    /// # Return Value
    ///
    /// If matching string is found, returns `ArcStr` with incremented reference count; otherwise returns `None`.
    #[must_use]
    #[inline]
    fn try_find_existing(pool: &PtrMap, hash: u64, string: &str) -> Option<Self> {
        // Use hashbrown's from_key_hashed_nocheck API
        // This utilizes Equivalent trait for efficient comparison
        use scc::hash_map::RawEntry;
        let RawEntry::Occupied(entry) = pool.raw_entry().from_key_hashed_nocheck_sync(hash, string)
        else {
            return None;
        };
        let ThreadSafePtr(ptr) = *entry.key();

        // Found matching string, increment its reference count
        // SAFETY: Pointers in pool are always valid, and reference count operations are atomic
        unsafe { ptr.as_ref().inc_strong() };

        Some(Self { ptr, _marker: PhantomData })
    }
}

impl Clone for ArcStr {
    /// Clone string reference (only increments reference count)
    ///
    /// This is an extremely lightweight operation, only involves one atomic increment.
    /// Does not copy string content, new `ArcStr` shares the same underlying memory with original instance.
    ///
    /// # Performance
    ///
    /// Time complexity: O(1) - single atomic operation
    /// Space complexity: O(1) - no extra memory allocation
    #[inline]
    fn clone(&self) -> Self {
        // SAFETY: ptr is valid during current ArcStr's lifetime
        unsafe { self.ptr.as_ref().inc_strong() }
        Self { ptr: self.ptr, _marker: PhantomData }
    }
}

impl Drop for ArcStr {
    /// Release string reference
    ///
    /// Decrements reference count, if this is the last reference, removes from global pool and frees memory.
    ///
    /// # Concurrency Handling
    ///
    /// Since multiple threads may release references to the same string simultaneously, careful double-checking is used here:
    /// 1. Atomically decrement reference count
    /// 2. If count becomes 0, acquire pool's write lock
    /// 3. Check reference count again (prevent concurrent clone operations)
    /// 4. After confirmation, remove from pool and free memory
    ///
    /// # Performance Optimization
    ///
    /// Uses pre-stored hash value for O(1) pool lookup and deletion, avoids recalculating hash.
    fn drop(&mut self) {
        // SAFETY: ptr is still valid when drop starts
        unsafe {
            let inner = self.ptr.as_ref();

            // Atomically decrement reference count
            if !inner.dec_strong() {
                // Not the last reference, return directly
                return;
            }

            // This is the last reference, need to cleanup resources
            let pool = ARC_STR_POOL.get();
            // Use pointer equality comparison, this is absolute O(1) operation
            let entry =
                pool.raw_entry().from_key_hashed_nocheck_sync(inner.hash, &ThreadSafePtr(self.ptr));

            if let scc::hash_map::RawEntry::Occupied(e) = entry {
                // Double check reference count
                // While acquiring write lock, other threads may have cloned this string
                if inner.strong_count() != 0 {
                    return;
                }

                // Confirm this is the last reference, execute cleanup
                e.remove();

                // Free underlying memory
                let layout = ArcStrInner::layout_for_string_unchecked(inner.string_len);
                alloc::alloc::dealloc(self.ptr.cast().as_ptr(), layout);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//                          Layer 2: Standard Library Integration
// ═══════════════════════════════════════════════════════════════════════════

// # Basic Trait Implementations
//
// These implementations ensure `ArcStr` can seamlessly integrate with Rust's standard library types,
// providing intuitive comparison, formatting and access interfaces.

impl PartialEq for ArcStr {
    /// Fast equality comparison based on pointer
    ///
    /// # Optimization Principle
    ///
    /// Since string pool guarantees strings with same content have the same memory address,
    /// we can quickly determine string equality by comparing pointers,
    /// avoiding byte-by-byte content comparison.
    ///
    /// This makes equality comparison an O(1) operation instead of O(n).
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.ptr == other.ptr }
}

impl Eq for ArcStr {}

impl PartialOrd for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for ArcStr {
    /// Lexicographic comparison based on string content
    ///
    /// Note: Must compare content here, not pointers, because pointer addresses are unrelated to lexicographic order.
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering { self.as_str().cmp(other.as_str()) }
}

impl Hash for ArcStr {
    /// Hash based on string content
    ///
    /// Although pre-computed hash value is stored internally, recalculate here to ensure
    /// consistency with hash values of `&str` and `String`.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) { self.as_str().hash(state) }
}

impl fmt::Display for ArcStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { fmt::Display::fmt(self.as_str(), f) }
}

impl fmt::Debug for ArcStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(self.as_str(), f) }
}

impl const AsRef<str> for ArcStr {
    #[inline]
    fn as_ref(&self) -> &str { self.as_str() }
}

impl const AsRef<[u8]> for ArcStr {
    #[inline]
    fn as_ref(&self) -> &[u8] { self.as_bytes() }
}

impl const Borrow<str> for ArcStr {
    #[inline]
    fn borrow(&self) -> &str { self.as_str() }
}

impl const core::ops::Deref for ArcStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target { self.as_str() }
}

// # Interoperability with Other String Types
//
// These implementations allow `ArcStr` to directly compare with various string types
// in the Rust ecosystem, providing good development experience.

impl const PartialEq<str> for ArcStr {
    #[inline]
    fn eq(&self, other: &str) -> bool { self.as_str() == other }
}

impl const PartialEq<&str> for ArcStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool { self.as_str() == *other }
}

impl const PartialEq<ArcStr> for str {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { self == other.as_str() }
}

impl const PartialEq<ArcStr> for &str {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { *self == other.as_str() }
}

impl const PartialEq<String> for ArcStr {
    #[inline]
    fn eq(&self, other: &String) -> bool { self.as_str() == other.as_str() }
}

impl const PartialEq<ArcStr> for String {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { self.as_str() == other.as_str() }
}

impl PartialOrd<str> for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> { Some(self.as_str().cmp(other)) }
}

impl PartialOrd<String> for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<Ordering> {
        Some(self.as_str().cmp(other.as_str()))
    }
}

// # Type Conversion Implementations
//
// Provides convenient conversions from various string types to `ArcStr`,
// as well as conversions from `ArcStr` to other types.

impl<'a> From<&'a str> for ArcStr {
    #[inline]
    fn from(s: &'a str) -> Self { Self::new(s) }
}

impl<'a> From<&'a String> for ArcStr {
    #[inline]
    fn from(s: &'a String) -> Self { Self::new(s) }
}

impl From<String> for ArcStr {
    #[inline]
    fn from(s: String) -> Self { Self::new(s) }
}

impl<'a> From<alloc::borrow::Cow<'a, str>> for ArcStr {
    #[inline]
    fn from(cow: alloc::borrow::Cow<'a, str>) -> Self { Self::new(cow) }
}

impl From<alloc::boxed::Box<str>> for ArcStr {
    #[inline]
    fn from(s: alloc::boxed::Box<str>) -> Self { Self::new(s) }
}

impl From<ArcStr> for String {
    #[inline]
    fn from(s: ArcStr) -> Self { s.as_str().to_owned() }
}

impl From<ArcStr> for alloc::boxed::Box<str> {
    #[inline]
    fn from(s: ArcStr) -> Self { s.as_str().into() }
}

impl str::FromStr for ArcStr {
    type Err = core::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::new(s)) }
}

/// # Serde Serialization Support
///
/// Conditionally compiled Serde support, allows `ArcStr` to participate in serialization/deserialization.
/// Outputs string content when serializing, re-establishes pooled reference when deserializing.
#[cfg(feature = "serde")]
mod serde_impls {
    use super::ArcStr;
    use serde_core::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for ArcStr {
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
            self.as_str().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for ArcStr {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
            String::deserialize(deserializer).map(ArcStr::new)
        }
    }
}

/// # Rkyv Serialization Support
///
/// Conditionally compiled Rkyv support, allows `ArcStr` to participate in serialization/deserialization.
/// Outputs string content when serializing, re-establishes pooled reference when deserializing.
#[cfg(feature = "rkyv")]
mod rkyv_impls {
    use super::ArcStr;
    use core::cmp::Ordering;
    use rkyv::{
        Archive, Deserialize, DeserializeUnsized, Place, Serialize, SerializeUnsized,
        rancor::{Fallible, Source},
        string::{ArchivedString, StringResolver},
    };

    impl Archive for ArcStr {
        type Archived = ArchivedString;
        type Resolver = StringResolver;

        #[inline]
        fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
            ArchivedString::resolve_from_str(self.as_str(), resolver, out);
        }
    }

    impl<S: Fallible + ?Sized> Serialize<S> for ArcStr
    where
        S::Error: Source,
        str: SerializeUnsized<S>,
    {
        fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
            ArchivedString::serialize_from_str(self.as_str(), serializer)
        }
    }

    impl<D: Fallible + ?Sized> Deserialize<ArcStr, D> for ArchivedString
    where str: DeserializeUnsized<str, D>
    {
        fn deserialize(&self, _: &mut D) -> Result<ArcStr, D::Error> {
            Ok(ArcStr::new(self.as_str()))
        }
    }

    impl PartialEq<ArcStr> for ArchivedString {
        #[inline]
        fn eq(&self, other: &ArcStr) -> bool { PartialEq::eq(self.as_str(), other.as_str()) }
    }

    impl PartialEq<ArchivedString> for ArcStr {
        #[inline]
        fn eq(&self, other: &ArchivedString) -> bool {
            PartialEq::eq(other.as_str(), self.as_str())
        }
    }

    impl PartialOrd<ArcStr> for ArchivedString {
        #[inline]
        fn partial_cmp(&self, other: &ArcStr) -> Option<Ordering> {
            self.as_str().partial_cmp(other.as_str())
        }
    }

    impl PartialOrd<ArchivedString> for ArcStr {
        #[inline]
        fn partial_cmp(&self, other: &ArchivedString) -> Option<Ordering> {
            self.as_str().partial_cmp(other.as_str())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//                          Layer 3: Core Implementation Mechanism
// ═══════════════════════════════════════════════════════════════════════════

// # Memory Layout and Data Structure Design
//
// This module contains the underlying data structure definitions and memory layout management for `ArcStr`.
// Understanding this part helps to deeply understand the principles of performance optimization.

/// Internal representation of string content (DST header)
///
/// # Memory Layout Design
///
/// Uses `#[repr(C)]` to ensure stable memory layout, string data follows immediately after the struct:
///
/// ```text
/// 64-bit system memory layout:
/// ┌────────────────────┬──────────────────────────────────────────┐
/// │ Field              │ Size and Alignment                       │
/// ├────────────────────┼──────────────────────────────────────────┤
/// │ hash: u64          │ 8 bytes, 8-byte aligned (offset: 0)      │
/// │ count: AtomicUsize │ 8 bytes, 8-byte aligned (offset: 8)      │
/// │ string_len: usize  │ 8 bytes, 8-byte aligned (offset: 16)     │
/// ├────────────────────┼──────────────────────────────────────────┤
/// │ [string data]      │ string_len bytes, 1-byte aligned (offset: 24) │
/// └────────────────────┴──────────────────────────────────────────┘
/// Total header size: 24 bytes
///
/// 32-bit system memory layout:
/// ┌────────────────────┬──────────────────────────────────────────┐
/// │ hash: u64          │ 8 bytes, 8-byte aligned (offset: 0)      │
/// │ count: AtomicUsize │ 4 bytes, 4-byte aligned (offset: 8)      │
/// │ string_len: usize  │ 4 bytes, 4-byte aligned (offset: 12)     │
/// ├────────────────────┼──────────────────────────────────────────┤
/// │ [string data]      │ string_len bytes, 1-byte aligned (offset: 16) │
/// └────────────────────┴──────────────────────────────────────────┘
/// Total header size: 16 bytes
/// ```
///
/// # Design Considerations
///
/// 1. **Hash value first**: Placing `hash` first ensures correct alignment on 32-bit systems
/// 2. **Atomic counter**: Uses `AtomicUsize` to guarantee thread-safe reference counting
/// 3. **Length caching**: Pre-stores string length to avoid repeated calculation
/// 4. **DST layout**: String data directly follows the struct, reducing indirect access
#[repr(C)]
struct ArcStrInner {
    /// Pre-computed content hash value
    ///
    /// This hash value is reused in multiple scenarios:
    /// - Global pool's `HashMap` key
    /// - Fast lookup during `Drop`
    /// - Performance optimization to avoid repeated hash calculation
    hash: u64,

    /// Atomic reference count
    ///
    /// Uses native atomic type to ensure optimal performance.
    /// Count range: [1, `isize::MAX`], triggers abort when exceeded.
    count: AtomicUsize,

    /// Byte length of string (UTF-8 encoded)
    ///
    /// Pre-stores length to avoid scanning string on each access.
    /// Does not include NUL terminator.
    string_len: usize,
    // Note: String data follows immediately after this struct,
    // layout calculated by layout_for_string() ensures correct memory allocation
}

impl ArcStrInner {
    /// Upper limit for string length
    ///
    /// Formula: `isize::MAX - sizeof(ArcStrInner)`
    /// This ensures total allocation size won't overflow signed integer range.
    const MAX_LEN: usize = isize::MAX as usize - core::mem::size_of::<Self>();

    /// Get starting address of string data
    ///
    /// # Safety
    ///
    /// - `self` must be a pointer to valid `ArcStrInner`
    /// - Must ensure string data has been correctly initialized
    /// - Caller is responsible for ensuring returned pointer remains valid during use
    #[must_use]
    #[inline]
    const unsafe fn string_ptr(&self) -> *const u8 {
        // SAFETY: repr(C) guarantees string data is at fixed offset after struct end
        core::ptr::from_ref(self).add(1).cast()
    }

    /// Get byte slice of string
    ///
    /// # Safety
    ///
    /// - `self` must be a pointer to valid `ArcStrInner`
    /// - String data must have been correctly initialized
    /// - `string_len` must accurately reflect actual string length
    /// - String data must remain valid during returned slice's lifetime
    #[must_use]
    #[inline]
    const unsafe fn as_bytes(&self) -> &[u8] {
        let ptr = self.string_ptr();
        // SAFETY: Caller guarantees ptr points to valid string_len bytes of data
        core::slice::from_raw_parts(ptr, self.string_len)
    }

    /// Get string slice reference
    ///
    /// # Safety
    ///
    /// - `self` must be a pointer to valid `ArcStrInner`
    /// - String data must be valid UTF-8 encoding
    /// - `string_len` must accurately reflect actual string length
    /// - String data must remain valid during returned slice's lifetime
    #[must_use]
    #[inline]
    const unsafe fn as_str(&self) -> &str {
        // SAFETY: Caller guarantees string data is valid UTF-8
        core::str::from_utf8_unchecked(self.as_bytes())
    }

    /// Calculate memory layout needed to store string of specified length
    ///
    /// This function calculates correct memory size and alignment requirements,
    /// ensuring both struct and string data can be correctly aligned.
    ///
    /// # Panics
    ///
    /// If `string_len > Self::MAX_LEN`, function will panic.
    /// This is to prevent integer overflow and invalid memory layout.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let layout = ArcStrInner::layout_for_string(5); // "hello"
    /// assert!(layout.size() >= 24 + 5); // 64-bit system
    /// ```
    fn layout_for_string(string_len: usize) -> Layout {
        if string_len > Self::MAX_LEN {
            hint::cold_path();
            panic!("String too long: {} bytes (max supported: {})", string_len, Self::MAX_LEN);
        }

        // SAFETY: Length check passed, layout calculation is safe
        unsafe { Self::layout_for_string_unchecked(string_len) }
    }

    /// Calculate memory layout needed to store string of specified length (without length check)
    ///
    /// # Safety
    ///
    /// Caller must guarantee `string_len <= Self::MAX_LEN`
    const unsafe fn layout_for_string_unchecked(string_len: usize) -> Layout {
        let header = Layout::new::<Self>();
        let string_data = Layout::from_size_align_unchecked(string_len, 1);
        // SAFETY: Length has been checked, layout calculation won't overflow
        let (combined, _offset) = header.extend(string_data).unwrap_unchecked();
        combined.pad_to_align()
    }

    /// Initialize `ArcStrInner` at specified memory location and write string data
    ///
    /// This is a low-level function responsible for setting up complete DST structure:
    /// 1. Initialize header fields
    /// 2. Copy string data to adjacent memory
    ///
    /// # Safety
    ///
    /// - `ptr` must point to valid memory allocated via `layout_for_string(string.len())`
    /// - Memory must be correctly aligned and large enough
    /// - `string` must be valid UTF-8 string
    /// - Caller is responsible for eventually freeing this memory
    /// - After calling this function, caller must ensure reference count is correctly managed
    const unsafe fn write_with_string(ptr: NonNull<Self>, string: &str, hash: u64) {
        let inner = ptr.as_ptr();

        // Step 1: Initialize header struct
        // SAFETY: ptr points to valid allocated memory, large enough to hold Self
        core::ptr::write(
            inner,
            Self { hash, count: AtomicUsize::new(1), string_len: string.len() },
        );

        // Step 2: Copy string data to memory immediately after header
        // SAFETY:
        // - Address calculated by string_ptr() is within allocated memory range
        // - string.len() matches length used during allocation
        // - string.as_ptr() points to valid UTF-8 data
        let string_ptr = (*inner).string_ptr().cast_mut();
        core::ptr::copy_nonoverlapping(string.as_ptr(), string_ptr, string.len());
    }

    /// Atomically increment reference count
    ///
    /// # Overflow Handling
    ///
    /// If reference count exceeds `isize::MAX`, function will immediately abort program.
    /// This is an extreme case that almost never happens in normal use.
    ///
    /// # Safety
    ///
    /// - `self` must point to valid `ArcStrInner`
    /// - Current reference count must be at least 1 (i.e., valid reference exists)
    #[inline]
    unsafe fn inc_strong(&self) {
        let old_count = self.count.fetch_add(1, Relaxed);

        // Prevent reference count overflow - this is a safety check
        if old_count > isize::MAX as usize {
            hint::cold_path();
            // Overflow is a memory safety issue, must immediately terminate program
            core::intrinsics::abort();
        }
    }

    /// Atomically decrement reference count
    ///
    /// Uses Release memory ordering to ensure all previous modifications are visible to subsequent operations.
    /// This is crucial for safe memory reclamation.
    ///
    /// # Safety
    ///
    /// - `self` must point to valid `ArcStrInner`
    /// - Current reference count must be at least 1
    ///
    /// # Return Value
    ///
    /// If this is the last reference (count becomes 0), returns `true`; otherwise returns `false`.
    #[inline]
    unsafe fn dec_strong(&self) -> bool {
        // Release ordering: Ensures all previous modifications are visible to subsequent memory release operations
        self.count.fetch_sub(1, Release) == 1
    }

    /// Get snapshot of current reference count
    ///
    /// Note: Due to concurrency, returned value may be outdated immediately after return.
    /// This method is mainly used for debugging and testing purposes.
    #[inline]
    fn strong_count(&self) -> usize { self.count.load(Relaxed) }
}

// # Global String Pool Design and Implementation
//
// The global pool is the core of the entire system, responsible for deduplication and lifecycle management.

/// Thread-safe internal pointer wrapper
///
/// This type solves the problem of storing `NonNull<ArcStrInner>` in `HashMap`:
/// - Provides necessary trait implementations (`Hash`, `PartialEq`, `Send`, `Sync`)
/// - Encapsulates pointer's thread-safe semantics
/// - Supports content-based lookup (via Equivalent trait)
///
/// # Thread Safety
///
/// Although wrapping a raw pointer, `ThreadSafePtr` is thread-safe because:
/// - The pointed `ArcStrInner` is immutable (except for atomic reference count)
/// - Reference counting uses atomic operations
/// - Lifecycle is managed by global pool, ensuring pointer validity
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct ThreadSafePtr(NonNull<ArcStrInner>);

// SAFETY: ArcStrInner content is immutable and uses atomic reference counting, can be safely accessed across threads
unsafe impl Send for ThreadSafePtr {}
unsafe impl Sync for ThreadSafePtr {}

impl const core::ops::Deref for ThreadSafePtr {
    type Target = NonNull<ArcStrInner>;

    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl Hash for ThreadSafePtr {
    /// Use pre-stored hash value
    ///
    /// This is a key optimization: We don't recalculate string content hash,
    /// but directly use pre-computed value stored in `ArcStrInner`.
    /// Used with `IdentityHasher` to avoid any extra hash calculation.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        // SAFETY: ThreadSafePtr guarantees pointer is always valid during pool's lifetime
        unsafe {
            let inner = self.0.as_ref();
            state.write_u64(inner.hash);
        }
    }
}

impl PartialEq for ThreadSafePtr {
    /// Comparison based on pointer equality
    ///
    /// This is the core of pool deduplication mechanism: Only pointers pointing to same memory address
    /// are considered "same" pool entries. Strings with same content but different addresses
    /// should not exist simultaneously in the pool.
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}

impl Eq for ThreadSafePtr {}

impl Equivalent<ThreadSafePtr> for str {
    /// Support lookup with `&str` in `HashSet<ThreadSafePtr>`
    ///
    /// This implementation allows us to use string content to find entries in pool,
    /// without needing to construct a `ThreadSafePtr` first.
    ///
    /// # Performance Optimization
    ///
    /// First compare string length (single usize comparison), only when lengths are equal
    /// do content comparison (potential memcmp). This avoids overhead of constructing
    /// fat pointer when lengths are unequal.
    #[inline]
    fn equivalent(&self, key: &ThreadSafePtr) -> bool {
        // SAFETY: ThreadSafePtr in pool guarantees pointing to valid ArcStrInner
        unsafe {
            let inner = key.0.as_ref();

            // Optimization: Compare length first (O(1)), avoid unnecessary content comparison
            if inner.string_len != self.len() {
                return false;
            }

            // When lengths are equal, do content comparison
            inner.as_str() == self
        }
    }
}

// # Hash Algorithm Selection and Pool Type Definition

/// Pass-through hasher, used internally by global pool
///
/// Since we pre-store hash value in `ArcStrInner`, pool's internal `HashMap`
/// doesn't need to recalculate hash. `IdentityHasher` directly passes through u64 value.
///
/// # How It Works
///
/// 1. `ThreadSafePtr::hash()` calls `hasher.write_u64(stored_hash)`
/// 2. `IdentityHasher::write_u64()` directly stores this value
/// 3. `IdentityHasher::finish()` returns stored value
/// 4. `HashMap` uses this hash value for bucket allocation and lookup
///
/// This avoids repeated hash calculation, minimizing hash overhead for pool operations.
#[derive(Default, Clone, Copy)]
struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("IdentityHasher usage error");
    }

    #[inline]
    fn write_u64(&mut self, id: u64) { self.0 = id }

    #[inline]
    fn finish(&self) -> u64 { self.0 }
}

/// Pool type alias, simplifies code
type PoolHasher = BuildHasherDefault<IdentityHasher>;
type PtrMap = HashMap<ThreadSafePtr, (), PoolHasher>;

/// Content hash calculator
///
/// Uses ahash's high-performance random hash algorithm to calculate string content hash value.
/// This hash value will be stored in `ArcStrInner`, used throughout its lifetime.
///
/// # Why ahash?
///
/// - High performance: Faster than standard library's `DefaultHasher`
/// - Security: Resistant to hash flooding attacks
/// - Quality: Uniform distribution, reduces hash collisions
static CONTENT_HASHER: ManuallyInit<ahash::RandomState> = ManuallyInit::new();

/// Global string pool
///
/// Uses `HashMap` to implement high-concurrency string pool:
/// - **Read lock**: Multiple threads can simultaneously lookup existing strings
/// - **Write lock**: Creating new strings requires exclusive access
/// - **Capacity pre-allocation**: Avoids frequent resizing in early stages
///
/// # Concurrency Pattern
///
/// ```text
/// Concurrent reads (common case):
/// Thread A: read_lock() -> lookup "hello" -> found -> return
/// Thread B: read_lock() -> lookup "world" -> found -> return
/// Thread C: read_lock() -> lookup "hello" -> found -> return
///
/// Concurrent writes (occasional):
/// Thread D: write_lock() -> lookup "new" -> not found -> create -> insert -> return
/// ```
static ARC_STR_POOL: ManuallyInit<PtrMap> = ManuallyInit::new();

/// Initialize global string pool
///
/// This function must be called before using `ArcStr`, usually done at program startup.
/// Initialization process includes:
/// 1. Create content hash calculator
/// 2. Create empty string pool (pre-allocate capacity for 128 entries)
///
/// # Thread Safety
///
/// Although this function itself is not thread-safe, it should be called once
/// in a single-threaded environment (like at the start of main function or during static initialization).
#[inline(always)]
// Called only once
#[allow(clippy::inline_always)]
pub(crate) fn __init() {
    CONTENT_HASHER.init(ahash::RandomState::new());
    ARC_STR_POOL.init(PtrMap::with_capacity_and_hasher(128, PoolHasher::default()));
}

// ═══════════════════════════════════════════════════════════════════════════
//                          Layer 4: Performance Optimization Implementation
// ═══════════════════════════════════════════════════════════════════════════

/// # Memory Management Optimization Strategy
///
/// This module contains various low-level performance optimization implementations,
/// including memory layout calculation, allocation strategy and concurrency optimization.

// (Performance-critical internal function implementations are already reflected in the code above)

/// # Concurrency Control Optimization
///
/// Detailed implementation analysis of double-checked locking pattern:
///
/// ```text
/// Timeline example:
/// T1: Thread A calls ArcStr::new("test")
/// T2: Thread A acquires read lock, searches pool, not found
/// T3: Thread A releases read lock
/// T4: Thread B calls ArcStr::new("test")
/// T5: Thread B acquires read lock, searches pool, not found
/// T6: Thread B releases read lock
/// T7: Thread A acquires write lock
/// T8: Thread A searches again (double check), confirms not found
/// T9: Thread A creates new instance, inserts into pool
/// T10: Thread A releases write lock
/// T11: Thread B waiting for write lock...
/// T12: Thread B acquires write lock
/// T13: Thread B searches again (double check), found!
/// T14: Thread B increments reference count, releases write lock
/// ```

// ═══════════════════════════════════════════════════════════════════════════
//                          Layer 5: Testing and Tools
// ═══════════════════════════════════════════════════════════════════════════

/// # Test Helper Tools
///
/// These functions are only available in test environment, used to check pool's internal state
/// and perform isolated tests.

#[cfg(test)]
pub(crate) fn pool_stats() -> (usize, usize) {
    let pool = ARC_STR_POOL.get();
    (pool.len(), pool.capacity())
}

#[cfg(test)]
pub(crate) fn clear_pool_for_test() {
    use std::{thread, time::Duration};
    // Brief wait to ensure other threads complete operations
    thread::sleep(Duration::from_millis(10));
    ARC_STR_POOL.get().clear_sync();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    /// Run isolated test, ensure tests don't affect each other
    fn run_isolated_test<F: FnOnce()>(f: F) {
        clear_pool_for_test();
        f();
        clear_pool_for_test();
    }

    #[test]
    fn test_basic_functionality() {
        run_isolated_test(|| {
            let s1 = ArcStr::new("hello");
            let s2 = ArcStr::new("hello");
            let s3 = ArcStr::new("world");

            // Verify equality and pointer sharing
            assert_eq!(s1, s2);
            assert_ne!(s1, s3);
            assert_eq!(s1.ptr, s2.ptr); // Same content shares memory
            assert_ne!(s1.ptr, s3.ptr); // Different content different memory

            // Verify basic operations
            assert_eq!(s1.as_str(), "hello");
            assert_eq!(s1.len(), 5);
            assert!(!s1.is_empty());

            // Verify pool state
            let (count, _) = pool_stats();
            assert_eq!(count, 2); // "hello" and "world"
        });
    }

    #[test]
    fn test_reference_counting() {
        run_isolated_test(|| {
            let s1 = ArcStr::new("test");
            assert_eq!(s1.ref_count(), 1);

            let s2 = s1.clone();
            assert_eq!(s1.ref_count(), 2);
            assert_eq!(s2.ref_count(), 2);
            assert_eq!(s1.ptr, s2.ptr);

            drop(s2);
            assert_eq!(s1.ref_count(), 1);

            drop(s1);
            // Wait for drop to complete
            thread::sleep(Duration::from_millis(5));
            assert_eq!(pool_stats().0, 0);
        });
    }

    #[test]
    fn test_pool_reuse() {
        run_isolated_test(|| {
            let s1 = ArcStr::new("reuse_test");
            let s2 = ArcStr::new("reuse_test");

            assert_eq!(s1.ptr, s2.ptr);
            assert_eq!(s1.ref_count(), 2);
            assert_eq!(pool_stats().0, 1); // Only one pool entry
        });
    }

    #[test]
    fn test_thread_safety() {
        run_isolated_test(|| {
            let s = ArcStr::new("shared");
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    let s_clone = ArcStr::clone(&s);
                    thread::spawn(move || {
                        let local = ArcStr::new("shared");
                        assert_eq!(*s_clone, local);
                        assert_eq!(s_clone.ptr, local.ptr);
                    })
                })
                .collect();

            for handle in handles {
                handle.join().unwrap();
            }
        });
    }

    #[test]
    fn test_empty_string() {
        run_isolated_test(|| {
            let empty = ArcStr::new("");
            assert!(empty.is_empty());
            assert_eq!(empty.len(), 0);
            assert_eq!(empty.as_str(), "");
        });
    }

    #[test]
    fn test_from_implementations() {
        run_isolated_test(|| {
            use alloc::borrow::Cow;

            let s1 = ArcStr::from("from_str");
            let s2 = ArcStr::from(String::from("from_string"));
            let s3 = ArcStr::from(Cow::Borrowed("from_cow"));

            assert_eq!(s1.as_str(), "from_str");
            assert_eq!(s2.as_str(), "from_string");
            assert_eq!(s3.as_str(), "from_cow");
        });
    }
}
