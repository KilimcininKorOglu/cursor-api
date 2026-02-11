use alloc::borrow::Cow;

use crate::common::model::{ApiStatus, GenericError};

const SIZE_LIMIT_MSG: &str = "Message exceeds 4 MiB size limit";

#[derive(Debug)]
pub struct ExceedSizeLimit;

impl ExceedSizeLimit {
    #[inline]
    pub const fn message() -> &'static str { SIZE_LIMIT_MSG }

    #[inline]
    pub const fn into_generic(self) -> GenericError {
        GenericError {
            status: ApiStatus::Error,
            code: Some(http::StatusCode::PAYLOAD_TOO_LARGE),
            error: Some(Cow::Borrowed("resource_exhausted")),
            message: Some(Cow::Borrowed(SIZE_LIMIT_MSG)),
        }
    }

    #[inline]
    pub const fn into_response_tuple(self) -> (http::StatusCode, axum::Json<GenericError>) {
        (http::StatusCode::PAYLOAD_TOO_LARGE, axum::Json(self.into_generic()))
    }
}

impl core::fmt::Display for ExceedSizeLimit {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(SIZE_LIMIT_MSG)
    }
}

impl std::error::Error for ExceedSizeLimit {}

impl axum::response::IntoResponse for ExceedSizeLimit {
    #[inline]
    fn into_response(self) -> axum::response::Response {
        self.into_response_tuple().into_response()
    }
}

/// Try to compress data, only return compressed result if it's smaller
///
/// Compression decision logic:
/// 1. Data ≤ 1KB → no compression (overhead > benefit)
/// 2. Compressed size ≥ original → no compression (ineffective)
/// 3. Otherwise return compressed data
#[inline]
fn try_compress_if_beneficial(data: &[u8]) -> Option<Vec<u8>> {
    const COMPRESSION_THRESHOLD: usize = 1024; // 1KB

    // Small data not compressed
    if data.len() <= COMPRESSION_THRESHOLD {
        return None;
    }

    let compressed = grpc_stream::compress_gzip(data);

    // Only return if compression is effective
    if compressed.len() < data.len() { Some(compressed) } else { None }
}

/// Encode protobuf message with automatic compression optimization
///
/// Automatically select optimal encoding method based on message size and compression effectiveness:
/// - Small message (≤1KB): return raw encoding directly
/// - Large message: try gzip compression, only use if effective
///
/// # Arguments
/// * `message` - protobuf message implementing `prost::Message`
///
/// # Returns
/// - `Ok((data, is_compressed))` - Encoding successful
///   - `data`: Encoded byte data (may already be compressed)
///   - `is_compressed`: `true` means returned data is compressed, `false` means raw encoding
/// - `Err(&str)` - Message exceeds 4MiB size limit
///
/// # Errors
/// Returns error when message encoded length exceeds `MAX_DECOMPRESSED_SIZE_BYTES` (4MiB)
///
/// # Example
/// ```ignore
/// let msg = MyMessage { field: 42 };
/// let (data, compressed) = encode_message(&msg)?;
/// if compressed {
///     println!("Using compression, saving space");
/// }
/// ```
#[inline(always)]
pub fn encode_message(message: &impl ::prost::Message) -> Result<(Vec<u8>, bool), ExceedSizeLimit> {
    let estimated_size = message.encoded_len();

    // Check if message size exceeds limit
    if estimated_size > grpc_stream::MAX_DECOMPRESSED_SIZE_BYTES {
        __cold_path!();
        return Err(ExceedSizeLimit);
    }

    // Encode to Vec
    let mut encoded = Vec::with_capacity(estimated_size);
    message.encode_raw(&mut encoded);

    // Try compression and return optimal result
    if let Some(compressed) = try_compress_if_beneficial(&encoded) {
        Ok((compressed, true))
    } else {
        Ok((encoded, false))
    }
}

/// Encode protobuf message to frame format with protocol header
///
/// Generate complete protocol frame with metadata, suitable for streaming scenarios.
///
/// # Protocol Format
/// ```text
/// [compression flag 1B][message length 4B BE][message body/compressed data]
/// ```
/// - **Byte 0**: Compression flag (`0x00`=uncompressed, `0x01`=gzip compressed)
/// - **Bytes 1-4**: Message body length, big-endian u32
/// - **Bytes 5+**: Actual message data
///
/// # Compression Strategy
/// Same as `encode_message`: small messages not compressed, fallback to raw data if compression ineffective
///
/// # Arguments
/// * `message` - protobuf message implementing `prost::Message`
///
/// # Returns
/// - `Ok(framed_data)` - Complete protocol frame data
/// - `Err(&str)` - Message exceeds 4MiB size limit
///
/// # Errors
/// Returns error when message encoded length exceeds `MAX_DECOMPRESSED_SIZE_BYTES` (4MiB)
///
/// # Safety
/// Internal use of `MaybeUninit` and unsafe code to optimize performance, but guarantee memory safety:
/// - All write operations within bounds
/// - Ensure all data is initialized before returning
///
/// # Example
/// ```ignore
/// let msg = MyMessage { field: 42 };
/// let frame = encode_message_framed(&msg)?;
/// // frame can be written directly to network stream
/// stream.write_all(&frame)?;
/// ```
#[inline(always)]
pub fn encode_message_framed(message: &impl ::prost::Message) -> Result<Vec<u8>, ExceedSizeLimit> {
    let estimated_size = message.encoded_len();

    // Check if message size exceeds limit (4MiB much smaller than u32::MAX-5, no need for additional protocol limit check)
    if estimated_size > grpc_stream::MAX_DECOMPRESSED_SIZE_BYTES {
        __cold_path!();
        return Err(ExceedSizeLimit);
    }

    use ::core::mem::MaybeUninit;

    // Allocate uninitialized buffer: [5-byte header][message body]
    // Use MaybeUninit to avoid unnecessary zero initialization
    let mut buffer = Vec::<MaybeUninit<u8>>::with_capacity(5 + estimated_size);

    unsafe {
        // Pre-set length (content to be initialized)
        buffer.set_len(5 + estimated_size);

        // Get pointers to header and message body
        let header_ptr: *mut u8 = buffer.as_mut_ptr().cast();
        let body_ptr = header_ptr.add(5);

        // Encode message body to offset 5
        message.encode_raw(&mut ::core::slice::from_raw_parts_mut(body_ptr, estimated_size));

        // Try to compress message body
        let body_slice = ::core::slice::from_raw_parts(body_ptr, estimated_size);
        let (compression_flag, final_len) =
            if let Some(compressed) = try_compress_if_beneficial(body_slice) {
                let compressed_len = compressed.len();

                // When compression succeeds, message length must be < original length ≤ 4MiB
                ::core::hint::assert_unchecked(compressed_len < estimated_size);

                // Overwrite original message body with compressed data
                ::core::ptr::copy_nonoverlapping(compressed.as_ptr(), body_ptr, compressed_len);

                // Truncate buffer to actual used length
                buffer.set_len(5 + compressed_len);

                (0x01, compressed_len)
            } else {
                // Compression ineffective, use original data
                (0x00, estimated_size)
            };

        // Write protocol header
        // Byte 0: compression flag
        *header_ptr = compression_flag;

        // Bytes 1-4: message length (big-endian)
        let len_bytes = (final_len as u32).to_be_bytes();
        ::core::ptr::copy_nonoverlapping(len_bytes.as_ptr(), header_ptr.add(1), 4);

        // At this point all buffer data is initialized, safe to convert to Vec<u8>
        #[allow(clippy::missing_transmute_annotations)]
        Ok(::core::intrinsics::transmute(buffer))
    }
}
