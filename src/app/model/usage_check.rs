use crate::{
    app::constant::COMMA,
    core::{
        config::configured_key,
        constant::{FREE_MODELS, Models},
    },
};
use serde::Deserialize;

// Define type constants
crate::define_typed_constants! {
    &'static str => {
        TYPE_NONE = "none",
        TYPE_DISABLED = "disabled",
        TYPE_DEFAULT = "default",
        TYPE_ALL = "all",
        TYPE_EVERYTHING = "everything",
        TYPE_LIST = "list",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UsageCheck {
    None,
    Default,
    All,
    Custom(Vec<&'static str>),
}

impl UsageCheck {
    #[inline]
    pub fn from_string(mut s: String) -> Self {
        s.make_ascii_lowercase();
        match s.as_str() {
            TYPE_NONE | TYPE_DISABLED => Self::None,
            TYPE_DEFAULT => Self::Default,
            TYPE_ALL | TYPE_EVERYTHING => Self::All,
            s => Self::parse_custom_models(s),
        }
    }

    #[inline]
    fn parse_custom_models(s: &str) -> Self {
        let models: Vec<_> = s
            .split(COMMA)
            .filter_map(|model| Models::find_id(model.trim()))
            .map(|m| m.id)
            .collect();

        if models.is_empty() { Self::default() } else { Self::Custom(models) }
    }

    #[inline]
    pub fn from_pb(model: &configured_key::UsageCheckModel) -> Self {
        use configured_key::usage_check_model::Type;

        match model.r#type {
            Type::Default => Self::Default,
            Type::Disabled => Self::None,
            Type::All => Self::All,
            Type::Custom => {
                let models: Vec<_> = model
                    .model_ids
                    .iter()
                    .filter_map(|id| Models::find_id(id))
                    .map(|m| m.id)
                    .collect();

                if models.is_empty() { Self::None } else { Self::Custom(models) }
            }
        }
    }

    // Helper method: get type string
    // #[inline]
    // fn type_str(&self) -> &'static str {
    //     match self {
    //         Self::None => TYPE_NONE,
    //         Self::Default => TYPE_DEFAULT,
    //         Self::All => TYPE_ALL,
    //         Self::Custom(_) => TYPE_LIST,
    //     }
    // }

    #[inline]
    pub fn check(&self, id: &&'static str) -> bool {
        match self {
            Self::None => false,
            Self::Default => !FREE_MODELS.contains(id),
            Self::All => true,
            Self::Custom(models) => models.contains(id),
        }
    }

    #[inline]
    pub fn hash(&self, hasher: &mut impl sha2::Digest) {
        match self {
            Self::None => hasher.update(TYPE_NONE.as_bytes()),
            Self::Default => hasher.update(TYPE_DEFAULT.as_bytes()),
            Self::All => hasher.update(TYPE_ALL.as_bytes()),
            Self::Custom(list) => {
                hasher.update(TYPE_LIST.as_bytes());
                for id in list {
                    hasher.update(id.as_bytes());
                }
            }
        }
    }
}

impl const Default for UsageCheck {
    #[inline(always)]
    fn default() -> Self { Self::Default }
}

impl<'de> Deserialize<'de> for UsageCheck {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_string(s))
    }
}

// impl Serialize for UsageCheck {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where S: serde::Serializer {
//         use serde::ser::SerializeStruct;

//         match self {
//             Self::Custom(models) => {
//                 let mut state = serializer.serialize_struct(STRUCT_NAME, 2)?;
//                 state.serialize_field(FIELD_TYPE, TYPE_LIST)?;
//                 state.serialize_field(FIELD_CONTENT, &models.join(COMMA_STRING))?;
//                 state.end()
//             }
//             _ => {
//                 let mut state = serializer.serialize_struct(STRUCT_NAME, 1)?;
//                 state.serialize_field(FIELD_TYPE, self.type_str())?;
//                 state.end()
//             }
//         }
//     }
// }

// impl<'de> Deserialize<'de> for UsageCheck {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where D: serde::Deserializer<'de> {
//         use serde::de::{self, MapAccess, Visitor};

//         struct UsageCheckVisitor;

//         impl<'de> Visitor<'de> for UsageCheckVisitor {
//             type Value = UsageCheck;

//             fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
//                 formatter.write_str("a UsageCheck object with 'type' field")
//             }

//             fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//             where A: MapAccess<'de> {
//                 let mut type_value: Option<String> = None;
//                 let mut content_value: Option<String> = None;

//                 while let Some(key) = map.next_key::<String>()? {
//                     match key.as_str() {
//                         FIELD_TYPE => {
//                             type_value = Some(map.next_value()?);
//                         }
//                         FIELD_CONTENT => {
//                             content_value = Some(map.next_value()?);
//                         }
//                         _ => {
//                             // Ignore unknown field
//                             let _: de::IgnoredAny = map.next_value()?;
//                         }
//                     }
//                 }

//                 let type_str = type_value.ok_or_else(|| de::Error::missing_field(FIELD_TYPE))?;

//                 Ok(match type_str.as_str() {
//                     TYPE_NONE | TYPE_DISABLED => UsageCheck::None,
//                     TYPE_DEFAULT => UsageCheck::Default,
//                     TYPE_ALL | TYPE_EVERYTHING => UsageCheck::All,
//                     TYPE_LIST => {
//                         let content =
//                             content_value.ok_or_else(|| de::Error::missing_field(FIELD_CONTENT))?;

//                         if content.is_empty() {
//                             UsageCheck::None
//                         } else {
//                             UsageCheck::parse_custom_models(&content)
//                         }
//                     }
//                     _ => {
//                         return Err(de::Error::unknown_variant(
//                             &type_str,
//                             &[
//                                 TYPE_NONE,
//                                 TYPE_DISABLED,
//                                 TYPE_DEFAULT,
//                                 TYPE_ALL,
//                                 TYPE_EVERYTHING,
//                                 TYPE_LIST,
//                             ],
//                         ));
//                     }
//                 })
//             }
//         }

//         deserializer.deserialize_struct(
//             STRUCT_NAME,
//             &[FIELD_TYPE, FIELD_CONTENT],
//             UsageCheckVisitor,
//         )
//     }
// }
