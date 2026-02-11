/// Model data fetch mode
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum FetchMode {
    /// Overwrite existing data
    Truncate,

    /// Append new data and partially overwrite existing data
    AppendTruncate,

    /// Append new data
    Append,
}

impl FetchMode {
    /// String constant representing truncate mode
    const TRUNCATE: &'static str = "truncate";

    /// String constant representing append truncate mode
    const APPEND_TRUNCATE: &'static str = "append:truncate";

    /// String constant representing append mode
    const APPEND: &'static str = "append";

    /// Parse fetch mode from string
    #[inline]
    pub fn from_str(mut s: String) -> Self {
        s.make_ascii_lowercase();
        match s.as_str() {
            Self::TRUNCATE => Self::Truncate,
            Self::APPEND_TRUNCATE => Self::AppendTruncate,
            Self::APPEND => Self::Append,
            _ => Self::default(),
        }
    }

    /// Return string representation of fetch mode
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Truncate => Self::TRUNCATE,
            Self::AppendTruncate => Self::APPEND_TRUNCATE,
            Self::Append => Self::APPEND,
        }
    }
}

impl const Default for FetchMode {
    #[inline(always)]
    fn default() -> Self { Self::Truncate }
}

impl ::serde::Serialize for FetchMode {
    /// Serialize fetch mode
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> ::serde::Deserialize<'de> for FetchMode {
    /// Deserialize fetch mode
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_str(s))
    }
}
