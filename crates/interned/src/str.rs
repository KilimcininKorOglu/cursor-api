//! Combined string type, unifying compile-time and runtime strings
//!
//! # Design Philosophy
//!
//! Two common types of strings in Rust:
//! - **Literals** (`&'static str`): Determined at compile time, zero cost, never released
//! - **Dynamic strings** (`String`, `ArcStr`): Constructed at runtime, requires memory management
//!
//! `Str` unifies both through an enum, providing a consistent API while preserving their respective performance advantages.
//!
//! # Memory Layout
//!
//! ```text
//! enum Str {
//!     Static(&'static str)  // 16 bytes (fat pointer)
//!     Counted(ArcStr)        // 8 bytes (NonNull)
//! }
//!
//! Total size: 17-24 bytes (depends on compiler optimization)
//! - Discriminant: 1 byte
//! - Padding: 0-7 bytes
//! - Data: 16 bytes (largest variant)
//! ```
//!
//! # Performance Comparison
//!
//! | Operation | Static | Counted |
//! |-----------|--------|---------|
//! | Create | 0 ns | ~100 ns (first time) / ~20 ns (pool hit) |
//! | Clone | ~1 ns | ~5 ns (atomic inc) |
//! | Drop | 0 ns | ~5 ns (atomic dec) + possible cleanup |
//! | `as_str()` | 0 ns | 0 ns (direct access) |
//! | `len()` | 0 ns | 0 ns (direct field read) |
//!
//! # Usage Scenarios
//!
//! ## Use Static variant
//!
//! ```rust
//! use interned::Str;
//!
//! // Constant table
//! static KEYWORDS: &[Str] = &[
//!     Str::from_static("fn"),
//!     Str::from_static("let"),
//!     Str::from_static("match"),
//! ];
//!
//! // Compile-time string
//! const ERROR_MSG: Str = Str::from_static("error occurred");
//! ```
//!
//! ## Use Counted variant
//!
//! ```rust
//! use interned::Str;
//!
//! // Runtime string (deduplicated)
//! let user_input = Str::new(get_user_input());
//!
//! // Cross-thread sharing
//! let shared = Str::new("config");
//! std::thread::spawn(move || {
//!     process(shared);
//! });
//! ```
//!
//! ## Common Pitfalls
//!
//! ```rust
//! use interned::Str;
//!
//! // Don't use new() for literals
//! let bad = Str::new("literal");  // Creates Counted, enters pool
//!
//! // Should use from_static
//! const GOOD: Str = Str::from_static("literal");  // Static variant, zero cost
//! ```

use super::arc_str::ArcStr;
use alloc::borrow::Cow;
use core::{
    cmp::Ordering,
    hash::{Hash, Hasher},
};

// ============================================================================
// Core Type Definition
// ============================================================================

/// Combined string type, supports compile-time literals and runtime reference-counted strings
///
/// # Variants
///
/// ## Static
///
/// - Wraps `&'static str`
/// - Zero allocation cost
/// - Zero runtime overhead
/// - Clone is simple pointer copy
/// - Never released
///
/// ## Counted
///
/// - Wraps `ArcStr`
/// - Heap allocated, deduplicated through global string pool
/// - Atomic reference count management
/// - Thread-safe sharing
/// - Reclaimed when last reference is released
///
/// # Method Shadowing
///
/// `Str` provides methods with the same names as `str` (like `len()`, `is_empty()`),
/// these methods shadow the versions provided by `Deref`, so that:
///
/// - For `Static` variant: Directly access `&'static str`
/// - For `Counted` variant: Use `ArcStr`'s optimized implementation (directly read internal fields)
///
/// ```rust
/// use interned::Str;
///
/// let s = Str::new("hello");
/// // Calls Str::len(), not <Str as Deref>::deref().len()
/// // For Counted variant, this avoids the overhead of constructing &str
/// assert_eq!(s.len(), 5);
/// ```
///
/// # Examples
///
/// ```rust
/// use interned::Str;
///
/// // Compile-time string
/// let s1 = Str::from_static("hello");
/// assert!(s1.is_static());
/// assert_eq!(s1.ref_count(), None);
///
/// // Runtime string
/// let s2 = Str::new("world");
/// assert!(!s2.is_static());
/// assert_eq!(s2.ref_count(), Some(1));
///
/// // Unified interface
/// assert_eq!(s1.len(), 5);
/// assert_eq!(s2.len(), 5);
/// ```
///
/// # Thread Safety
///
/// `Str` is `Send + Sync`, can be safely passed between threads:
///
/// ```rust
/// use interned::Str;
/// use std::thread;
///
/// let s = Str::new("shared");
/// thread::spawn(move || {
///     println!("{}", s);
/// });
/// ```
#[derive(Clone)]
pub enum Str {
    /// Compile-time string literal
    ///
    /// - Zero cost creation and access
    /// - Clone is pointer copy (~1ns)
    /// - Never releases memory
    /// - Suitable for constant tables and configuration
    Static(&'static str),

    /// Runtime reference-counted string
    ///
    /// - Automatically deduplicated through string pool
    /// - Atomic reference counting (thread-safe)
    /// - Clone increments reference count (~5ns)
    /// - Reclaimed when last reference is released
    Counted(ArcStr),
}

// SAFETY: Both variants are Send + Sync
unsafe impl Send for Str {}
unsafe impl Sync for Str {}

// ============================================================================
// Construction
// ============================================================================

impl Str {
    /// Create static string variant (compile-time literal)
    ///
    /// This is the **recommended way** to create zero-cost strings.
    ///
    /// # Const Context
    ///
    /// This function is `const fn`, can be evaluated at compile time:
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// const GREETING: Str = Str::from_static("Hello");
    ///
    /// static KEYWORDS: &[Str] = &[
    ///     Str::from_static("fn"),
    ///     Str::from_static("let"),
    /// ];
    /// ```
    ///
    /// # Performance
    ///
    /// - Compile time: Zero cost (string embedded in binary)
    /// - Runtime: Zero cost (just a pointer)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::from_static("constant");
    /// assert!(s.is_static());
    /// assert_eq!(s.as_static(), Some("constant"));
    /// assert_eq!(s.ref_count(), None);
    /// ```
    #[must_use]
    #[inline]
    pub const fn from_static(s: &'static str) -> Self { Self::Static(s) }

    /// Create or reuse runtime string
    ///
    /// String enters global string pool, strings with same content reuse the same memory.
    ///
    /// # Performance
    ///
    /// - **First creation**: Heap allocation + `HashMap` insertion ≈ 100-200ns
    /// - **Pool hit**: `HashMap` lookup + reference count increment ≈ 10-20ns
    ///
    /// # Thread Safety
    ///
    /// String pool is protected by `RwLock`, supports concurrent access:
    /// - Multiple threads can read simultaneously (lookup existing strings)
    /// - Creating new strings requires exclusive write lock
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::new("dynamic");
    /// let s2 = Str::new("dynamic");
    ///
    /// // Both strings share the same memory
    /// assert_eq!(s1.ref_count(), s2.ref_count());
    /// assert!(s1.ref_count().unwrap() >= 2);
    /// ```
    ///
    /// # Use Cases
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// // Compiler: identifier deduplication
    /// let ident = Str::new(token.text);
    ///
    /// // Config system: key name reuse
    /// let key = Str::new("database.host");
    ///
    /// // Cross-thread sharing
    /// let shared = Str::new("data");
    /// std::thread::spawn(move || {
    ///     process(shared);
    /// });
    /// # fn token() -> Token { Token { text: "x" } }
    /// # struct Token { text: &'static str }
    /// # fn process(_: Str) {}
    /// ```
    #[inline]
    pub fn new<S: AsRef<str>>(s: S) -> Self { Self::Counted(ArcStr::new(s)) }

    /// Check if it is Static variant
    ///
    /// Used to determine if string is a compile-time literal.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("literal");
    /// let s2 = Str::new("dynamic");
    ///
    /// assert!(s1.is_static());
    /// assert!(!s2.is_static());
    /// ```
    ///
    /// # Use Cases
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// fn optimize_for_static(s: &Str) {
    ///     if s.is_static() {
    ///         // Can safely convert to &'static str
    ///         let static_str = s.as_static().unwrap();
    ///         register_constant(static_str);
    ///     }
    /// }
    /// # fn register_constant(_: &'static str) {}
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_static(&self) -> bool { matches!(self, Self::Static(_)) }

    /// Get reference count
    ///
    /// - **Static variant**: Returns `None` (no reference count concept)
    /// - **Counted variant**: Returns `Some(count)`
    ///
    /// # Note
    ///
    /// Due to concurrent access, returned value may be outdated immediately after reading.
    /// Mainly used for debugging and testing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("static");
    /// let s2 = Str::new("counted");
    /// let s3 = s2.clone();
    ///
    /// assert_eq!(s1.ref_count(), None);
    /// assert_eq!(s2.ref_count(), Some(2));
    /// assert_eq!(s3.ref_count(), Some(2));
    /// ```
    #[must_use]
    #[inline]
    pub fn ref_count(&self) -> Option<usize> {
        match self {
            Self::Static(_) => None,
            Self::Counted(arc) => Some(arc.ref_count()),
        }
    }

    /// Try to get static string reference
    ///
    /// Only Static variant returns `Some`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("literal");
    /// let s2 = Str::new("dynamic");
    ///
    /// assert_eq!(s1.as_static(), Some("literal"));
    /// assert_eq!(s2.as_static(), None);
    /// ```
    ///
    /// # Use Cases
    ///
    /// Some APIs require `&'static str`:
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// fn register_global(name: &'static str) {
    ///     // Register string that requires static lifetime
    ///     # drop(name);
    /// }
    ///
    /// let s = Str::from_static("name");
    /// if let Some(static_str) = s.as_static() {
    ///     register_global(static_str);
    /// } else {
    ///     // Counted variant cannot be converted to 'static
    ///     eprintln!("warning: not a static string");
    /// }
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_static(&self) -> Option<&'static str> {
        match self {
            Self::Static(s) => Some(*s),
            Self::Counted(_) => None,
        }
    }

    /// Try to get reference to internal `ArcStr`
    ///
    /// Only Counted variant returns `Some`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("literal");
    /// let s2 = Str::new("dynamic");
    ///
    /// assert!(s1.as_arc_str().is_none());
    /// assert!(s2.as_arc_str().is_some());
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_arc_str(&self) -> Option<&ArcStr> {
        match self {
            Self::Static(_) => None,
            Self::Counted(arc) => Some(arc),
        }
    }

    /// Try to convert Counted variant to `ArcStr`
    ///
    /// - **Counted**: Returns `Some(ArcStr)`, zero-cost conversion
    /// - **Static**: Returns `None`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::new("counted");
    /// let s2 = Str::from_static("static");
    ///
    /// assert!(s1.into_arc_str().is_some());
    /// assert!(s2.into_arc_str().is_none());
    /// ```
    #[must_use]
    #[inline]
    pub fn into_arc_str(self) -> Option<ArcStr> {
        match self {
            Self::Static(_) => None,
            Self::Counted(arc) => Some(arc),
        }
    }
}

// ============================================================================
// Optimized str Methods (Method Shadowing)
// ============================================================================

impl Str {
    /// Get string slice
    ///
    /// This method overrides `as_str()` provided by `Deref`, so that:
    /// - For `Static` variant: Directly return `&'static str`
    /// - For `Counted` variant: Use `ArcStr::as_str()`'s optimized implementation
    ///
    /// # Performance
    ///
    /// - **Static**: Zero cost (just returns pointer)
    /// - **Counted**: Zero cost (directly accesses internal field)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("hello");
    /// assert_eq!(s.as_str(), "hello");
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Counted(arc) => arc.as_str(),
        }
    }

    /// Get byte slice of string
    ///
    /// Overrides `Deref` version to propagate `ArcStr::as_bytes()` optimization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("hello");
    /// assert_eq!(s.as_bytes(), b"hello");
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Static(s) => s.as_bytes(),
            Self::Counted(arc) => arc.as_bytes(),
        }
    }

    /// Get string length (in bytes)
    ///
    /// Overrides `Deref` version to propagate `ArcStr::len()` optimization (directly reads field).
    ///
    /// # Performance
    ///
    /// - **Static**: Reads len field of fat pointer
    /// - **Counted**: Reads `ArcStrInner::string_len` field (no need to construct `&str`)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("hello");
    /// assert_eq!(s.len(), 5);
    /// ```
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        match self {
            Self::Static(s) => s.len(),
            Self::Counted(arc) => arc.len(),
        }
    }

    /// Check if string is empty
    ///
    /// Overrides `Deref` version to propagate `ArcStr::is_empty()` optimization.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::new("");
    /// let s2 = Str::new("not empty");
    ///
    /// assert!(s1.is_empty());
    /// assert!(!s2.is_empty());
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        match self {
            Self::Static(s) => s.is_empty(),
            Self::Counted(arc) => arc.is_empty(),
        }
    }

    /// Get internal pointer (for debugging and testing)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("ptr");
    /// let ptr = s.as_ptr();
    /// assert!(!ptr.is_null());
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_ptr(&self) -> *const u8 {
        match self {
            Self::Static(s) => s.as_ptr(),
            Self::Counted(arc) => arc.as_ptr(),
        }
    }
}

// ============================================================================
// From Conversions
// ============================================================================

impl const From<&'static str> for Str {
    /// Create Static variant from literal
    ///
    /// **Note**: Only true `&'static str` will be automatically inferred as Static.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// // Literal automatically inferred as Static
    /// let s: Str = "literal".into();
    /// assert!(s.is_static());
    ///
    /// // But this won't work (compile error):
    /// // let owned = String::from("not static");
    /// // let s: Str = owned.as_str().into();  // Lifetime is not 'static
    /// ```
    #[inline]
    fn from(s: &'static str) -> Self { Self::Static(s) }
}

impl From<String> for Str {
    /// Create Counted variant from `String`
    ///
    /// String enters string pool, reuses if same content already exists.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s: Str = String::from("owned").into();
    /// assert!(!s.is_static());
    /// assert_eq!(s.as_str(), "owned");
    /// ```
    #[inline]
    fn from(s: String) -> Self { Self::Counted(ArcStr::from(s)) }
}

impl From<&String> for Str {
    /// Create Counted variant from `&String`
    #[inline]
    fn from(s: &String) -> Self { Self::Counted(ArcStr::from(s)) }
}

impl From<ArcStr> for Str {
    /// Create Counted variant from `ArcStr`
    ///
    /// Directly wraps, does not additionally increment reference count.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::{Str, ArcStr};
    ///
    /// let arc = ArcStr::new("shared");
    /// let count_before = arc.ref_count();
    ///
    /// let s: Str = arc.into();
    /// assert_eq!(s.ref_count(), Some(count_before));
    /// ```
    #[inline]
    fn from(arc: ArcStr) -> Self { Self::Counted(arc) }
}

impl<'a> From<Cow<'a, str>> for Str {
    /// Create Counted variant from `Cow<str>`
    ///
    /// Whether Cow is Borrowed or Owned, it will enter the string pool.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    /// use std::borrow::Cow;
    ///
    /// let borrowed: Cow<str> = Cow::Borrowed("borrowed");
    /// let owned: Cow<str> = Cow::Owned(String::from("owned"));
    ///
    /// let s1: Str = borrowed.into();
    /// let s2: Str = owned.into();
    ///
    /// assert!(!s1.is_static());
    /// assert!(!s2.is_static());
    /// ```
    #[inline]
    fn from(cow: Cow<'a, str>) -> Self { Self::Counted(ArcStr::from(cow)) }
}

impl From<alloc::boxed::Box<str>> for Str {
    /// Create Counted variant from `Box<str>`
    #[inline]
    fn from(s: alloc::boxed::Box<str>) -> Self { Self::Counted(ArcStr::from(s)) }
}

impl From<Str> for String {
    /// Convert to `String` (always requires allocation)
    ///
    /// # Performance
    ///
    /// Regardless of variant, requires allocation and copying string content.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("to_string");
    /// let string: String = s.into();
    /// assert_eq!(string, "to_string");
    /// ```
    #[inline]
    fn from(s: Str) -> Self { s.as_str().to_owned() }
}

impl From<Str> for alloc::boxed::Box<str> {
    /// Convert to `Box<str>` (requires allocation)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("boxed");
    /// let boxed: Box<str> = s.into();
    /// assert_eq!(&*boxed, "boxed");
    /// ```
    #[inline]
    fn from(s: Str) -> Self { s.as_str().into() }
}

impl From<Str> for Cow<'_, str> {
    /// Convert to `Cow`
    ///
    /// - **Static variant**: Converts to `Cow::Borrowed` (zero cost)
    /// - **Counted variant**: Converts to `Cow::Owned` (requires allocation)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    /// use std::borrow::Cow;
    ///
    /// let s1 = Str::from_static("static");
    /// let cow1: Cow<str> = s1.into();
    /// assert!(matches!(cow1, Cow::Borrowed(_)));
    ///
    /// let s2 = Str::new("counted");
    /// let cow2: Cow<str> = s2.into();
    /// assert!(matches!(cow2, Cow::Owned(_)));
    /// ```
    #[inline]
    fn from(s: Str) -> Self {
        match s {
            Str::Static(s) => Cow::Borrowed(s),
            Str::Counted(arc) => Cow::Owned(arc.into()),
        }
    }
}

impl<'a> const From<&'a Str> for Cow<'a, str> {
    /// Convert to `Cow::Borrowed` (zero cost)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    /// use std::borrow::Cow;
    ///
    /// let s = Str::from_static("cow");
    /// let cow: Cow<str> = (&s).into();
    ///
    /// assert!(matches!(cow, Cow::Borrowed(_)));
    /// assert_eq!(cow, "cow");
    /// ```
    #[inline]
    fn from(s: &'a Str) -> Self { Cow::Borrowed(s.as_str()) }
}

impl core::str::FromStr for Str {
    type Err = core::convert::Infallible;

    /// Parse from string (always succeeds, creates Counted variant)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    /// use std::str::FromStr;
    ///
    /// let s = Str::from_str("parsed").unwrap();
    /// assert!(!s.is_static());
    /// assert_eq!(s.as_str(), "parsed");
    /// ```
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::new(s)) }
}

// ============================================================================
// Comparison & Hashing
// ============================================================================

impl PartialEq for Str {
    /// Compare string content
    ///
    /// # Optimization
    ///
    /// - **Counted vs Counted**: First compare pointers (O(1)), then compare content
    /// - **Static vs Static**: Directly compare content (compiler may optimize to pointer comparison)
    /// - **Static vs Counted**: Must compare content
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("test");
    /// let s2 = Str::new("test");
    ///
    /// assert_eq!(s1, s2);  // Equal if content is same
    /// ```
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Counted vs Counted: Use ArcStr's pointer comparison optimization
            (Self::Counted(a), Self::Counted(b)) => a == b,
            // Other cases: Compare string content
            _ => self.as_str() == other.as_str(),
        }
    }
}

impl Eq for Str {}

impl const PartialEq<str> for Str {
    #[inline]
    fn eq(&self, other: &str) -> bool { self.as_str() == other }
}

impl const PartialEq<&str> for Str {
    #[inline]
    fn eq(&self, other: &&str) -> bool { self.as_str() == *other }
}

impl const PartialEq<String> for Str {
    #[inline]
    fn eq(&self, other: &String) -> bool { self.as_str() == other.as_str() }
}

impl const PartialEq<Str> for str {
    #[inline]
    fn eq(&self, other: &Str) -> bool { self == other.as_str() }
}

impl const PartialEq<Str> for &str {
    #[inline]
    fn eq(&self, other: &Str) -> bool { *self == other.as_str() }
}

impl const PartialEq<Str> for String {
    #[inline]
    fn eq(&self, other: &Str) -> bool { self.as_str() == other.as_str() }
}

impl PartialEq<ArcStr> for Str {
    /// Optimized `Str` and `ArcStr` comparison
    ///
    /// If `Str` is Counted variant, uses pointer comparison (fast path).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::{Str, ArcStr};
    ///
    /// let arc = ArcStr::new("test");
    /// let s1 = Str::from(arc.clone());
    /// let s2 = Str::from_static("test");
    ///
    /// assert_eq!(s1, arc);  // Pointer comparison
    /// assert_eq!(s2, arc);  // Content comparison
    /// ```
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool {
        match self {
            Self::Counted(arc) => arc == other,
            Self::Static(s) => *s == other.as_str(),
        }
    }
}

impl PartialEq<Str> for ArcStr {
    #[inline]
    fn eq(&self, other: &Str) -> bool { other == self }
}

impl Hash for Str {
    /// Hash based on string content, independent of variant type
    ///
    /// This ensures that `Static("a")` and `Counted(ArcStr::new("a"))`
    /// have the same hash value, can be used as the same key in `HashMap`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// let s1 = Str::from_static("key");
    /// let s2 = Str::new("key");
    ///
    /// map.insert(s1, "value");
    /// assert_eq!(map.get(&s2), Some(&"value"));  // s2 can find value inserted by s1
    /// ```
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) { self.as_str().hash(state) }
}

// ============================================================================
// Ordering
// ============================================================================

impl PartialOrd for Str {
    /// Lexicographic comparison
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let a = Str::from_static("apple");
    /// let b = Str::new("banana");
    ///
    /// assert!(a < b);
    /// assert!(b > a);
    /// ```
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for Str {
    /// Lexicographic comparison (total order)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let mut strs = vec![
    ///     Str::new("cherry"),
    ///     Str::from_static("apple"),
    ///     Str::new("banana"),
    /// ];
    ///
    /// strs.sort();
    ///
    /// assert_eq!(strs[0].as_str(), "apple");
    /// assert_eq!(strs[1].as_str(), "banana");
    /// assert_eq!(strs[2].as_str(), "cherry");
    /// ```
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering { self.as_str().cmp(other.as_str()) }
}

// ============================================================================
// Deref & AsRef
// ============================================================================

impl core::ops::Deref for Str {
    type Target = str;

    /// Supports automatic dereferencing to `&str`
    ///
    /// This allows directly calling all `str` methods (like `starts_with()`, `contains()`, etc.).
    ///
    /// **Note**: Common methods (like `len()`, `is_empty()`) are overridden by `Str`'s methods
    /// with the same name to propagate `ArcStr`'s optimizations.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::from_static("deref");
    ///
    /// // Can directly call str methods
    /// assert!(s.starts_with("de"));
    /// assert!(s.contains("ref"));
    /// assert_eq!(s.to_uppercase(), "DEREF");
    /// ```
    #[inline]
    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl const AsRef<str> for Str {
    #[inline]
    fn as_ref(&self) -> &str { self.as_str() }
}

impl const AsRef<[u8]> for Str {
    #[inline]
    fn as_ref(&self) -> &[u8] { self.as_bytes() }
}

impl const core::borrow::Borrow<str> for Str {
    /// Supports lookup with `&str` in `HashMap<Str, V>`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(Str::new("key"), "value");
    ///
    /// // Can use &str for lookup
    /// assert_eq!(map.get("key"), Some(&"value"));
    /// ```
    #[inline]
    fn borrow(&self) -> &str { self.as_str() }
}

// ============================================================================
// Display & Debug
// ============================================================================

impl core::fmt::Display for Str {
    /// Output string content
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::from_static("display");
    /// assert_eq!(format!("{}", s), "display");
    /// ```
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { f.write_str(self.as_str()) }
}

impl core::fmt::Debug for Str {
    /// Debug output, shows variant type and content
    ///
    /// # Output Format
    ///
    /// - **Static**: `Str::Static("content")`
    /// - **Counted**: `Str::Counted("content", refcount=N)`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("debug");
    /// let s2 = Str::new("counted");
    ///
    /// println!("{:?}", s1);  // Str::Static("debug")
    /// println!("{:?}", s2);  // Str::Counted("counted", refcount=1)
    /// ```
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Static(s) => f.debug_tuple("Str::Static").field(s).finish(),
            Self::Counted(arc) => f
                .debug_tuple("Str::Counted")
                .field(&arc.as_str())
                .field(&format_args!("refcount={}", arc.ref_count()))
                .finish(),
        }
    }
}

// ============================================================================
// Default
// ============================================================================

impl const Default for Str {
    /// Returns Static variant of empty string
    ///
    /// This is zero cost, does not allocate any memory.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::default();
    /// assert!(s.is_empty());
    /// assert!(s.is_static());
    /// assert_eq!(s.as_str(), "");
    /// ```
    #[inline]
    fn default() -> Self { Self::Static(Default::default()) }
}

// ============================================================================
// Serde Support
// ============================================================================

#[cfg(feature = "serde")]
mod serde_impls {
    use super::Str;
    use serde_core::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for Str {
        /// Serialize as plain string, loses variant information
        ///
        /// **Note**: After deserialization, it's always Counted variant.
        ///
        /// # Examples
        ///
        /// ```rust
        /// use interned::Str;
        ///
        /// let s = Str::from_static("serialize");
        /// let json = serde_json::to_string(&s).unwrap();
        /// assert_eq!(json, r#""serialize""#);
        /// ```
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
            self.as_str().serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for Str {
        /// Deserialize as Counted variant
        ///
        /// **Note**: Cannot restore Static variant, because deserialized string
        /// does not have `'static` lifetime.
        ///
        /// # Examples
        ///
        /// ```rust
        /// use interned::Str;
        ///
        /// let json = r#""deserialize""#;
        /// let s: Str = serde_json::from_str(json).unwrap();
        ///
        /// assert!(!s.is_static());  // Always Counted
        /// assert_eq!(s.as_str(), "deserialize");
        /// ```
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
            String::deserialize(deserializer).map(Str::from)
        }
    }
}

/// # Rkyv Serialization Support
///
/// Conditionally compiled Rkyv support, allows `ArcStr` to participate in serialization/deserialization.
/// Outputs string content when serializing, re-establishes pooled reference when deserializing.
#[cfg(feature = "rkyv")]
mod rkyv_impls {
    use super::Str;
    use core::cmp::Ordering;
    use rkyv::{
        Archive, Deserialize, DeserializeUnsized, Place, Serialize, SerializeUnsized,
        rancor::{Fallible, Source},
        string::{ArchivedString, StringResolver},
    };

    impl Archive for Str {
        type Archived = ArchivedString;
        type Resolver = StringResolver;

        #[inline]
        fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
            ArchivedString::resolve_from_str(self.as_str(), resolver, out);
        }
    }

    impl<S: Fallible + ?Sized> Serialize<S> for Str
    where
        S::Error: Source,
        str: SerializeUnsized<S>,
    {
        fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
            ArchivedString::serialize_from_str(self.as_str(), serializer)
        }
    }

    impl<D: Fallible + ?Sized> Deserialize<Str, D> for ArchivedString
    where str: DeserializeUnsized<str, D>
    {
        fn deserialize(&self, _: &mut D) -> Result<Str, D::Error> {
            Ok(Str::new(self.as_str()))
        }
    }

    impl PartialEq<Str> for ArchivedString {
        #[inline]
        fn eq(&self, other: &Str) -> bool { PartialEq::eq(self.as_str(), other.as_str()) }
    }

    impl PartialEq<ArchivedString> for Str {
        #[inline]
        fn eq(&self, other: &ArchivedString) -> bool {
            PartialEq::eq(other.as_str(), self.as_str())
        }
    }

    impl PartialOrd<Str> for ArchivedString {
        #[inline]
        fn partial_cmp(&self, other: &Str) -> Option<Ordering> {
            self.as_str().partial_cmp(other.as_str())
        }
    }

    impl PartialOrd<ArchivedString> for Str {
        #[inline]
        fn partial_cmp(&self, other: &ArchivedString) -> Option<Ordering> {
            self.as_str().partial_cmp(other.as_str())
        }
    }
}

// ============================================================================
// Testing
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_shadowing() {
        let s1 = Str::from_static("hello");
        let s2 = Str::new("world");

        // Verify the overridden version is called (compiling is enough)
        assert_eq!(s1.len(), 5);
        assert_eq!(s2.len(), 5);
        assert!(!s1.is_empty());
        assert_eq!(s1.as_bytes(), b"hello");
        assert_eq!(s1.as_str(), "hello");
    }

    #[test]
    fn test_static_vs_counted() {
        let s1 = Str::from_static("hello");
        let s2 = Str::new("hello");

        assert!(s1.is_static());
        assert!(!s2.is_static());
        assert_eq!(s1.ref_count(), None);
        assert!(s2.ref_count().is_some());
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_arcstr_conversions() {
        let arc = ArcStr::new("test");
        let count_before = arc.ref_count();

        // ArcStr -> Str
        let s: Str = arc.clone().into();
        assert!(!s.is_static());
        assert_eq!(s.ref_count(), Some(count_before + 1));

        // Str -> Option<ArcStr>
        let arc_back = s.into_arc_str();
        assert!(arc_back.is_some());
        assert_eq!(arc_back.unwrap(), arc);
    }

    #[test]
    fn test_arcstr_equality() {
        let arc = ArcStr::new("same");
        let s1 = Str::from(arc.clone());
        let s2 = Str::from_static("same");

        // Counted vs ArcStr: pointer comparison
        assert_eq!(s1, arc);

        // Static vs ArcStr: content comparison
        assert_eq!(s2, arc);
    }

    #[test]
    fn test_default() {
        let s = Str::default();
        assert!(s.is_empty());
        assert!(s.is_static());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn test_const_construction() {
        const GREETING: Str = Str::from_static("Hello");
        static KEYWORDS: &[Str] =
            &[Str::from_static("fn"), Str::from_static("let"), Str::from_static("match")];

        assert!(GREETING.is_static());
        assert_eq!(KEYWORDS.len(), 3);
        assert!(KEYWORDS[0].is_static());
    }

    #[test]
    fn test_deref() {
        let s = Str::from_static("deref");

        // Access str methods through Deref
        assert!(s.starts_with("de"));
        assert!(s.contains("ref"));
        assert_eq!(s.to_uppercase(), "DEREF");
    }

    #[test]
    fn test_ordering() {
        let mut strs = vec![Str::new("cherry"), Str::from_static("apple"), Str::new("banana")];

        strs.sort();

        assert_eq!(strs[0], "apple");
        assert_eq!(strs[1], "banana");
        assert_eq!(strs[2], "cherry");
    }

    #[test]
    fn test_conversions() {
        // From implementations
        let s1: Str = "literal".into();
        let s2: Str = String::from("owned").into();
        let s3: Str = ArcStr::new("arc").into();

        assert!(s1.is_static());
        assert!(!s2.is_static());
        assert!(!s3.is_static());

        // Into implementations
        let string: String = s2.clone().into();
        assert_eq!(string, "owned");

        let boxed: alloc::boxed::Box<str> = s3.into();
        assert_eq!(&*boxed, "arc");
    }

    #[test]
    fn test_hash_consistency() {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };

        let s1 = Str::from_static("test");
        let s2 = Str::new("test");

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();

        s1.hash(&mut h1);
        s2.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }
}
