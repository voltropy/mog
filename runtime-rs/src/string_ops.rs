// String operations for the Mog runtime.
//
// Every function is `extern "C"` + `#[unsafe(no_mangle)]` so that QBE-generated code
// can call them directly.  Returned strings are GC-allocated via
// `gc_alloc_kind` with `OBJ_STRING` (kind 5).

use std::ffi::CStr;
use std::ptr;

const OBJ_STRING: i32 = 5;

unsafe extern "C" {
    fn gc_alloc_kind(size: usize, kind: i32) -> *mut u8;
    fn array_alloc(element_size: u64, dimension_count: u64, dimensions: *const u64) -> *mut u8;
    fn array_set(arr: *mut u8, index: i64, value: i64);
}

/// Allocate a GC-managed string of `len` content bytes (+ NUL terminator).
unsafe fn alloc_string(len: usize) -> *mut u8 {
    unsafe { gc_alloc_kind(len + 1, OBJ_STRING) }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw C string pointer to a byte slice (excluding the NUL).
/// Returns an empty slice for null pointers.
unsafe fn cstr_bytes(s: *const u8) -> &'static [u8] {
    if s.is_null() {
        return &[];
    }
    unsafe { CStr::from_ptr(s as *const i8).to_bytes() }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn string_concat(a: *const u8, b: *const u8) -> *mut u8 {
    unsafe {
        let a_bytes = cstr_bytes(a);
        let b_bytes = cstr_bytes(b);
        let total = a_bytes.len() + b_bytes.len();
        let result = alloc_string(total);
        ptr::copy_nonoverlapping(a_bytes.as_ptr(), result, a_bytes.len());
        ptr::copy_nonoverlapping(b_bytes.as_ptr(), result.add(a_bytes.len()), b_bytes.len());
        *result.add(total) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_length(s: *const u8) -> i64 {
    unsafe {
        if s.is_null() {
            return 0;
        }
        CStr::from_ptr(s as *const i8).to_bytes().len() as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_eq(a: *const u8, b: *const u8) -> i64 {
    unsafe {
        if a.is_null() || b.is_null() {
            return if a == b { 1 } else { 0 };
        }
        let a_bytes = cstr_bytes(a);
        let b_bytes = cstr_bytes(b);
        if a_bytes == b_bytes {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_upper(s: *const u8) -> *mut u8 {
    unsafe {
        let bytes = cstr_bytes(s);
        let len = bytes.len();
        let result = alloc_string(len);
        for i in 0..len {
            let c = bytes[i];
            *result.add(i) = if c >= b'a' && c <= b'z' { c - 32 } else { c };
        }
        *result.add(len) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_lower(s: *const u8) -> *mut u8 {
    unsafe {
        let bytes = cstr_bytes(s);
        let len = bytes.len();
        let result = alloc_string(len);
        for i in 0..len {
            let c = bytes[i];
            *result.add(i) = if c >= b'A' && c <= b'Z' { c + 32 } else { c };
        }
        *result.add(len) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_trim(s: *const u8) -> *mut u8 {
    unsafe {
        let bytes = cstr_bytes(s);
        let len = bytes.len();

        fn is_ws(c: u8) -> bool {
            c == b' ' || c == b'\t' || c == b'\n' || c == b'\r'
        }

        let mut start = 0usize;
        while start < len && is_ws(bytes[start]) {
            start += 1;
        }
        let mut end = len;
        while end > start && is_ws(bytes[end - 1]) {
            end -= 1;
        }

        let result_len = end - start;
        let result = alloc_string(result_len);
        ptr::copy_nonoverlapping(bytes.as_ptr().add(start), result, result_len);
        *result.add(result_len) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_contains(s: *const u8, needle: *const u8) -> i64 {
    unsafe {
        if s.is_null() || needle.is_null() {
            return 0;
        }
        let haystack = cstr_bytes(s);
        let needle = cstr_bytes(needle);
        if needle.is_empty() {
            return 1;
        }
        if needle.len() > haystack.len() {
            return 0;
        }
        for i in 0..=(haystack.len() - needle.len()) {
            if &haystack[i..i + needle.len()] == needle {
                return 1;
            }
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_starts_with(s: *const u8, prefix: *const u8) -> i64 {
    unsafe {
        if s.is_null() || prefix.is_null() {
            return 0;
        }
        let s_bytes = cstr_bytes(s);
        let p_bytes = cstr_bytes(prefix);
        if p_bytes.len() > s_bytes.len() {
            return 0;
        }
        if &s_bytes[..p_bytes.len()] == p_bytes {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_ends_with(s: *const u8, suffix: *const u8) -> i64 {
    unsafe {
        if s.is_null() || suffix.is_null() {
            return 0;
        }
        let s_bytes = cstr_bytes(s);
        let suf_bytes = cstr_bytes(suffix);
        if suf_bytes.len() > s_bytes.len() {
            return 0;
        }
        if &s_bytes[s_bytes.len() - suf_bytes.len()..] == suf_bytes {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_replace(s: *const u8, from: *const u8, to: *const u8) -> *mut u8 {
    unsafe {
        let s_bytes = cstr_bytes(s);
        let from_bytes = cstr_bytes(from);
        let to_bytes = cstr_bytes(to);

        if from_bytes.is_empty() {
            // No replacement possible — return a copy.
            let result = alloc_string(s_bytes.len());
            ptr::copy_nonoverlapping(s_bytes.as_ptr(), result, s_bytes.len());
            *result.add(s_bytes.len()) = 0;
            return result;
        }

        // Count occurrences.
        let mut count = 0usize;
        let mut pos = 0usize;
        while pos + from_bytes.len() <= s_bytes.len() {
            if &s_bytes[pos..pos + from_bytes.len()] == from_bytes {
                count += 1;
                pos += from_bytes.len();
            } else {
                pos += 1;
            }
        }

        if count == 0 {
            let result = alloc_string(s_bytes.len());
            ptr::copy_nonoverlapping(s_bytes.as_ptr(), result, s_bytes.len());
            *result.add(s_bytes.len()) = 0;
            return result;
        }

        let result_len = s_bytes.len() + count * to_bytes.len() - count * from_bytes.len();
        let result = alloc_string(result_len);

        let mut dest = 0usize;
        let mut src = 0usize;
        while src + from_bytes.len() <= s_bytes.len() {
            if &s_bytes[src..src + from_bytes.len()] == from_bytes {
                ptr::copy_nonoverlapping(to_bytes.as_ptr(), result.add(dest), to_bytes.len());
                dest += to_bytes.len();
                src += from_bytes.len();
            } else {
                *result.add(dest) = s_bytes[src];
                dest += 1;
                src += 1;
            }
        }
        // Copy remaining bytes after the last possible match position.
        while src < s_bytes.len() {
            *result.add(dest) = s_bytes[src];
            dest += 1;
            src += 1;
        }
        *result.add(result_len) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_split(s: *const u8, delim: *const u8) -> *mut u8 {
    unsafe {
        if s.is_null() || delim.is_null() {
            let dims: [u64; 1] = [0];
            return array_alloc(8, 1, dims.as_ptr());
        }

        let s_bytes = cstr_bytes(s);
        let d_bytes = cstr_bytes(delim);

        if d_bytes.is_empty() {
            // Split into individual characters.
            let len = s_bytes.len() as u64;
            let dims: [u64; 1] = [len];
            let arr = array_alloc(8, 1, dims.as_ptr());
            for i in 0..s_bytes.len() {
                let ch = alloc_string(1);
                *ch = s_bytes[i];
                *ch.add(1) = 0;
                array_set(arr, i as i64, ch as i64);
            }
            return arr;
        }

        // First pass: count splits.
        let mut count: u64 = 1;
        let mut pos = 0usize;
        while pos + d_bytes.len() <= s_bytes.len() {
            if &s_bytes[pos..pos + d_bytes.len()] == d_bytes {
                count += 1;
                pos += d_bytes.len();
            } else {
                pos += 1;
            }
        }

        let dims: [u64; 1] = [count];
        let arr = array_alloc(8, 1, dims.as_ptr());

        // Second pass: extract substrings.
        let mut idx: i64 = 0;
        let mut start = 0usize;
        pos = 0;
        while pos + d_bytes.len() <= s_bytes.len() {
            if &s_bytes[pos..pos + d_bytes.len()] == d_bytes {
                let part_len = pos - start;
                let part = alloc_string(part_len);
                ptr::copy_nonoverlapping(s_bytes.as_ptr().add(start), part, part_len);
                *part.add(part_len) = 0;
                array_set(arr, idx, part as i64);
                idx += 1;
                start = pos + d_bytes.len();
                pos = start;
            } else {
                pos += 1;
            }
        }

        // Last part.
        let last_len = s_bytes.len() - start;
        let last = alloc_string(last_len);
        ptr::copy_nonoverlapping(s_bytes.as_ptr().add(start), last, last_len);
        *last.add(last_len) = 0;
        array_set(arr, idx, last as i64);

        arr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn string_slice(s: *const u8, start: i64, end: i64) -> *mut u8 {
    unsafe {
        let bytes = cstr_bytes(s);
        let len = bytes.len() as u64;
        let start = start as u64;
        let end = end as u64;

        if start >= len {
            let result = alloc_string(0);
            *result = 0;
            return result;
        }
        let end = if end > len { len } else { end };
        if end <= start {
            let result = alloc_string(0);
            *result = 0;
            return result;
        }

        let slice_len = (end - start) as usize;
        let result = alloc_string(slice_len);
        ptr::copy_nonoverlapping(bytes.as_ptr().add(start as usize), result, slice_len);
        *result.add(slice_len) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn parse_int(s: *const u8) -> i64 {
    unsafe {
        if s.is_null() {
            return 0;
        }
        let bytes = cstr_bytes(s);
        if bytes.is_empty() {
            return 0;
        }
        // Use Rust's str parsing which handles the same cases as strtoll base-10.
        let Ok(st) = std::str::from_utf8(bytes) else {
            return 0;
        };
        let trimmed = st.trim();
        // Parse leading integer portion (strtoll stops at first non-digit).
        // We need to replicate strtoll behaviour: optional leading whitespace,
        // optional sign, then digits.
        let mut chars = trimmed.chars().peekable();
        let mut negative = false;
        if chars.peek() == Some(&'-') {
            negative = true;
            chars.next();
        } else if chars.peek() == Some(&'+') {
            chars.next();
        }
        let mut value: i64 = 0;
        let mut any_digit = false;
        for c in chars {
            if c.is_ascii_digit() {
                any_digit = true;
                value = value
                    .wrapping_mul(10)
                    .wrapping_add((c as i64) - ('0' as i64));
            } else {
                break;
            }
        }
        if !any_digit {
            return 0;
        }
        if negative {
            -value
        } else {
            value
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn parse_float(s: *const u8) -> f64 {
    unsafe {
        if s.is_null() {
            return 0.0;
        }
        let bytes = cstr_bytes(s);
        if bytes.is_empty() {
            return 0.0;
        }
        let Ok(st) = std::str::from_utf8(bytes) else {
            return 0.0;
        };
        st.trim().parse::<f64>().unwrap_or(0.0)
    }
}
