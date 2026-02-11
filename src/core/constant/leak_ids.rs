use alloc::alloc::{alloc, handle_alloc_error};
use core::{
    alloc::Layout,
    borrow::Borrow,
    ptr::{copy_nonoverlapping, from_raw_parts, metadata},
};
use manually_init::ManuallyInit;

type HashSet<T> = scc::HashMap<T, (), ahash::RandomState>;

const SUFFIX: &'static str = "-online";

#[derive(Clone, Copy)]
struct Id(*const u8, usize);

impl Id {
    #[inline]
    const fn suffix(self) -> &'static str {
        unsafe { &*from_raw_parts(self.0, self.1.unchecked_add(SUFFIX.len())) }
    }
    #[inline]
    const fn non_suffix(self) -> &'static str { unsafe { &*from_raw_parts(self.0, self.1) } }
    #[inline]
    const fn from_ptr(ptr: *const str) -> Self { Self(ptr.cast(), metadata(ptr)) }
    #[inline]
    const fn from_ref(s: &'static str) -> Self { Self::from_ptr(s as _) }
}

/// Manually allocate memory and copy string
///
/// # Safety
/// The allocated memory will be converted to 'static lifetime, caller must ensure it won't be manually freed
#[inline]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn alloc_ids(s: &str) -> Id {
    let sptr = s.as_ptr();
    let len = s.len();

    // Calculate layout, string doesn't need special alignment
    let layout = Layout::from_size_align_unchecked(len + SUFFIX.len(), 1);

    // Allocate memory
    let ptr = alloc(layout);
    if ptr.is_null() {
        // Memory allocation failed
        handle_alloc_error(layout);
    }

    // Copy string content
    copy_nonoverlapping(sptr, ptr, len);
    copy_nonoverlapping(SUFFIX.as_ptr(), ptr.add(len), SUFFIX.len());

    // Construct string slice from raw parts
    Id(ptr, len)
}

// Global instance
static STATIC_POOL: ManuallyInit<HashSet<&'static str>> = ManuallyInit::new();

pub(super) fn init() { STATIC_POOL.init(HashSet::default()) }

#[inline]
fn __intern(pool: &HashSet<&'static str>, s: &str) -> (Id, bool) {
    use scc::hash_map::RawEntry;

    let (key, is_suffix) = match s.strip_suffix(SUFFIX) {
        Some(s) => (s, true),
        None => (s, false),
    };

    let id = match pool.raw_entry().from_key_sync(key) {
        RawEntry::Occupied(entry) => Id::from_ref(entry.key()),
        RawEntry::Vacant(entry) => {
            let leaked = unsafe { alloc_ids(s) };
            entry.insert(leaked.non_suffix(), ());
            leaked
        }
    };
    (id, is_suffix)
}

// Public API
pub fn add<S: Borrow<str>>(s: S) -> (&'static str, &'static str) {
    let id = __intern(&STATIC_POOL, s.borrow()).0;
    (id.suffix(), id.non_suffix())
}

pub fn intern<S: Borrow<str>>(s: S) -> &'static str {
    let (id, is) = __intern(&STATIC_POOL, s.borrow());
    if is { id.suffix() } else { id.non_suffix() }
}
