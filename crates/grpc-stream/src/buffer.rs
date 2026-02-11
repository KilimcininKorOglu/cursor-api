//! Internal buffer management

use crate::frame::RawMessage;
use core::iter::FusedIterator;

/// Message buffer (internal use)
pub struct Buffer {
    inner: Vec<u8>,
    cursor: usize,
}

impl Buffer {
    #[inline]
    pub fn new() -> Self { Self { inner: Vec::new(), cursor: 0 } }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: Vec::with_capacity(capacity), cursor: 0 }
    }

    #[inline]
    pub fn len(&self) -> usize { unsafe { self.inner.len().unchecked_sub(self.cursor) } }

    #[inline]
    pub fn is_empty(&self) -> bool { self.len() == 0 }

    #[inline]
    pub fn extend_from_slice(&mut self, data: &[u8]) {
        self.try_reclaim();
        self.inner.extend_from_slice(data)
    }

    #[inline]
    pub unsafe fn advance_unchecked(&mut self, cnt: usize) {
        self.cursor = unsafe { self.cursor.unchecked_add(cnt) }
    }

    #[inline]
    /// reset if empty
    fn try_reclaim(&mut self) {
        if self.is_empty() {
            self.inner.clear();
            self.cursor = 0
        }
    }
}

impl Default for Buffer {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl AsRef<[u8]> for Buffer {
    #[inline]
    fn as_ref(&self) -> &[u8] { unsafe { self.inner.get_unchecked(self.cursor..) } }
}

/// Message iterator (internal use)
#[derive(Debug, Clone)]
pub struct MessageIter<'b> {
    buffer: &'b [u8],
    offset: usize,
}

impl<'b> MessageIter<'b> {
    /// Return current consumed byte count
    #[inline]
    pub fn offset(&self) -> usize { self.offset }
}

impl<'b> Iterator for MessageIter<'b> {
    type Item = RawMessage<'b>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Need at least 5 bytes (1 byte type + 4 bytes length)
        if self.offset + 5 > self.buffer.len() {
            return None;
        }

        let r#type = unsafe {
            let ptr: *const u8 =
                ::core::intrinsics::slice_get_unchecked(self.buffer as *const [u8], self.offset);
            *ptr
        };
        let msg_len = u32::from_be_bytes(unsafe {
            *get_offset_len_noubcheck(self.buffer, self.offset + 1, 4).cast()
        }) as usize;

        // Check if message is complete
        if self.offset + 5 + msg_len > self.buffer.len() {
            return None;
        }

        self.offset += 5;

        let data = unsafe { &*get_offset_len_noubcheck(self.buffer, self.offset, msg_len) };

        self.offset += msg_len;

        Some(RawMessage { r#type, data })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.len();
        (count, Some(count)) // Exact value
    }
}

// Implement ExactSizeIterator
impl<'b> ExactSizeIterator for MessageIter<'b> {
    #[inline]
    fn len(&self) -> usize {
        // Precisely calculate remaining complete message count
        let mut count = 0;
        let mut offset = self.offset;

        while offset + 5 <= self.buffer.len() {
            let msg_len = u32::from_be_bytes(unsafe {
                *get_offset_len_noubcheck(self.buffer, offset + 1, 4).cast()
            }) as usize;

            if offset + 5 + msg_len > self.buffer.len() {
                break;
            }

            count += 1;
            offset += 5 + msg_len;
        }

        count
    }
}

// Implement FusedIterator
impl<'b> FusedIterator for MessageIter<'b> {}

impl<'b> IntoIterator for &'b Buffer {
    type Item = RawMessage<'b>;
    type IntoIter = MessageIter<'b>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter { MessageIter { buffer: self.as_ref(), offset: 0 } }
}

#[inline(always)]
const unsafe fn get_offset_len_noubcheck<T>(
    ptr: *const [T],
    offset: usize,
    len: usize,
) -> *const [T] {
    let ptr = ptr as *const T;
    // SAFETY: The caller already checked these preconditions
    let ptr = unsafe { ::core::intrinsics::offset(ptr, offset) };
    ::core::intrinsics::aggregate_raw_ptr(ptr, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_size_iterator() {
        let mut buffer = Buffer::new();

        // Construct two messages: type=0, len=3, data="abc"
        buffer.extend_from_slice(&[0, 0, 0, 0, 3, b'a', b'b', b'c']);
        buffer.extend_from_slice(&[0, 0, 0, 0, 2, b'x', b'y']);

        let iter = (&buffer).into_iter();

        // Verify ExactSizeIterator
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.size_hint(), (2, Some(2)));

        let messages: Vec<_> = iter.collect();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_fused_iterator() {
        let buffer = Buffer::new(); // Empty buffer

        let mut iter = (&buffer).into_iter();

        // Verify FusedIterator
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None); // Still None
        assert_eq!(iter.next(), None); // Always None
    }

    #[test]
    fn test_clone_iterator() {
        let mut buffer = Buffer::new();
        buffer.extend_from_slice(&[0, 0, 0, 0, 3, b'a', b'b', b'c']);

        let iter = (&buffer).into_iter();
        let iter_clone = iter.clone();

        // Consume original iterator
        assert_eq!(iter.count(), 1);

        // Clone is still usable
        assert_eq!(iter_clone.count(), 1);
    }
}
