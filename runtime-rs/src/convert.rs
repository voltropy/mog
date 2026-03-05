// Mog runtime — type conversion helpers.
//
// Every function returns a GC-allocated, null-terminated C string.

use core::ptr;

unsafe extern "C" {
    fn gc_alloc(size: usize) -> *mut u8;
}

/// Helper: write `s` into a fresh GC-allocated buffer with a trailing NUL.
fn alloc_cstr(s: &str) -> *mut u8 {
    let bytes = s.as_bytes();
    let len = bytes.len();
    unsafe {
        let buf = gc_alloc(len + 1);
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
        *buf.add(len) = 0;
        buf
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn int_to_string(n: i64) -> *mut u8 {
    // itoa-style: format on the stack, then copy into GC memory.
    let mut buf = [0u8; 21]; // enough for i64 + sign + NUL
    let s = format_i64(n, &mut buf);
    alloc_cstr(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn float_to_string(f: f64) -> *mut u8 {
    // Match the C runtime's `snprintf(buf, 64, "%f", value)` behaviour:
    // 6 decimal places, no scientific notation.
    let mut buf = [0u8; 64];
    let len = fmt_f64(f, &mut buf);
    let s = unsafe { core::str::from_utf8_unchecked(&buf[..len]) };
    alloc_cstr(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn str(value: i64) -> *mut u8 {
    int_to_string(value)
}

#[unsafe(no_mangle)]
pub extern "C" fn str_f64(value: f64) -> *mut u8 {
    float_to_string(value)
}

// ---------------------------------------------------------------------------
// Aliases for QBE codegen
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn i64_to_string(n: i64) -> *mut u8 {
    int_to_string(n)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn f64_to_string(f: f64) -> *mut u8 {
    float_to_string(f)
}

#[unsafe(no_mangle)]
pub extern "C" fn bool_to_string(b: i64) -> *mut u8 {
    if b != 0 {
        alloc_cstr("true")
    } else {
        alloc_cstr("false")
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers (no allocator needed, stack-only)
// ---------------------------------------------------------------------------

/// Format an i64 into `buf` and return the written portion as a `&str`.
fn format_i64(n: i64, buf: &mut [u8; 21]) -> &str {
    if n == 0 {
        return "0";
    }

    let negative = n < 0;
    // Work with the absolute value as u64 to avoid overflow on i64::MIN.
    let mut v: u64 = if negative {
        (n as u128).wrapping_neg() as u64
    } else {
        n as u64
    };

    let mut pos = buf.len();
    while v > 0 {
        pos -= 1;
        buf[pos] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    if negative {
        pos -= 1;
        buf[pos] = b'-';
    }

    unsafe { core::str::from_utf8_unchecked(&buf[pos..]) }
}

/// Format an f64 in `%f` style (6 decimal places) into `buf`. Returns the
/// number of bytes written.
fn fmt_f64(value: f64, buf: &mut [u8; 64]) -> usize {
    // Handle special cases.
    if value != value {
        // NaN
        let s = b"nan";
        buf[..s.len()].copy_from_slice(s);
        return s.len();
    }
    if value == f64::INFINITY {
        let s = b"inf";
        buf[..s.len()].copy_from_slice(s);
        return s.len();
    }
    if value == f64::NEG_INFINITY {
        let s = b"-inf";
        buf[..s.len()].copy_from_slice(s);
        return s.len();
    }

    let mut pos: usize = 0;
    let mut v = value;

    if v < 0.0 {
        buf[pos] = b'-';
        pos += 1;
        v = -v;
    }

    // Split into integer and fractional parts.
    let int_part = v as u64;
    // Round the fractional part to 6 decimal places.
    let frac_raw = ((v - int_part as f64) * 1_000_000.0 + 0.5) as u64;

    // Write integer part.
    pos += write_u64(int_part, &mut buf[pos..]);

    // Decimal point.
    buf[pos] = b'.';
    pos += 1;

    // Write fractional part, zero-padded to 6 digits.
    let frac = if frac_raw >= 1_000_000 {
        // Rounding overflow (e.g. 0.9999999…) — already handled by int_part
        // being incremented in the cast above, but guard the digit count.
        0u64
    } else {
        frac_raw
    };
    let frac_start = pos;
    pos += write_u64(frac, &mut buf[pos..]);
    // Left-pad with zeros if fewer than 6 digits were written.
    let frac_digits = pos - frac_start;
    if frac_digits < 6 {
        let pad = 6 - frac_digits;
        // Shift digits right.
        buf.copy_within(frac_start..pos, frac_start + pad);
        for i in 0..pad {
            buf[frac_start + i] = b'0';
        }
        pos += pad;
    }

    pos
}

/// Write a u64 in decimal to `buf`, returning the number of bytes written.
/// Writes at least one digit (handles zero).
fn write_u64(n: u64, buf: &mut [u8]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }

    // Write digits in reverse, then reverse in place.
    let mut v = n;
    let mut len = 0usize;
    while v > 0 {
        buf[len] = b'0' + (v % 10) as u8;
        v /= 10;
        len += 1;
    }
    buf[..len].reverse();
    len
}
