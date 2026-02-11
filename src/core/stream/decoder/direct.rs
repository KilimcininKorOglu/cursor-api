// use ::bytes::{Buf as _, BytesMut};

use super::{
    decompress_gzip,
    types::{DecodedMessage, DecoderError, ProtobufMessage},
};
use alloc::borrow::Cow;

// #[derive(Clone)]
// pub struct DirectDecoder<T: ProtobufMessage> {
//     buf: BytesMut,
//     _phantom: std::marker::PhantomData<T>,
// }

// impl<T: ProtobufMessage> DirectDecoder<T> {
//     pub fn new() -> Self {
//         Self {
//             buf: BytesMut::new(),
//             _phantom: std::marker::PhantomData,
//         }
//     }

//     pub fn decode(&mut self, data: &[u8]) -> Result<Option<DecodedMessage<T>>, DecoderError> {
//         self.buf.extend_from_slice(data);

//         // First try to process in format with header
//         if self.buf.len() >= 5 && self.buf[0] <= 1 {
//             // Check header
//             let is_compressed = data[0] == 1;
//             let msg_len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

//             // If data is complete, process in format with header
//             if self.buf.len() == 5 + msg_len {
//                 self.buf.advance(5);

//                 if is_compressed {
//                     match decompress_gzip(&self.buf) {
//                         Some(data) => {
//                             self.buf = data.as_slice().into();
//                         }
//                         None => return Err(DecoderError::Internal("decompress error")),
//                     }
//                 };

//                 if let Ok(msg) = T::decode(&self.buf[..]) {
//                     return Ok(Some(DecodedMessage::Protobuf(msg)));
//                 } else if let Ok(text) = String::from_utf8(self.buf.to_vec()) {
//                     return Ok(Some(DecodedMessage::Text(text)));
//                 }
//             }
//         }

//         // If not in format with header, try to process data directly
//         // First try to decompress (may be compressed direct data)
//         if let Some(decompressed) = decompress_gzip(&self.buf) {
//             self.buf = decompressed.as_slice().into();
//         };

//         // Try to parse
//         if let Ok(msg) = T::decode(&self.buf[..]) {
//             self.buf.clear();
//             Ok(Some(DecodedMessage::Protobuf(msg)))
//         } else if let Ok(text) = String::from_utf8(self.buf.to_vec()) {
//             self.buf.clear();
//             Ok(Some(DecodedMessage::Text(text)))
//         } else {
//             Ok(None)
//         }
//     }
// }

pub fn decode<T: ProtobufMessage>(data: &[u8]) -> Result<DecodedMessage<T>, DecoderError> {
    // First try to process in format with header
    if data.len() >= 5 && data[0] <= 1 {
        // Check header
        let is_compressed = data[0] == 1;
        let msg_len = u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize;

        // If data is complete, process in format with header
        if data.len() == 5 + msg_len {
            let payload = &data[5..];

            let decompressed = if is_compressed {
                match decompress_gzip(payload) {
                    Some(data) => Cow::Owned(data),
                    None => return Err(DecoderError::Internal("decompress error")),
                }
            } else {
                Cow::Borrowed(payload)
            };

            if let Ok(msg) = T::decode(&*decompressed) {
                return Ok(DecodedMessage::Protobuf(msg));
            } else if let Some(text) = super::utils::string_from_utf8(decompressed) {
                return Ok(DecodedMessage::Text(text));
            }
        }
    }

    // Try to parse
    if let Ok(msg) = T::decode(data) {
        Ok(DecodedMessage::Protobuf(msg))
    } else if let Some(text) = super::utils::string_from_utf8(data) {
        Ok(DecodedMessage::Text(text))
    } else {
        Err(DecoderError::Internal("decode error"))
    }
}
