//! Compressed data handling

use crate::MAX_DECOMPRESSED_SIZE_BYTES;
use flate2::read::GzDecoder;
use std::io::Read as _;

/// Compress data to gzip format
///
/// Uses fixed compression level 6, pre-allocates capacity to reduce memory allocation
#[inline]
pub fn compress_gzip(data: &[u8]) -> Vec<u8> {
    use ::std::io::Write as _;
    use flate2::{Compression, write::GzEncoder};

    const LEVEL: Compression = Compression::new(6);

    // Pre-allocate capacity: assume 50% compression ratio + gzip header ~18 bytes
    let estimated_size = data.len() / 2 + 18;
    let mut encoder = GzEncoder::new(Vec::with_capacity(estimated_size), LEVEL);

    // Writing to Vec won't fail, safe to unwrap
    unsafe {
        encoder.write_all(data).unwrap_unchecked();
        encoder.finish().unwrap_unchecked()
    }
}

/// Decompress gzip data
///
/// # Parameters
/// - `data`: gzip compressed data
///
/// # Returns
/// - `Some(Vec<u8>)`: Decompression successful
/// - `None`: Not valid gzip data or decompression failed
///
/// # Minimum GZIP File Structure
///
/// ```text
/// +----------+-------------+----------+
/// | Header   | DEFLATE     | Footer   |
/// | 10 bytes | 2+ bytes    | 8 bytes  |
/// +----------+-------------+----------+
/// Minimum: 10 + 2 + 8 = 20 bytes
/// ```
///
/// # Security
/// - Limits decompressed size to not exceed `MAX_DECOMPRESSED_SIZE_BYTES`
/// - Prevents gzip bomb attacks
pub fn decompress_gzip(data: &[u8]) -> Option<Vec<u8>> {
    // Fast path: reject obviously invalid data
    // Minimum valid gzip file is 20 bytes (header 10 + data 2 + footer 8)
    if data.len() < 20 {
        return None;
    }

    // SAFETY: Already verified data.len() >= 20, guarantees indices 0, 1, 2 are valid
    // Check gzip magic number (0x1f 0x8b) and compression method (0x08 = DEFLATE)
    if unsafe {
        *data.get_unchecked(0) != 0x1f
            || *data.get_unchecked(1) != 0x8b
            || *data.get_unchecked(2) != 0x08
    } {
        return None;
    }

    // Read ISIZE from gzip footer (original size, last 4 bytes, little-endian)
    // SAFETY: Already verified data.len() >= 20, last 4 bytes are guaranteed valid
    let capacity = unsafe {
        let ptr = data.as_ptr().add(data.len() - 4) as *const [u8; 4];
        u32::from_le_bytes(ptr.read()) as usize
    };

    // Prevent decompression bomb attack
    if capacity > MAX_DECOMPRESSED_SIZE_BYTES {
        return None;
    }

    // Perform actual decompression
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::with_capacity(capacity);

    decoder.read_to_end(&mut decompressed).ok()?;

    Some(decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_short() {
        // Data less than 20 bytes should be rejected directly
        assert!(decompress_gzip(&[]).is_none());
        assert!(decompress_gzip(&[0x1f, 0x8b, 0x08]).is_none());
        assert!(decompress_gzip(&[0u8; 19]).is_none());
    }

    #[test]
    fn test_invalid_magic() {
        // Length sufficient but magic number wrong
        let mut data = vec![0u8; 20];
        data[0] = 0x00; // Wrong magic number
        data[1] = 0x8b;
        data[2] = 0x08;
        assert!(decompress_gzip(&data).is_none());

        // First byte correct, second byte wrong
        data[0] = 0x1f;
        data[1] = 0x00;
        assert!(decompress_gzip(&data).is_none());

        // First two bytes correct, compression method wrong
        data[1] = 0x8b;
        data[2] = 0x09; // Non-DEFLATE
        assert!(decompress_gzip(&data).is_none());
    }

    #[test]
    fn test_gzip_bomb_protection() {
        // Construct fake gzip data claiming to decompress to 2MB
        let mut fake_gzip = vec![0x1f, 0x8b, 0x08]; // Correct magic number
        fake_gzip.extend_from_slice(&[0u8; 14]); // Pad to 17 bytes

        // ISIZE field (last 4 bytes): 2MB
        let size_2mb = 2 * 1024 * 1024u32;
        fake_gzip.extend_from_slice(&size_2mb.to_le_bytes());

        assert_eq!(fake_gzip.len(), 21); // 17 + 4
        assert!(decompress_gzip(&fake_gzip).is_none());
    }

    #[test]
    fn test_valid_gzip() {
        // Use standard library to compress some data
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let original = b"Hello, GZIP!";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify: compressed data >= 20 bytes
        assert!(compressed.len() >= 20);

        // Decompress and verify
        let decompressed = decompress_gzip(&compressed).unwrap();
        assert_eq!(&decompressed, original);
    }

    #[test]
    fn test_empty_gzip() {
        // Compress empty data (minimum valid gzip)
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&[]).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify: minimum gzip file ~20 bytes
        assert!(compressed.len() >= 20);

        let decompressed = decompress_gzip(&compressed).unwrap();
        assert_eq!(decompressed.len(), 0);
    }
}
