// Copyright Mozilla Foundation. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(all(
    feature = "nightly",
    any(
        target_feature = "sse2",
        all(target_endian = "little", target_arch = "aarch64"),
        all(target_endian = "little", target_feature = "neon")
    )
))]
#[allow(unused_imports)]
use super::simd_funcs::*;

// Safety invariants for masks: data & mask = 0 for valid ASCII or basic latin utf-16

// `as` truncates, so works on 32-bit, too.
#[allow(dead_code)]
pub const ASCII_MASK: usize = 0x8080_8080_8080_8080u64 as usize;

// ----------------------------------------------------------------------------------
// Architecture Constants Definition
// ----------------------------------------------------------------------------------

cfg_if! {
    if #[cfg(all(feature = "nightly", target_endian = "little", target_arch = "aarch64"))] {
        pub const ALU_STRIDE_SIZE: usize = 16;
        pub const ALU_ALIGNMENT: usize = 8;
        pub const ALU_ALIGNMENT_MASK: usize = 7;
    } else if #[cfg(all(feature = "nightly", target_endian = "little", target_feature = "neon"))] {
        // Even with NEON enabled, we use the ALU path for ASCII validation.
        // Merged definition from the bottom of the original file for clarity.
        pub const ALU_STRIDE_SIZE: usize = 8;
        pub const ALU_ALIGNMENT: usize = 4;
        pub const ALU_ALIGNMENT_MASK: usize = 3;
    } else if #[cfg(all(feature = "nightly", target_feature = "sse2"))] {
        pub const SIMD_STRIDE_SIZE: usize = 16;
        /// Safety-usable invariant: This should be identical to SIMD_STRIDE_SIZE
        pub const SIMD_ALIGNMENT: usize = 16;
        pub const SIMD_ALIGNMENT_MASK: usize = 15;
    } else if #[cfg(all(target_endian = "little", target_pointer_width = "64"))] {
        // Aligned ALU word, little-endian, 64-bit
        pub const ALU_STRIDE_SIZE: usize = 16;
        pub const ALU_ALIGNMENT: usize = 8;
        pub const ALU_ALIGNMENT_MASK: usize = 7;
    } else if #[cfg(all(target_endian = "little", target_pointer_width = "32"))] {
        // Aligned ALU word, little-endian, 32-bit
        pub const ALU_STRIDE_SIZE: usize = 8;
        pub const ALU_ALIGNMENT: usize = 4;
        pub const ALU_ALIGNMENT_MASK: usize = 3;
    } else if #[cfg(all(target_endian = "big", target_pointer_width = "64"))] {
        // Aligned ALU word, big-endian, 64-bit
        pub const ALU_STRIDE_SIZE: usize = 16;
        pub const ALU_ALIGNMENT: usize = 8;
        pub const ALU_ALIGNMENT_MASK: usize = 7;
    } else if #[cfg(all(target_endian = "big", target_pointer_width = "32"))] {
        // Aligned ALU word, big-endian, 32-bit
        pub const ALU_STRIDE_SIZE: usize = 8;
        pub const ALU_ALIGNMENT: usize = 4;
        pub const ALU_ALIGNMENT_MASK: usize = 3;
    } else {
        // Fallback / Naive defaults if strictly needed, though ALU logic is gated below
    }
}

// ----------------------------------------------------------------------------------
// Helper Functions
// ----------------------------------------------------------------------------------

cfg_if! {
    // Safety-usable invariant: this counts the zeroes from the "first byte" of utf-8 data packed into a usize
    // with the target endianness
    if #[cfg(target_endian = "little")] {
        #[allow(dead_code)]
        #[inline(always)]
        fn count_zeros(word: usize) -> u32 {
            word.trailing_zeros()
        }
    } else {
        #[allow(dead_code)]
        #[inline(always)]
        fn count_zeros(word: usize) -> u32 {
            word.leading_zeros()
        }
    }
}

// ----------------------------------------------------------------------------------
// validate_ascii Implementation
// ----------------------------------------------------------------------------------

cfg_if! {
    if #[cfg(all(feature = "nightly", target_feature = "sse2"))] {
        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found
        #[inline(always)]
        pub fn validate_ascii(slice: &[u8]) -> Option<(u8, usize)> {
            let src = slice.as_ptr();
            let len = slice.len();
            let mut offset = 0usize;
            // Safety: if this check succeeds we're valid for reading at least `stride` elements.
            if SIMD_STRIDE_SIZE <= len {
                // First, process one unaligned vector
                // Safety: src is valid for a `SIMD_STRIDE_SIZE` read
                let simd = unsafe { load16_unaligned(src) };
                let mask = mask_ascii(simd);
                if mask != 0 {
                    offset = mask.trailing_zeros() as usize;
                    let non_ascii = unsafe { *src.add(offset) };
                    return Some((non_ascii, offset));
                }
                offset = SIMD_STRIDE_SIZE;
                // Safety: Now that offset has changed we don't yet know how much it is valid for

                // We have now seen 16 ASCII bytes. Let's guess that
                // there will be enough more to justify more expense
                // in the case of non-ASCII.
                // Use aligned reads for the sake of old microachitectures.
                // Safety: this correctly calculates the number of src_units that need to be read before the remaining list is aligned.
                // This is by definition less than SIMD_ALIGNMENT, which is defined to be equal to SIMD_STRIDE_SIZE.
                let until_alignment = unsafe { (SIMD_ALIGNMENT - ((src.add(offset) as usize) & SIMD_ALIGNMENT_MASK)) & SIMD_ALIGNMENT_MASK };
                // This addition won't overflow, because even in the 32-bit PAE case the
                // address space holds enough code that the slice length can't be that
                // close to address space size.
                // offset now equals SIMD_STRIDE_SIZE, hence times 3 below.
                //
                // Safety: if this check succeeds we're valid for reading at least `2 * SIMD_STRIDE_SIZE` elements plus `until_alignment`.
                // The extra SIMD_STRIDE_SIZE in the condition is because `offset` is already `SIMD_STRIDE_SIZE`.
                if until_alignment + (SIMD_STRIDE_SIZE * 3) <= len {
                    if until_alignment != 0 {
                        // Safety: this is safe to call since we're valid for this read (and more), and don't care about alignment
                        // This will copy over bytes that get decoded twice since it's not incrementing `offset` by SIMD_STRIDE_SIZE. This is fine.
                        let simd = unsafe { load16_unaligned(src.add(offset)) };
                        let mask = mask_ascii(simd);
                        if mask != 0 {
                            offset += mask.trailing_zeros() as usize;
                            let non_ascii = unsafe { *src.add(offset) };
                            return Some((non_ascii, offset));
                        }
                        offset += until_alignment;
                    }
                    // Safety: At this point we're valid for reading 2*SIMD_STRIDE_SIZE elements
                    // Safety: Now `offset` is aligned for `src`
                    let len_minus_stride_times_two = len - (SIMD_STRIDE_SIZE * 2);
                    loop {
                        // Safety: We were valid for this read, and were aligned.
                        let first = unsafe { load16_aligned(src.add(offset)) };
                        let second = unsafe { load16_aligned(src.add(offset + SIMD_STRIDE_SIZE)) };
                        if !simd_is_ascii(first | second) {
                            // Safety: mask_ascii produces a mask of all the high bits.
                            let mask_first = mask_ascii(first);
                            if mask_first != 0 {
                                // Safety: on little endian systems this will be the number of ascii bytes
                                // before the first non-ascii, i.e. valid for indexing src
                                // TODO SAFETY: What about big-endian systems?
                                offset += mask_first.trailing_zeros() as usize;
                            } else {
                                let mask_second = mask_ascii(second);
                                // Safety: on little endian systems this will be the number of ascii bytes
                                // before the first non-ascii, i.e. valid for indexing src
                                offset += SIMD_STRIDE_SIZE + mask_second.trailing_zeros() as usize;
                            }
                            // Safety: We know this is non-ASCII, and can uphold the safety-usable invariant here
                            let non_ascii = unsafe { *src.add(offset) };

                            return Some((non_ascii, offset));
                        }
                        offset += SIMD_STRIDE_SIZE * 2;
                        // Safety: This is `offset > len - 2 * SIMD_STRIDE_SIZE` which means we always have at least `2 * SIMD_STRIDE_SIZE` elements to munch next time.
                        if offset > len_minus_stride_times_two {
                            break;
                        }
                    }
                    // Safety: if this check succeeds we're valid for reading at least `SIMD_STRIDE_SIZE`
                    if offset + SIMD_STRIDE_SIZE <= len {
                        // Safety: We were valid for this read, and were aligned.
                        let simd = unsafe { load16_aligned(src.add(offset)) };
                        // Safety: mask_ascii produces a mask of all the high bits.
                        let mask = mask_ascii(simd);
                        if mask != 0 {
                            // Safety: on little endian systems this will be the number of ascii bytes
                            // before the first non-ascii, i.e. valid for indexing src
                            offset += mask.trailing_zeros() as usize;
                            let non_ascii = unsafe { *src.add(offset) };
                            // Safety: We know this is non-ASCII, and can uphold the safety-usable invariant here
                            return Some((non_ascii, offset));
                        }
                        offset += SIMD_STRIDE_SIZE;
                    }
                } else {
                    // Safety: this is the unaligned branch
                    // At most two iterations, so unroll
                    // Safety: if this check succeeds we're valid for reading at least `SIMD_STRIDE_SIZE`
                    if offset + SIMD_STRIDE_SIZE <= len {
                        // Safety: We're valid for this read but must use an unaligned read
                        let simd = unsafe { load16_unaligned(src.add(offset)) };
                        let mask = mask_ascii(simd);
                        if mask != 0 {
                            offset += mask.trailing_zeros() as usize;
                            let non_ascii = unsafe { *src.add(offset) };
                            // Safety-usable invariant upheld here (same as above)
                            return Some((non_ascii, offset));
                        }
                        offset += SIMD_STRIDE_SIZE;
                        // Safety: if this check succeeds we're valid for reading at least `SIMD_STRIDE_SIZE`
                        if offset + SIMD_STRIDE_SIZE <= len {
                            // Safety: We're valid for this read but must use an unaligned read
                             let simd = unsafe { load16_unaligned(src.add(offset)) };
                             let mask = mask_ascii(simd);
                            if mask != 0 {
                                offset += mask.trailing_zeros() as usize;
                                let non_ascii = unsafe { *src.add(offset) };
                                // Safety-usable invariant upheld here (same as above)
                                return Some((non_ascii, offset));
                            }
                            offset += SIMD_STRIDE_SIZE;
                        }
                    }
                }
            }
            while offset < len {
                // Safety: relies straightforwardly on the `len` invariant
                let code_unit = unsafe { *(src.add(offset)) };
                if code_unit > 127 {
                    // Safety-usable invariant upheld here
                    return Some((code_unit, offset));
                }
                offset += 1;
            }
            None
        }
    } else {
        // Generic ALU Implementation (also used for NEON and aarch64 in this file)

        // Safety-usable invariant: returns byte index of first non-ascii byte
        #[inline(always)]
        fn find_non_ascii(word: usize, second_word: usize) -> Option<usize> {
            let word_masked = word & ASCII_MASK;
            let second_masked = second_word & ASCII_MASK;
            if (word_masked | second_masked) == 0 {
                // Both are ascii, invariant upheld
                return None;
            }
            if word_masked != 0 {
                let zeros = count_zeros(word_masked);
                // `zeros` now contains 0 to 7 (for the seven bits of masked ASCII in little endian,
                // or up to 7 bits of non-ASCII in big endian if the first byte is non-ASCII)
                // plus 8 times the number of ASCII in text order before the
                // non-ASCII byte in the little-endian case or 8 times the number of ASCII in
                // text order before the non-ASCII byte in the big-endian case.
                let num_ascii = (zeros >> 3) as usize;
                // Safety-usable invariant upheld here
                return Some(num_ascii);
            }
            let zeros = count_zeros(second_masked);
            // `zeros` now contains 0 to 7 (for the seven bits of masked ASCII in little endian,
            // or up to 7 bits of non-ASCII in big endian if the first byte is non-ASCII)
            // plus 8 times the number of ASCII in text order before the
            // non-ASCII byte in the little-endian case or 8 times the number of ASCII in
            // text order before the non-ASCII byte in the big-endian case.
            let num_ascii = (zeros >> 3) as usize;
            // Safety-usable invariant upheld here
            Some(ALU_ALIGNMENT + num_ascii)
        }

        /// Safety: `src` must be valid for the reads of two `usize`s
        ///
        /// Safety-usable invariant: will return byte index of first non-ascii byte
        #[inline(always)]
        unsafe fn validate_ascii_stride(src: *const usize) -> Option<usize> {
            let word = *src;
            let second_word = *(src.add(1));
            find_non_ascii(word, second_word)
        }

        /// Safety-usable invariant: will return Some() when it encounters non-ASCII, with the first element in the Some being
        /// guaranteed to be non-ASCII (> 127), and the second being the offset where it is found
        #[allow(clippy::cast_ptr_alignment)]
        #[inline(always)]
        pub fn validate_ascii(slice: &[u8]) -> Option<(u8, usize)> {
            let src = slice.as_ptr();
            let len = slice.len();
            let mut offset = 0usize;
            let mut until_alignment = (ALU_ALIGNMENT - ((src as usize) & ALU_ALIGNMENT_MASK)) & ALU_ALIGNMENT_MASK;
            // Safety: If this check fails we're valid to read `until_alignment + ALU_STRIDE_SIZE` elements
            if until_alignment + ALU_STRIDE_SIZE <= len {
                while until_alignment != 0 {
                    let code_unit = slice[offset];
                    if code_unit > 127 {
                        // Safety-usable invairant upheld here
                        return Some((code_unit, offset));
                    }
                    offset += 1;
                    until_alignment -= 1;
                }
                // Safety: At this point we have read until_alignment elements and
                // are valid for `ALU_STRIDE_SIZE` more.
                let len_minus_stride = len - ALU_STRIDE_SIZE;
                loop {
                    // Safety: we were valid for this read
                    let ptr = unsafe { src.add(offset) as *const usize };
                    if let Some(num_ascii) = unsafe { validate_ascii_stride(ptr) } {
                        offset += num_ascii;
                        // Safety-usable invairant upheld here using the invariant from validate_ascii_stride()
                        return Some((unsafe { *(src.add(offset)) }, offset));
                    }
                    offset += ALU_STRIDE_SIZE;
                    // Safety: This is `offset > ALU_STRIDE_SIZE` which means we always have at least `2 * ALU_STRIDE_SIZE` elements to munch next time.
                    if offset > len_minus_stride {
                        break;
                    }
                }
            }
            while offset < len {
                let code_unit = slice[offset];
                if code_unit > 127 {
                    // Safety-usable invairant upheld here
                    return Some((code_unit, offset));
                }
                offset += 1;
           }
           None
        }
    }
}

// ----------------------------------------------------------------------------------
// Public API
// ----------------------------------------------------------------------------------

// pub fn ascii_valid_up_to(bytes: &[u8]) -> usize {
//     match validate_ascii(bytes) {
//         None => bytes.len(),
//         Some((_, num_valid)) => num_valid,
//     }
// }

#[inline(always)]
pub fn is_valid_ascii(v: &[u8]) -> bool { validate_ascii(v).is_none() }
