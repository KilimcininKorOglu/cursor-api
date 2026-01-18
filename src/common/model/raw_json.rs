use byte_str::ByteStr;
use bytes::Bytes;
use core::mem::transmute;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;

#[repr(transparent)]
pub struct RawJson(ByteStr);

impl RawJson {
    #[inline]
    pub fn into_bytes(self) -> Bytes { self.0.into_bytes() }

    #[inline]
    pub const fn as_str(&self) -> &str { &self.0 }
}

impl const Default for RawJson {
    #[inline]
    fn default() -> Self { Self(ByteStr::new()) }
}

impl Clone for RawJson {
    #[inline]
    fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl Serialize for RawJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serialize_as_raw_value(self.as_str(), serializer)
    }
}

pub fn to_raw_json<T>(value: &T) -> Result<RawJson, serde_json::Error>
where T: ?Sized + Serialize {
    let json = match serde_json::to_string(value) {
        Ok(s) => s.into(),
        Err(e) => return Err(e),
    };
    Ok(RawJson(json))
}

#[inline(always)]
pub fn serialize_as_raw_value<S>(src: &str, serializer: S) -> Result<S::Ok, S::Error>
where S: Serializer {
    unsafe { transmute::<&str, &RawValue>(src) }.serialize(serializer)
}

#[inline(always)]
pub fn serialize_as_option_raw_value<'a, S>(
    src: &'a Option<&'a str>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match src {
        Some(src) => serialize_as_raw_value(src, serializer),
        src => src.serialize(serializer),
    }
}
