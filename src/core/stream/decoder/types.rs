use ::prost::Message;

/// Represents a message type that can be encoded/decoded with Protobuf and can create default instances
pub trait ProtobufMessage: Message + Default {}

// /// Automatically implement ProtobufMessage for all types that implement both Message and Default
// impl<T: Message + Default> ProtobufMessage for T {}

macro_rules! impl_protobuf_message {
    ($($t:ty),*$(,)?) => {
        $(impl ProtobufMessage for $t {})*
    };
}

impl_protobuf_message!(
    crate::core::aiserver::v1::CppConfigRequest,
    crate::core::aiserver::v1::CppConfigResponse,
    // crate::core::aiserver::v1::GetServerConfigResponse,
    // crate::core::aiserver::v1::GetEmailResponse,
    crate::core::aiserver::v1::StreamCppRequest,
    crate::core::aiserver::v1::StreamCppResponse,
    crate::core::aiserver::v1::AvailableCppModelsResponse,
    crate::core::aiserver::v1::FsSyncFileRequest,
    crate::core::aiserver::v1::FsUploadFileRequest,
    crate::core::aiserver::v1::FsSyncFileResponse,
    crate::core::aiserver::v1::FsUploadFileResponse,
);

pub enum DecodedMessage<T: ProtobufMessage> {
    Protobuf(T),
    Text(String),
}

// impl<T: ProtobufMessage> DecodedMessage<T> {
//     #[inline]
//     pub fn encode(&self) -> Vec<u8>
//     where
//         Self: Sized,
//     {
//         match self {
//             DecodedMessage::Protobuf(msg) => msg.encode_to_vec(),
//             DecodedMessage::Text(s) => s.as_bytes().to_vec(),
//         }
//     }
// }

// impl<T: ProtobufMessage> ::core::fmt::Debug for DecodedMessage<T> {
//     fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
//         match self {
//             Self::Protobuf(msg) => write!(f, "\n{msg:#?}"),
//             Self::Text(s) => write!(f, "\n{s:?}"),
//         }
//     }
// }

// impl<T: ProtobufMessage + ::serde::Serialize> ::core::fmt::Display for DecodedMessage<T> {
//     fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
//         match self {
//             Self::Protobuf(msg) => write!(f, "\n{}", serde_json::to_string(msg).unwrap()),
//             Self::Text(s) => write!(f, "\n{s}"),
//         }
//     }
// }

// #[derive(Debug, PartialEq)]
pub enum DecoderError {
    // Error passed from upstream service
    // ChatError(String),
    // DataLengthLessThan5,
    // EmptyStream,
    Internal(&'static str),
}

// impl std::fmt::Display for DecoderError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         match self {
//             // DecoderError::ChatError(s) => write!(f, "{s}"),
//             // DecoderError::DataLengthLessThan5 => write!(f, "data length less than 5"),
//             // DecoderError::EmptyStream => write!(f, "empty stream"),
//             DecoderError::Internal(s) => write!(f, "{s}"),
//         }
//     }
// }

// impl core::error::Error for DecoderError {}

// unsafe impl Send for DecoderError {}
// unsafe impl Sync for DecoderError {}
