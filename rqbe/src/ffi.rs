//! C-compatible FFI layer for the rqbe compiler backend.
//!
//! Exposes three entry points matching the old `libmog_backend.dylib` ABI
//! so the Mog TypeScript compiler can call them via Bun FFI (`dlopen`).
//!
//! Safety: `unsafe` is confined to the FFI boundary (pointer-to-slice
//! conversion). All internal compilation remains safe Rust.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;

use crate::arm64::T_ARM64_APPLE;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw `(ptr, len)` pair from the caller into a `&str`.
///
/// # Safety
/// The caller must ensure `input` points to `input_len` valid UTF-8 bytes.
unsafe fn slice_from_raw(input: *const u8, input_len: c_int) -> &'static str {
    let bytes = std::slice::from_raw_parts(input, input_len as usize);
    std::str::from_utf8_unchecked(bytes)
}

/// Convert a null-terminated `*const c_char` to an owned `String`.
///
/// # Safety
/// The pointer must be non-null and point to a valid C string.
unsafe fn string_from_cstr(p: *const c_char) -> String {
    CStr::from_ptr(p).to_string_lossy().into_owned()
}

/// Write assembly text to a temp file and return its path.
fn write_temp_asm(asm: &str) -> std::io::Result<std::path::PathBuf> {
    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("rqbe_{pid}_{ts}.s"));
    std::fs::write(&path, asm)?;
    Ok(path)
}

/// Get the macOS SDK sysroot path via `xcrun --show-sdk-path`.
fn sdk_path() -> Option<String> {
    let out = std::process::Command::new("xcrun")
        .args(["--show-sdk-path"])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Public FFI
// ---------------------------------------------------------------------------

/// Compile QBE IL to ARM64 assembly.
///
/// Returns a heap-allocated C string (caller must free with `mog_qbe_free`),
/// or `NULL` on error.
#[no_mangle]
pub extern "C" fn mog_qbe_compile(input: *const u8, input_len: c_int) -> *mut c_char {
    if input.is_null() || input_len <= 0 {
        return ptr::null_mut();
    }

    let src = unsafe { slice_from_raw(input, input_len) };

    match crate::compile(src, &T_ARM64_APPLE) {
        Ok(asm) => match CString::new(asm) {
            Ok(c) => c.into_raw(),
            Err(_) => ptr::null_mut(), // interior NUL — shouldn't happen
        },
        Err(e) => {
            eprintln!("rqbe error: {e}");
            ptr::null_mut()
        }
    }
}

/// Free a string previously returned by `mog_qbe_compile`.
#[no_mangle]
pub extern "C" fn mog_qbe_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

/// Compile QBE IL to an object file on disk.
///
/// 1. Compile QBE IL → ARM64 assembly via `rqbe::compile`.
/// 2. Write assembly to a temp `.s` file.
/// 3. Invoke `/usr/bin/as` to assemble into `output_obj`.
/// 4. Remove the temp file.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn mog_assemble(
    input: *const u8,
    input_len: c_int,
    output_obj: *const c_char,
) -> c_int {
    if input.is_null() || input_len <= 0 || output_obj.is_null() {
        return -1;
    }

    let src = unsafe { slice_from_raw(input, input_len) };
    let obj_path = unsafe { string_from_cstr(output_obj) };

    // Step 1: compile IL → assembly
    let asm = match crate::compile(src, &T_ARM64_APPLE) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rqbe compile error: {e}");
            return -1;
        }
    };

    // Step 2: write assembly to temp file
    let asm_path = match write_temp_asm(&asm) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rqbe: failed to write temp asm: {e}");
            return -1;
        }
    };

    // Step 3: assemble with /usr/bin/as
    let status = std::process::Command::new("/usr/bin/as")
        .args(["-o", &obj_path, asm_path.to_str().unwrap_or("")])
        .status();

    // Step 4: clean up temp file regardless of result
    let _ = std::fs::remove_file(&asm_path);

    match status {
        Ok(s) if s.success() => 0,
        Ok(s) => {
            eprintln!(
                "rqbe: assembler failed with exit code {}",
                s.code().unwrap_or(-1)
            );
            -1
        }
        Err(e) => {
            eprintln!("rqbe: failed to run assembler: {e}");
            -1
        }
    }
}

/// Compile QBE IL and link into a final binary.
///
/// 1. Assemble QBE IL → temp `.o` via the same pipeline as `mog_assemble`.
/// 2. Link with `cc -o <output_path> <temp.o> [extra_objects...] -lSystem -lm`.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn mog_compile_and_link(
    input: *const u8,
    input_len: c_int,
    output_path: *const c_char,
    extra_objects: *const *const c_char,
    num_extra: c_int,
) -> c_int {
    if input.is_null() || input_len <= 0 || output_path.is_null() {
        return -1;
    }

    let src = unsafe { slice_from_raw(input, input_len) };
    let out_path = unsafe { string_from_cstr(output_path) };

    // Step 1: compile IL → assembly
    let asm = match crate::compile(src, &T_ARM64_APPLE) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rqbe compile error: {e}");
            return -1;
        }
    };

    // Step 2: write assembly to temp file
    let asm_path = match write_temp_asm(&asm) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("rqbe: failed to write temp asm: {e}");
            return -1;
        }
    };

    // Step 3: assemble to temp .o
    let obj_path = asm_path.with_extension("o");
    let as_status = std::process::Command::new("/usr/bin/as")
        .args([
            "-o",
            obj_path.to_str().unwrap_or(""),
            asm_path.to_str().unwrap_or(""),
        ])
        .status();

    let _ = std::fs::remove_file(&asm_path);

    match as_status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!(
                "rqbe: assembler failed with exit code {}",
                s.code().unwrap_or(-1)
            );
            return -1;
        }
        Err(e) => {
            eprintln!("rqbe: failed to run assembler: {e}");
            return -1;
        }
    }

    // Step 4: link with cc
    let mut args: Vec<String> = vec![
        "-o".to_string(),
        out_path.clone(),
        obj_path.to_str().unwrap_or("").to_string(),
    ];

    // Append extra object files
    if !extra_objects.is_null() && num_extra > 0 {
        for i in 0..num_extra as usize {
            let obj_ptr = unsafe { *extra_objects.add(i) };
            if !obj_ptr.is_null() {
                let obj = unsafe { string_from_cstr(obj_ptr) };
                args.push(obj);
            }
        }
    }

    // Add SDK sysroot if available (needed on macOS for -lSystem)
    if let Some(sdk) = sdk_path() {
        args.push("-isysroot".to_string());
        args.push(sdk);
    }

    args.push("-lSystem".to_string());
    args.push("-lm".to_string());

    let link_status = std::process::Command::new("cc").args(&args).status();

    // Clean up temp object file
    let _ = std::fs::remove_file(&obj_path);

    match link_status {
        Ok(s) if s.success() => 0,
        Ok(s) => {
            eprintln!(
                "rqbe: linker failed with exit code {}",
                s.code().unwrap_or(-1)
            );
            -1
        }
        Err(e) => {
            eprintln!("rqbe: failed to run linker: {e}");
            -1
        }
    }
}
