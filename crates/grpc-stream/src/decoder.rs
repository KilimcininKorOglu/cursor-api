//! Streaming message decoder

use prost::Message;

use crate::buffer::Buffer;
use crate::compression::decompress_gzip;
use crate::frame::RawMessage;

/// gRPC streaming message decoder
///
/// Processes incremental data chunks, parses complete Protobuf messages.
///
/// # Example
///
/// ```no_run
/// use grpc_stream_decoder::StreamDecoder;
/// use prost::Message;
///
/// #[derive(Message, Default)]
/// struct MyMessage {
///     #[prost(string, tag = "1")]
///     content: String,
/// }
///
/// let mut decoder = StreamDecoder::new();
///
/// // Using default processor
/// loop {
///     let chunk = receive_network_data();
///     let messages: Vec<MyMessage> = decoder.decode_default(&chunk);
///     
///     for msg in messages {
///         process(msg);
///     }
/// }
///
/// // Using custom processor
/// let messages = decoder.decode(&chunk, |raw_msg| {
///     // Custom decoding logic
///     match raw_msg.r#type {
///         0 => MyMessage::decode(raw_msg.data).ok(),
///         _ => None,
///     }
/// });
/// ```
pub struct StreamDecoder {
    buffer: Buffer,
}

impl StreamDecoder {
    /// Create new decoder
    #[inline]
    pub fn new() -> Self { Self { buffer: Buffer::new() } }

    /// Decode data chunk with custom processor
    ///
    /// # Type Parameters
    /// - `T`: Target message type
    /// - `F`: Processor function, signature is `Fn(RawMessage<'_>) -> Option<T>`
    ///
    /// # Parameters
    /// - `data`: Received data chunk
    /// - `processor`: Custom processor function, receives raw message and returns decode result
    ///
    /// # Returns
    /// List of successfully decoded messages
    ///
    /// # Example
    ///
    /// ```no_run
    /// // Custom processing: only accept uncompressed messages
    /// let messages = decoder.decode(&data, |raw_msg| {
    ///     if raw_msg.r#type == 0 {
    ///         MyMessage::decode(raw_msg.data).ok()
    ///     } else {
    ///         None
    ///     }
    /// });
    /// ```
    pub fn decode<T, F>(&mut self, data: &[u8], processor: F) -> Vec<T>
    where F: Fn(RawMessage<'_>) -> Option<T> {
        self.buffer.extend_from_slice(data);

        let mut iter = (&self.buffer).into_iter();
        let exact_count = iter.len();
        let mut messages = Vec::with_capacity(exact_count);

        for raw_msg in &mut iter {
            if let Some(msg) = processor(raw_msg) {
                messages.push(msg);
            }
        }

        unsafe { self.buffer.advance_unchecked(iter.offset()) };
        messages
    }

    /// Decode data chunk with default processor
    ///
    /// Default behavior:
    /// - Type 0: Directly decode Protobuf message
    /// - Type 1: First gzip decompress, then decode
    /// - Other types: Ignore
    ///
    /// # Type Parameters
    /// - `T`: Message type implementing `prost::Message + Default`
    ///
    /// # Parameters
    /// - `data`: Received data chunk
    ///
    /// # Returns
    /// List of successfully decoded messages
    pub fn decode_default<T: Message + Default>(&mut self, data: &[u8]) -> Vec<T> {
        self.decode(data, |raw_msg| match raw_msg.r#type {
            0 => Self::decode_message(raw_msg.data),
            1 => Self::decode_compressed_message(raw_msg.data),
            _ => None,
        })
    }

    /// Decode uncompressed message
    #[inline]
    fn decode_message<T: Message + Default>(data: &[u8]) -> Option<T> { T::decode(data).ok() }

    /// Decode gzip compressed message
    #[inline]
    fn decode_compressed_message<T: Message + Default>(data: &[u8]) -> Option<T> {
        let decompressed = decompress_gzip(data)?;
        Self::decode_message(&decompressed)
    }
}

impl Default for StreamDecoder {
    fn default() -> Self { Self::new() }
}
