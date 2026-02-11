//! gRPC streaming message decoder
//!
//! Provides high-performance gRPC streaming message parsing with gzip compression support.
//!
//! # Example
//!
//! ```no_run
//! use grpc_stream_decoder::StreamDecoder;
//! use prost::Message;
//!
//! #[derive(Message, Default)]
//! struct MyMessage {
//!     #[prost(string, tag = "1")]
//!     content: String,
//! }
//!
//! let mut decoder = StreamDecoder::<MyMessage>::new();
//!
//! // Received data chunk
//! let chunk = receive_data();
//! let messages = decoder.decode(&chunk);
//!
//! for msg in messages {
//!     println!("{}", msg.content);
//! }
//! ```

#![allow(internal_features)]
#![feature(core_intrinsics)]

mod frame;
mod buffer;
mod compression;
mod decoder;

// Public API
pub use frame::RawMessage;
pub use buffer::Buffer;
pub use compression::{compress_gzip, decompress_gzip};
pub use decoder::StreamDecoder;

// Constants
/// Maximum decompressed message size limit (4 MiB)
///
/// Aligned with gRPC standard default max message size, prevents memory abuse attacks
pub const MAX_DECOMPRESSED_SIZE_BYTES: usize = 0x400000; // 4 * 1024 * 1024
