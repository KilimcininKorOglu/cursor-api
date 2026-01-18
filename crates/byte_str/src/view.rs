use bytes::Bytes;
use core::{mem::transmute, sync::atomic::AtomicPtr};

#[allow(unused)]
pub struct BytesUnsafeView {
    pub ptr: *const u8,
    pub len: usize,
    // inlined "trait object"
    data: AtomicPtr<()>,
    vtable: &'static (),
}

impl BytesUnsafeView {
    #[inline]
    pub const fn from_ref(src: &Bytes) -> &Self { unsafe { transmute(src) } }
    #[inline]
    pub const fn from(src: Bytes) -> Self { unsafe { transmute(src) } }
    #[inline]
    pub const fn to(self) -> Bytes { unsafe { transmute(self) } }
}
