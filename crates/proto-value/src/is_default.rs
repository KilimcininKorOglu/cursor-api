pub const trait IsDefault: Sized {
    fn is_default(&self) -> bool;
}

impl const IsDefault for bool {
    #[inline(always)]
    fn is_default(&self) -> bool {
        !*self
    }
}

impl const IsDefault for i32 {
    #[inline(always)]
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl const IsDefault for i64 {
    #[inline(always)]
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl const IsDefault for u32 {
    #[inline(always)]
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl const IsDefault for u64 {
    #[inline(always)]
    fn is_default(&self) -> bool {
        *self == 0
    }
}

impl const IsDefault for f32 {
    #[inline(always)]
    fn is_default(&self) -> bool {
        *self == 0.0
    }
}

impl const IsDefault for f64 {
    #[inline(always)]
    fn is_default(&self) -> bool {
        *self == 0.0
    }
}

#[cfg(feature = "alloc")]
impl const IsDefault for ::alloc::string::String {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "byte_str")]
impl const IsDefault for ::byte_str::ByteStr {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "bytes")]
impl const IsDefault for ::bytes::Bytes {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "alloc")]
impl<T> const IsDefault for ::alloc::vec::Vec<T> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

impl<T> const IsDefault for crate::Enum<T> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.get().is_default()
    }
}

#[cfg(feature = "bytes")]
impl const IsDefault for crate::Bytes<::bytes::Bytes> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
}

#[cfg(feature = "alloc")]
impl const IsDefault for crate::Bytes<::alloc::vec::Vec<u8>> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.0.is_default()
    }
}

impl<T> const IsDefault for ::core::option::Option<T> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_none()
    }
}

#[cfg(feature = "std")]
impl<K, V, S> IsDefault for std::collections::HashMap<K, V, S> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "alloc")]
impl<K, V, A: ::alloc::alloc::Allocator + Clone> IsDefault for ::alloc::collections::BTreeMap<K, V, A> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "indexmap")]
impl<K, V, S> IsDefault for ::indexmap::IndexMap<K, V, S> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}
