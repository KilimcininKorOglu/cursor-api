//! Raw message frame definition

/// Raw frame of gRPC streaming message
///
/// Contains frame header information and reference to message data.
///
/// # Frame Format
///
/// ```text
/// +------+----------+----------------+
/// | type | length   | data           |
/// | 1B   | 4B (BE)  | length bytes   |
/// +------+----------+----------------+
/// ```
///
/// - `type`: Message type
///   - `0`: Uncompressed
///   - `1`: gzip compressed
/// - `length`: Message body length (big-endian)
/// - `data`: Message body data
///
/// # Field Description
///
/// - `r#type`: Frame type flag (0=uncompressed, 1=gzip)
/// - `data`: Message body data slice, its length can be obtained via `data.len()`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawMessage<'b> {
    /// Message type (0=uncompressed, 1=gzip)
    pub r#type: u8,

    /// Message body data
    pub data: &'b [u8],
}

impl RawMessage<'_> {
    /// Calculate total bytes this message occupies in buffer
    ///
    /// Includes 5-byte frame header + message body length
    ///
    /// # Example
    ///
    /// ```
    /// # use grpc_stream_decoder::RawMessage;
    /// let msg = RawMessage {
    ///     r#type: 0,
    ///     data: &[1, 2, 3],
    /// };
    /// assert_eq!(msg.total_size(), 8); // 5 + 3
    /// ```
    #[inline]
    pub const fn total_size(&self) -> usize {
        5 + self.data.len()
    }
}
