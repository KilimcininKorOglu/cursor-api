pub trait ConstString: Default {
    const VALUE: &str;
    #[inline(always)]
    fn as_str(&self) -> &'static str { Self::VALUE }
}

macro_rules! const_string {
    ($name:ident = $value:literal) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name;

        impl $crate::common::utils::const_string::ConstString for $name {
            const VALUE: &str = $value;
        }

        impl ::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where S: ::serde::Serializer {
                $crate::common::utils::const_string::ConstString::as_str(self).serialize(serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where D: ::serde::Deserializer<'de> {
                let s = String::deserialize(deserializer)?;
                if s == <Self as $crate::common::utils::const_string::ConstString>::VALUE {
                    Ok($name)
                } else {
                    Err(::serde::de::Error::custom(concat!(
                        "expect const string value \"",
                        $value,
                        "\""
                    )))
                }
            }
        }
    };
}

pub(crate) use const_string;
