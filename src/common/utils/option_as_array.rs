use core::{fmt, marker::PhantomData};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeTuple as _};

/// Serialize Option<T> to JSON array:
/// Some(v) -> [v]
/// None    -> []
pub fn serialize<S, T>(option: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize,
{
    // Calculate tuple length: Some -> 1, None -> 0
    let len = option.is_some() as usize;
    let mut tup = serializer.serialize_tuple(len)?;
    match option {
        Some(value) => {
            // Serialize to single-element array [value]
            tup.serialize_element(value)?;
        }
        None => {
            // Serialize to empty array []
        }
    }
    tup.end()
}

/// Deserialize JSON array to Option<T>:
/// [v] -> Some(v)
/// []  -> None
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct OptionArrayVisitor<T>(PhantomData<T>);
    impl<'de, T> de::Visitor<'de> for OptionArrayVisitor<T>
    where T: Deserialize<'de>
    {
        type Value = Option<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an empty array or an array with one element")
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where A: de::SeqAccess<'de> {
            // Try to read first element of array
            match seq.next_element()? {
                Some(value) => {
                    // Read value, corresponds to [v] -> Some(value)
                    Ok(Some(value))
                }
                None => {
                    // No elements, corresponds to [] -> None
                    Ok(None)
                }
            }
        }
    }
    deserializer.deserialize_seq(OptionArrayVisitor(PhantomData))
}
