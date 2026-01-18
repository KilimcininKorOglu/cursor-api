use serde::Serialize;

#[derive(Clone, Copy, PartialEq)]
pub enum Tri<T> {
    Null(bool),
    Value(T),
}

impl<T> const Default for Tri<T> {
    fn default() -> Self { Self::Null(false) }
}

impl<T> Tri<T> {
    #[inline(always)]
    pub const fn is_undefined(&self) -> bool { matches!(*self, Tri::Null(false)) }

    // #[inline(always)]
    // pub const fn is_null(&self) -> bool {
    //     matches!(*self, Tri::Null)
    // }

    // #[inline(always)]
    // pub const fn is_value(&self) -> bool {
    //     matches!(*self, Tri::Value(_))
    // }

    // pub const fn as_value(&self) -> Option<&T> {
    //     match self {
    //         Tri::Value(v) => Some(v),
    //         _ => None,
    //     }
    // }
}

impl<T> Serialize for Tri<T>
where T: Serialize
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self {
            Tri::Null(..) => serializer.serialize_unit(),
            Tri::Value(value) => value.serialize(serializer),
        }
    }
}
