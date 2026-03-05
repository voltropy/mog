//! Print / IO builtins for the Mog runtime.

use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// GC extern
// ---------------------------------------------------------------------------
unsafe extern "C" {
    fn gc_alloc_kind(size: usize, kind: i32) -> *mut u8;
}

/// ObjectKind::OBJ_STRING = 3
const OBJ_STRING: i32 = 3;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// GC-allocate a string buffer of `len + 1` bytes (for null terminator).
unsafe fn gc_alloc_string(len: usize) -> *mut u8 {
    unsafe { gc_alloc_kind(len + 1, OBJ_STRING) }
}

// ---------------------------------------------------------------------------
// Print functions
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_i64(n: i64) {
    let s = format!("{}", n);
    let _ = io::stdout().write_all(s.as_bytes());
    let _ = io::stdout().flush();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_f64(f: f64) {
    let s = format!("{:.6}", f);
    let _ = io::stdout().write_all(s.as_bytes());
    let _ = io::stdout().flush();
}

// ---------------------------------------------------------------------------
// Aliases for QBE codegen
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_i(n: i64) {
    unsafe { print_i64(n) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_f(f: f64) {
    unsafe { print_f64(f) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_string(s: *const u8) {
    if s.is_null() {
        let _ = io::stdout().write_all(b"(null)");
    } else {
        let cstr = unsafe { std::ffi::CStr::from_ptr(s as *const i8) };
        let _ = io::stdout().write_all(cstr.to_bytes());
    }
    let _ = io::stdout().flush();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn println_i64(n: i64) {
    let s = format!("{}\n", n);
    let _ = io::stdout().write_all(s.as_bytes());
    let _ = io::stdout().flush();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn println_f64(f: f64) {
    let s = format!("{:.6}\n", f);
    let _ = io::stdout().write_all(s.as_bytes());
    let _ = io::stdout().flush();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn println_string(s: *const u8) {
    if s.is_null() {
        let _ = io::stdout().write_all(b"(null)\n");
    } else {
        let cstr = unsafe { std::ffi::CStr::from_ptr(s as *const i8) };
        let _ = io::stdout().write_all(cstr.to_bytes());
    }
    let _ = io::stdout().write_all(b"\n");
    let _ = io::stdout().flush();
}

/// Alias: `println` with a string argument behaves like `println_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn println(s: *const u8) {
    unsafe { println_string(s) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_bool(b: i64) {
    let text = if b != 0 { "true" } else { "false" };
    let _ = io::stdout().write_all(text.as_bytes());
    let _ = io::stdout().flush();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn println_bool(b: i64) {
    let text = if b != 0 { "true\n" } else { "false\n" };
    let _ = io::stdout().write_all(text.as_bytes());
    let _ = io::stdout().flush();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn print_char(c: i64) {
    if let Some(ch) = char::from_u32(c as u32) {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        let _ = io::stdout().write_all(encoded.as_bytes());
        let _ = io::stdout().flush();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn println_char(c: i64) {
    if let Some(ch) = char::from_u32(c as u32) {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf);
        let _ = io::stdout().write_all(encoded.as_bytes());
    }
    let _ = io::stdout().write_all(b"\n");
    let _ = io::stdout().flush();
}

// ---------------------------------------------------------------------------
// Input functions
// ---------------------------------------------------------------------------

/// Read a line from stdin (blocking). Returns a GC-allocated null-terminated
/// string with trailing newline/carriage-return stripped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_line() -> *mut u8 {
    let stdin = io::stdin();
    let mut line = String::new();
    let _ = stdin.lock().read_line(&mut line);
    // Strip trailing newline / carriage return
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    let len = line.len();
    let ptr = unsafe { gc_alloc_string(len) };
    unsafe {
        std::ptr::copy_nonoverlapping(line.as_ptr(), ptr, len);
        *ptr.add(len) = 0; // null terminator
    }
    ptr
}

/// Alias used by the compiler.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stdin_read_line() -> *mut u8 {
    unsafe { read_line() }
}

/// Returns 1 if stdin has data ready, 0 otherwise.
/// Uses select() with a zero timeout for non-blocking poll.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn stdin_poll() -> i64 {
    unsafe {
        let mut fds: libc::fd_set = std::mem::zeroed();
        libc::FD_ZERO(&mut fds);
        libc::FD_SET(libc::STDIN_FILENO, &mut fds);
        let mut tv = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let result = libc::select(
            libc::STDIN_FILENO + 1,
            &mut fds,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut tv,
        );
        if result > 0 {
            1
        } else {
            0
        }
    }
}

/// Returns milliseconds since epoch (wall-clock time).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn time_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
