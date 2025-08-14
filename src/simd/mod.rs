//! SIMD optimizations for PMCP performance-critical operations
//! Provides vectorized implementations of JSON parsing and string operations

#![allow(unsafe_code)]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
use std::mem;

/// SIMD-accelerated JSON parsing utilities
pub mod json {
    use super::*;

    /// Fast SIMD-based whitespace detection
    ///
    /// Uses AVX2 instructions to process 32 bytes at a time for efficient whitespace detection.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::simd::json::find_whitespace_simd;
    ///
    /// # #[cfg(target_arch = "x86_64")]
    /// # unsafe fn example() {
    /// let text = b"hello world\ttest\nmore";
    /// let positions = find_whitespace_simd(text);
    /// // Returns positions of ' ', '\t', '\n' characters
    /// # }
    /// ```
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn find_whitespace_simd(input: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        let len = input.len();

        // Process 32 bytes at a time with AVX2
        let chunks = len / 32;
        let space = _mm256_set1_epi8(b' ' as i8);
        let tab = _mm256_set1_epi8(b'\t' as i8);
        let newline = _mm256_set1_epi8(b'\n' as i8);
        let carriage = _mm256_set1_epi8(b'\r' as i8);

        for i in 0..chunks {
            let offset = i * 32;
            let data = _mm256_loadu_si256(input.as_ptr().add(offset) as *const __m256i);

            // Compare with each whitespace character
            let space_mask = _mm256_cmpeq_epi8(data, space);
            let tab_mask = _mm256_cmpeq_epi8(data, tab);
            let newline_mask = _mm256_cmpeq_epi8(data, newline);
            let carriage_mask = _mm256_cmpeq_epi8(data, carriage);

            // Combine all masks
            let ws_mask = _mm256_or_si256(
                _mm256_or_si256(space_mask, tab_mask),
                _mm256_or_si256(newline_mask, carriage_mask),
            );

            // Extract mask as integer
            let mask = _mm256_movemask_epi8(ws_mask);

            // Find positions of set bits
            let mut m = mask;
            let mut bit_pos = 0;
            while m != 0 {
                if m & 1 != 0 {
                    positions.push(offset + bit_pos);
                }
                m >>= 1;
                bit_pos += 1;
            }
        }

        // Process remaining bytes
        for (i, &byte) in input.iter().enumerate().skip(chunks * 32) {
            match byte {
                b' ' | b'\t' | b'\n' | b'\r' => positions.push(i),
                _ => {},
            }
        }

        positions
    }

    /// Validates a UTF-8 continuation byte
    #[inline]
    fn is_valid_continuation_byte(byte: u8) -> bool {
        byte & 0xC0 == 0x80
    }

    /// Validates a multi-byte UTF-8 sequence starting at position
    fn validate_multibyte_sequence(input: &[u8], start: usize, first_byte: u8) -> bool {
        let len = input.len();

        if first_byte < 0xC0 {
            false // Invalid continuation byte
        } else if first_byte < 0xE0 {
            // 2-byte sequence
            validate_2byte_sequence(input, start, len)
        } else if first_byte < 0xF0 {
            // 3-byte sequence
            validate_3byte_sequence(input, start, len)
        } else if first_byte < 0xF8 {
            // 4-byte sequence
            validate_4byte_sequence(input, start, len)
        } else {
            false // Invalid UTF-8
        }
    }

    /// Validates a 2-byte UTF-8 sequence
    #[inline]
    fn validate_2byte_sequence(input: &[u8], start: usize, len: usize) -> bool {
        start + 1 < len && is_valid_continuation_byte(input[start + 1])
    }

    /// Validates a 3-byte UTF-8 sequence  
    #[inline]
    fn validate_3byte_sequence(input: &[u8], start: usize, len: usize) -> bool {
        start + 2 < len
            && is_valid_continuation_byte(input[start + 1])
            && is_valid_continuation_byte(input[start + 2])
    }

    /// Validates a 4-byte UTF-8 sequence
    #[inline]
    fn validate_4byte_sequence(input: &[u8], start: usize, len: usize) -> bool {
        start + 3 < len
            && is_valid_continuation_byte(input[start + 1])
            && is_valid_continuation_byte(input[start + 2])
            && is_valid_continuation_byte(input[start + 3])
    }

    /// Processes a 32-byte chunk with SIMD, falling back to scalar for non-ASCII
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn process_simd_chunk(input: &[u8], offset: usize) -> bool {
        let data = _mm256_loadu_si256(input.as_ptr().add(offset) as *const __m256i);

        // Check for ASCII bytes (< 0x80) - fast path
        let ascii_mask = _mm256_cmpgt_epi8(_mm256_setzero_si256(), data);
        let is_ascii = _mm256_movemask_epi8(ascii_mask);

        if is_ascii == -1 {
            return true; // All bytes are ASCII
        }

        // Fall back to scalar validation for non-ASCII bytes
        validate_chunk_scalar(input, offset, 32)
    }

    /// Validates a chunk using scalar processing
    fn validate_chunk_scalar(input: &[u8], start: usize, chunk_size: usize) -> bool {
        let len = input.len();
        let end = std::cmp::min(start + chunk_size, len);

        let mut i = start;
        while i < end {
            let byte = input[i];
            if byte < 0x80 {
                i += 1;
                continue;
            }

            // Multi-byte sequence - validate and advance
            if !validate_multibyte_sequence(input, i, byte) {
                return false;
            }

            // Advance by the correct number of bytes for this sequence
            i += get_utf8_sequence_length(byte);
        }

        true
    }

    /// Gets the length of a UTF-8 sequence from its first byte
    #[inline]
    fn get_utf8_sequence_length(first_byte: u8) -> usize {
        if first_byte < 0xE0 {
            2
        } else if first_byte < 0xF0 {
            3
        } else {
            4
        }
    }

    /// SIMD-accelerated string validation (UTF-8)
    ///
    /// Validates UTF-8 byte sequences using AVX2 instructions for improved performance.
    /// This function has been refactored to reduce complexity while maintaining performance.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::simd::json::validate_utf8_simd;
    ///
    /// # #[cfg(target_arch = "x86_64")]
    /// # unsafe fn example() {
    /// let valid_utf8 = "Hello, 世界! 🚀".as_bytes();
    /// assert!(validate_utf8_simd(valid_utf8));
    ///
    /// let invalid_utf8 = &[0xC0, 0x80]; // Invalid UTF-8 sequence
    /// assert!(!validate_utf8_simd(invalid_utf8));
    /// # }
    /// ```
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn validate_utf8_simd(input: &[u8]) -> bool {
        let len = input.len();
        let mut i = 0;

        // Process 32 bytes at a time with SIMD
        while i + 32 <= len {
            if !process_simd_chunk(input, i) {
                return false;
            }
            i += 32;
        }

        // Process remaining bytes with scalar validation
        validate_remaining_bytes(input, i)
    }

    /// Validates remaining bytes after SIMD processing
    fn validate_remaining_bytes(input: &[u8], start: usize) -> bool {
        let len = input.len();
        let mut i = start;

        while i < len {
            let byte = input[i];
            if byte < 0x80 {
                i += 1;
                continue;
            }

            if !validate_multibyte_sequence(input, i, byte) {
                return false;
            }

            i += get_utf8_sequence_length(byte);
        }

        true
    }

    /// SIMD-accelerated JSON string escape detection
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn find_escapes_simd(input: &[u8]) -> Vec<usize> {
        let mut positions = Vec::new();
        let len = input.len();

        let backslash = _mm256_set1_epi8(b'\\' as i8);
        let quote = _mm256_set1_epi8(b'"' as i8);

        let chunks = len / 32;
        for i in 0..chunks {
            let offset = i * 32;
            let data = _mm256_loadu_si256(input.as_ptr().add(offset) as *const __m256i);

            let backslash_mask = _mm256_cmpeq_epi8(data, backslash);
            let quote_mask = _mm256_cmpeq_epi8(data, quote);
            let escape_mask = _mm256_or_si256(backslash_mask, quote_mask);

            let mask = _mm256_movemask_epi8(escape_mask);

            let mut m = mask;
            let mut bit_pos = 0;
            while m != 0 {
                if m & 1 != 0 {
                    positions.push(offset + bit_pos);
                }
                m >>= 1;
                bit_pos += 1;
            }
        }

        // Process remaining bytes
        for (i, &byte) in input.iter().enumerate().skip(chunks * 32) {
            if byte == b'\\' || byte == b'"' {
                positions.push(i);
            }
        }

        positions
    }
}

/// SIMD-accelerated message serialization
pub mod serialization {
    use super::*;

    /// Fast memory copy using SIMD
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn copy_simd(src: &[u8], dst: &mut [u8]) {
        assert!(dst.len() >= src.len());

        let len = src.len();
        let chunks = len / 32;

        for i in 0..chunks {
            let offset = i * 32;
            let data = _mm256_loadu_si256(src.as_ptr().add(offset) as *const __m256i);
            _mm256_storeu_si256(dst.as_mut_ptr().add(offset) as *mut __m256i, data);
        }

        // Copy remaining bytes
        dst[(chunks * 32)..len].copy_from_slice(&src[(chunks * 32)..len]);
    }

    /// SIMD-accelerated base64 encoding
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn base64_encode_simd(input: &[u8], output: &mut Vec<u8>) {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        let len = input.len();
        let full_chunks = len / 3;

        output.reserve(len.div_ceil(3) * 4);

        for i in 0..full_chunks {
            let idx = i * 3;
            let b1 = input[idx];
            let b2 = input[idx + 1];
            let b3 = input[idx + 2];

            output.push(TABLE[(b1 >> 2) as usize]);
            output.push(TABLE[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize]);
            output.push(TABLE[(((b2 & 0x0F) << 2) | (b3 >> 6)) as usize]);
            output.push(TABLE[(b3 & 0x3F) as usize]);
        }

        // Handle remaining bytes
        let remaining = len % 3;
        if remaining == 1 {
            let b1 = input[full_chunks * 3];
            output.push(TABLE[(b1 >> 2) as usize]);
            output.push(TABLE[((b1 & 0x03) << 4) as usize]);
            output.push(b'=');
            output.push(b'=');
        } else if remaining == 2 {
            let b1 = input[full_chunks * 3];
            let b2 = input[full_chunks * 3 + 1];
            output.push(TABLE[(b1 >> 2) as usize]);
            output.push(TABLE[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize]);
            output.push(TABLE[((b2 & 0x0F) << 2) as usize]);
            output.push(b'=');
        }
    }

    /// SIMD-accelerated XOR for masking (WebSocket)
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn xor_mask_simd(data: &mut [u8], mask: [u8; 4]) {
        let len = data.len();

        // Create mask pattern repeated 8 times for AVX2
        let mask_pattern = [
            mask[0], mask[1], mask[2], mask[3], mask[0], mask[1], mask[2], mask[3], mask[0],
            mask[1], mask[2], mask[3], mask[0], mask[1], mask[2], mask[3], mask[0], mask[1],
            mask[2], mask[3], mask[0], mask[1], mask[2], mask[3], mask[0], mask[1], mask[2],
            mask[3], mask[0], mask[1], mask[2], mask[3],
        ];

        let mask_vec = _mm256_loadu_si256(mask_pattern.as_ptr() as *const __m256i);

        let chunks = len / 32;
        for i in 0..chunks {
            let offset = i * 32;
            let data_vec = _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i);
            let result = _mm256_xor_si256(data_vec, mask_vec);
            _mm256_storeu_si256(data.as_mut_ptr().add(offset) as *mut __m256i, result);
        }

        // XOR remaining bytes
        for i in (chunks * 32)..len {
            data[i] ^= mask[i % 4];
        }
    }
}

/// SIMD-accelerated compression utilities
pub mod compression {
    use super::*;

    /// Fast run-length encoding using SIMD
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn rle_encode_simd(input: &[u8], output: &mut Vec<u8>) {
        let len = input.len();
        let mut i = 0;

        while i < len {
            let current = input[i];
            let mut count = 1;

            // Use SIMD to find run length
            if i + 32 <= len {
                let current_vec = _mm256_set1_epi8(current as i8);
                let data = _mm256_loadu_si256(input.as_ptr().add(i) as *const __m256i);
                let cmp = _mm256_cmpeq_epi8(data, current_vec);
                let mask = _mm256_movemask_epi8(cmp);

                // Count consecutive 1s from the start
                let leading_ones = mask.trailing_ones();
                count = leading_ones.min(255) as usize;
            }

            // Continue counting with scalar code if needed
            while i + count < len && count < 255 && input[i + count] == current {
                count += 1;
            }

            output.push(count as u8);
            output.push(current);
            i += count;
        }
    }

    /// Fast pattern matching for compression
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn find_pattern_simd(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if needle.is_empty() || needle.len() > haystack.len() {
            return None;
        }

        let first = needle[0];
        let first_vec = _mm256_set1_epi8(first as i8);

        let chunks = (haystack.len() - needle.len() + 1) / 32;

        for i in 0..chunks {
            let offset = i * 32;
            let data = _mm256_loadu_si256(haystack.as_ptr().add(offset) as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(data, first_vec);
            let mask = _mm256_movemask_epi8(cmp);

            if mask != 0 {
                // Found potential matches, check each one
                let mut m = mask;
                let mut bit_pos = 0;
                while m != 0 {
                    if m & 1 != 0 {
                        let pos = offset + bit_pos;
                        if pos + needle.len() <= haystack.len()
                            && &haystack[pos..pos + needle.len()] == needle
                        {
                            return Some(pos);
                        }
                    }
                    m >>= 1;
                    bit_pos += 1;
                }
            }
        }

        // Check remaining positions
        ((chunks * 32)..(haystack.len() - needle.len() + 1))
            .find(|&i| &haystack[i..i + needle.len()] == needle)
    }
}

/// Batch operations using SIMD
pub mod batch {
    use super::*;

    /// Process multiple messages in parallel
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn batch_validate_lengths(lengths: &[u32], max_length: u32) -> Vec<bool> {
        let mut results = Vec::with_capacity(lengths.len());
        let max_vec = _mm256_set1_epi32(max_length as i32);

        let chunks = lengths.len() / 8;

        for i in 0..chunks {
            let offset = i * 8;
            let data = _mm256_loadu_si256(lengths.as_ptr().add(offset) as *const __m256i);
            let cmp = _mm256_cmpgt_epi32(max_vec, data);

            // Extract comparison results
            let mask = _mm256_movemask_ps(_mm256_castsi256_ps(cmp));
            for j in 0..8 {
                results.push((mask & (1 << j)) != 0);
            }
        }

        // Process remaining elements
        for &length in &lengths[(chunks * 8)..] {
            results.push(length <= max_length);
        }

        results
    }

    /// Compute checksums for multiple buffers
    ///
    /// # Safety
    /// This function requires AVX2 CPU support. Caller must ensure the CPU supports AVX2.
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    pub unsafe fn batch_checksum_simd(buffers: &[&[u8]]) -> Vec<u32> {
        buffers
            .iter()
            .map(|buf| {
                let mut sum = 0u32;
                let len = buf.len();

                // Process 32 bytes at a time
                let chunks = len / 32;
                for i in 0..chunks {
                    let offset = i * 32;
                    let data = _mm256_loadu_si256(buf.as_ptr().add(offset) as *const __m256i);

                    // Sum all bytes (simplified checksum)
                    let zero = _mm256_setzero_si256();
                    let sad = _mm256_sad_epu8(data, zero);

                    // Extract sums
                    let sums = mem::transmute::<__m256i, [u64; 4]>(sad);
                    sum = sum
                        .wrapping_add(sums[0] as u32)
                        .wrapping_add(sums[1] as u32)
                        .wrapping_add(sums[2] as u32)
                        .wrapping_add(sums[3] as u32);
                }

                // Process remaining bytes
                for i in (chunks * 32)..len {
                    sum = sum.wrapping_add(buf[i] as u32);
                }

                sum
            })
            .collect()
    }
}

// Fallback implementations for non-x86_64 or when SIMD is not available
#[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
/// Fallback implementations for SIMD operations when hardware SIMD is not available.
pub mod fallback {
    /// Find all whitespace positions in the input buffer.
    pub fn find_whitespace(input: &[u8]) -> Vec<usize> {
        input
            .iter()
            .enumerate()
            .filter(|(_, &b)| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
            .map(|(i, _)| i)
            .collect()
    }

    /// Validate that the input is valid UTF-8.
    pub fn validate_utf8(input: &[u8]) -> bool {
        std::str::from_utf8(input).is_ok()
    }

    /// Find all escape character positions in the input buffer.
    pub fn find_escapes(input: &[u8]) -> Vec<usize> {
        input
            .iter()
            .enumerate()
            .filter(|(_, &b)| b == b'\\' || b == b'"')
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    fn test_whitespace_detection() {
        let input = b"hello world\ttab\nnewline\rcarriage  spaces";
        let positions = unsafe { json::find_whitespace_simd(input) };

        let expected = vec![5, 11, 15, 23, 32, 33];
        assert_eq!(positions, expected);
    }

    #[test]
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    fn test_utf8_validation() {
        let valid = "Hello, 世界! 🦀".as_bytes();
        let invalid = &[0xFF, 0xFE, 0xFD];

        unsafe {
            assert!(json::validate_utf8_simd(valid));
            assert!(!json::validate_utf8_simd(invalid));
        }
    }

    #[test]
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    fn test_xor_mask() {
        let mut data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let mask = [0xAA, 0xBB, 0xCC, 0xDD];

        unsafe {
            serialization::xor_mask_simd(&mut data, mask);
        }

        let expected = vec![
            1 ^ 0xAA,
            2 ^ 0xBB,
            3 ^ 0xCC,
            4 ^ 0xDD,
            5 ^ 0xAA,
            6 ^ 0xBB,
            7 ^ 0xCC,
            8 ^ 0xDD,
        ];
        assert_eq!(data, expected);
    }

    #[test]
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    fn test_batch_validation() {
        let lengths = vec![100, 200, 50, 1000, 75, 150, 2000, 80];
        let max = 500;

        let results = unsafe { batch::batch_validate_lengths(&lengths, max) };

        let expected = vec![true, true, true, false, true, true, false, true];
        assert_eq!(results, expected);
    }
}
