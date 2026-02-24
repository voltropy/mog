// C FFI layer for the Mog compiler.
//
// This module makes the Rust compiler embeddable from C (or any language
// with a C FFI). Every public symbol uses `extern "C"` linkage and
// `#[unsafe(no_mangle)]` so it is visible from the shared/static library
// produced by the crate.
//
// Ownership conventions:
//   - `mog_compiler_new`   -> caller owns the returned pointer; free with `mog_compiler_free`.
//   - `mog_compile`        -> caller owns the returned `MogCompileResult`; free with `mog_result_free`.
//   - `mog_compile_to_ir`  -> caller owns the returned string; free with `mog_free_string`.
//   - Borrowed pointers (`*const`) must remain valid for the duration of the call.

use std::ffi::{c_char, c_int, CStr, CString};
use std::path::PathBuf;
use std::ptr;

// ---------------------------------------------------------------------------
// Opaque compiler handle
// ---------------------------------------------------------------------------

/// Opaque handle exposed to C.  Wraps any state the compiler needs to carry
/// between calls (currently a unit struct — room to grow).
pub struct MogCompiler {
    // Reserved for future state (optimization level, target triple, etc.)
    _private: (),
}

// ---------------------------------------------------------------------------
// C-compatible option / result structs
// ---------------------------------------------------------------------------

/// Compilation options passed from C.
#[repr(C)]
pub struct MogCompileOptions {
    /// Optimisation level: 0 = none, 1 = basic, 2 = full.
    pub opt_level: c_int,
    /// If non-zero, emit debug information.
    pub debug_info: c_int,
    /// Optional target triple (null = host default).
    pub target: *const c_char,
}

/// Result of a compilation.  Owns all heap-allocated strings inside it.
pub struct MogCompileResult {
    /// The compiled IR (if compilation succeeded).
    ir: Option<CString>,
    /// Collected error messages (if any).
    errors: Vec<CString>,
}

// ---------------------------------------------------------------------------
// Compiler lifecycle
// ---------------------------------------------------------------------------

/// Create a new compiler instance.
/// Returns `NULL` on allocation failure (practically impossible).
#[unsafe(no_mangle)]
pub extern "C" fn mog_compiler_new() -> *mut MogCompiler {
    let compiler = Box::new(MogCompiler { _private: () });
    Box::into_raw(compiler)
}

/// Destroy a compiler instance previously created with `mog_compiler_new`.
/// Passing `NULL` is a safe no-op.
#[unsafe(no_mangle)]
pub extern "C" fn mog_compiler_free(compiler: *mut MogCompiler) {
    if !compiler.is_null() {
        unsafe {
            drop(Box::from_raw(compiler));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers (internal)
// ---------------------------------------------------------------------------

/// Convert a `*const c_char` to `&str`, returning `None` on null or invalid
/// UTF-8.
unsafe fn cstr_to_str<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

/// Convert a Rust `String` into an owned `*mut c_char`.
/// Returns `null` if the string contains interior NUL bytes.
fn string_to_c(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Compilation
// ---------------------------------------------------------------------------

/// Compile `source` with the given options, returning a result handle.
///
/// - `compiler` may be `NULL` (a temporary compiler is used).
/// - `source` must be a valid, NUL-terminated UTF-8 string.
/// - `options` may be `NULL` (defaults are used).
///
/// The caller must free the returned pointer with `mog_result_free`.
/// Returns `NULL` only if `source` is `NULL` or not valid UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn mog_compile(
    _compiler: *mut MogCompiler,
    source: *const c_char,
    _options: *const MogCompileOptions,
) -> *mut MogCompileResult {
    let input = match unsafe { cstr_to_str(source) } {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let result = match crate::compiler::compile_simple(input) {
        Ok(ir) => MogCompileResult {
            ir: CString::new(ir).ok(),
            errors: Vec::new(),
        },
        Err(errs) => MogCompileResult {
            ir: None,
            errors: errs
                .into_iter()
                .filter_map(|e| CString::new(e).ok())
                .collect(),
        },
    };

    Box::into_raw(Box::new(result))
}

/// Compile `source` and return the intermediate representation as a C string.
///
/// Returns `NULL` on error or if `source` is `NULL`.
/// The caller must free the returned pointer with `mog_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn mog_compile_to_ir(
    _compiler: *mut MogCompiler,
    source: *const c_char,
) -> *mut c_char {
    let input = match unsafe { cstr_to_str(source) } {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    match crate::compiler::compile_simple(input) {
        Ok(ir) => string_to_c(ir),
        Err(_) => ptr::null_mut(),
    }
}

/// Compile `source` and write the resulting binary to `output_path`.
///
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn mog_compile_to_binary(
    _compiler: *mut MogCompiler,
    source: *const c_char,
    output_path: *const c_char,
) -> c_int {
    let input = match unsafe { cstr_to_str(source) } {
        Some(s) => s,
        None => return -1,
    };
    let _path = match unsafe { cstr_to_str(output_path) } {
        Some(s) => s,
        None => return -1,
    };

    let options = crate::compiler::CompileOptions {
        output_path: Some(PathBuf::from(_path)),
        ..Default::default()
    };

    match crate::compiler::compile_to_binary(input, &options) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Compile `source` as a Mog plugin, embedding `name` and `version` metadata,
/// and write the shared library to `output_path`.
///
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn mog_compile_plugin(
    _compiler: *mut MogCompiler,
    source: *const c_char,
    name: *const c_char,
    version: *const c_char,
    output_path: *const c_char,
) -> c_int {
    let input = match unsafe { cstr_to_str(source) } {
        Some(s) => s,
        None => return -1,
    };
    let _name = match unsafe { cstr_to_str(name) } {
        Some(s) => s,
        None => return -1,
    };
    let _version = match unsafe { cstr_to_str(version) } {
        Some(s) => s,
        None => return -1,
    };
    let _path = match unsafe { cstr_to_str(output_path) } {
        Some(s) => s,
        None => return -1,
    };

    match crate::compiler::compile_plugin(input, _name, _version) {
        Ok(lib_path) => {
            // If the caller specified a different output path, copy the file there.
            let target = PathBuf::from(_path);
            if lib_path != target {
                if let Err(_) = std::fs::copy(&lib_path, &target) {
                    return -1;
                }
            }
            0
        }
        Err(_) => -1,
    }
}

// ---------------------------------------------------------------------------
// Result access
// ---------------------------------------------------------------------------

/// Get the compiled IR from a result.  Returns `NULL` if compilation failed
/// or if `result` is `NULL`.
///
/// The returned pointer is **borrowed** — valid until `mog_result_free` is
/// called on the owning result.
#[unsafe(no_mangle)]
pub extern "C" fn mog_result_get_ir(result: *const MogCompileResult) -> *const c_char {
    if result.is_null() {
        return ptr::null();
    }
    let r = unsafe { &*result };
    match &r.ir {
        Some(cstr) => cstr.as_ptr(),
        None => ptr::null(),
    }
}

/// Return the number of errors in the compilation result.
/// Returns 0 if `result` is `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn mog_result_get_error_count(result: *const MogCompileResult) -> c_int {
    if result.is_null() {
        return 0;
    }
    let r = unsafe { &*result };
    r.errors.len() as c_int
}

/// Get the `index`-th error message.  Returns `NULL` if out of bounds or if
/// `result` is `NULL`.
///
/// The returned pointer is **borrowed** — valid until `mog_result_free`.
#[unsafe(no_mangle)]
pub extern "C" fn mog_result_get_error(
    result: *const MogCompileResult,
    index: c_int,
) -> *const c_char {
    if result.is_null() || index < 0 {
        return ptr::null();
    }
    let r = unsafe { &*result };
    match r.errors.get(index as usize) {
        Some(cstr) => cstr.as_ptr(),
        None => ptr::null(),
    }
}

/// Free a compilation result previously returned by `mog_compile`.
/// Passing `NULL` is a safe no-op.
#[unsafe(no_mangle)]
pub extern "C" fn mog_result_free(result: *mut MogCompileResult) {
    if !result.is_null() {
        unsafe {
            drop(Box::from_raw(result));
        }
    }
}

// ---------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------

/// Free a C string returned by `mog_compile_to_ir` (or any other function
/// that hands out an owned `*mut c_char`).
/// Passing `NULL` is a safe no-op.
#[unsafe(no_mangle)]
pub extern "C" fn mog_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}
