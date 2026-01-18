use hybrid_array::{Array, ArraySize};

pub trait ToStrBuf: Sized {
    type BufSize: ArraySize;
    fn to_str<'buf>(&self, buf: &'buf mut Array<u8, Self::BufSize>) -> &'buf mut str;
}
