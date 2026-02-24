//! Mog Plugin Host -- Rust Edition
//!
//! Demonstrates compiling Mog source to a plugin (.dylib), loading it at
//! runtime with `libloading`, and calling exported functions through the
//! Mog plugin protocol.  All dynamic loading is done through safe Rust
//! wrappers -- no raw dlopen/dlsym calls.

use std::env;
use std::ffi::{c_char, c_void, CStr};
use std::path::PathBuf;
use std::process;

use libloading::Library;

// ---------------------------------------------------------------------------
// C-compatible structs matching the Mog plugin ABI
// ---------------------------------------------------------------------------

/// Plugin metadata returned by `mog_plugin_info()`.
///
/// Note: the QBE codegen emits a simplified 3-field struct (24 bytes):
///   [name: *const c_char, version: *const c_char, num_exports: i64]
#[repr(C)]
struct MogPluginInfo {
    name: *const c_char,
    version: *const c_char,
    num_exports: i64,
}

/// A single exported function entry returned by `mog_plugin_exports()`.
#[repr(C)]
struct MogPluginExport {
    name: *const c_char,
    func_ptr: *mut c_void,
}

// ---------------------------------------------------------------------------
// Safe helpers for reading C strings from plugin data
// ---------------------------------------------------------------------------

/// Convert a `*const c_char` to a Rust `&str`, returning `"<null>"` if the
/// pointer is null or the data is not valid UTF-8.
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        return "<null>";
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .unwrap_or("<invalid utf-8>")
}

// ---------------------------------------------------------------------------
// Plugin wrapper
// ---------------------------------------------------------------------------

/// A loaded Mog plugin.  Owns the `Library` handle and caches the export
/// table for fast function lookups.
struct MogPlugin {
    _lib: Library,
    exports: Vec<(String, *mut c_void)>,
    info_name: String,
    info_version: String,
    num_exports: i64,
}

impl MogPlugin {
    /// Load a compiled Mog plugin from the given path.
    ///
    /// This calls the three protocol functions:
    ///   1. `mog_plugin_info()`    -- metadata
    ///   2. `mog_plugin_init(vm)`  -- initialise the plugin (GC, globals)
    ///   3. `mog_plugin_exports()` -- retrieve the function table
    fn load(path: &std::path::Path) -> Result<Self, String> {
        // SAFETY: We are loading a shared library that follows the Mog plugin
        // protocol.  libloading handles dlopen/dlclose lifetime.
        let lib =
            unsafe { Library::new(path) }.map_err(|e| format!("failed to load plugin: {e}"))?;

        // --- mog_plugin_info ---
        let info_fn: libloading::Symbol<unsafe extern "C" fn() -> *const MogPluginInfo> =
            unsafe { lib.get(b"mog_plugin_info\0") }
                .map_err(|e| format!("missing mog_plugin_info: {e}"))?;

        let info_ptr = unsafe { info_fn() };
        if info_ptr.is_null() {
            return Err("mog_plugin_info() returned null".into());
        }
        let info = unsafe { &*info_ptr };

        let info_name = unsafe { cstr_to_str(info.name) }.to_string();
        let info_version = unsafe { cstr_to_str(info.version) }.to_string();
        let num_exports = info.num_exports;

        // --- mog_plugin_init ---
        let init_fn: libloading::Symbol<unsafe extern "C" fn(*mut c_void) -> i32> =
            unsafe { lib.get(b"mog_plugin_init\0") }
                .map_err(|e| format!("missing mog_plugin_init: {e}"))?;

        // The math plugin needs no capabilities, so we can pass null for the
        // VM pointer.  Plugins that call `mog_vm_set_global(NULL)` inside
        // init will simply set the global VM to NULL, which is fine when
        // no capabilities are used.
        let init_result = unsafe { init_fn(std::ptr::null_mut()) };
        if init_result != 0 {
            return Err(format!("mog_plugin_init failed with code {init_result}"));
        }

        // --- mog_plugin_exports ---
        let exports_fn: libloading::Symbol<
            unsafe extern "C" fn(*mut i32) -> *const MogPluginExport,
        > = unsafe { lib.get(b"mog_plugin_exports\0") }
            .map_err(|e| format!("missing mog_plugin_exports: {e}"))?;

        let mut count: i32 = 0;
        let export_ptr = unsafe { exports_fn(&mut count) };

        let mut exports = Vec::new();
        if !export_ptr.is_null() && count > 0 {
            for i in 0..count as usize {
                let entry = unsafe { &*export_ptr.add(i) };
                let name = unsafe { cstr_to_str(entry.name) }.to_string();
                exports.push((name, entry.func_ptr));
            }
        }

        Ok(MogPlugin {
            _lib: lib,
            exports,
            info_name,
            info_version,
            num_exports,
        })
    }

    /// Look up an exported function by name and call it with the given i64
    /// arguments.  Returns `None` if the function is not found.
    fn call(&self, name: &str, args: &[i64]) -> Option<i64> {
        let (_, fptr) = self.exports.iter().find(|(n, _)| n == name)?;
        let fptr = *fptr;
        if fptr.is_null() {
            return None;
        }
        // SAFETY: Mog exported functions use the C calling convention with
        // i64 arguments and an i64 return value.  We dispatch based on arity.
        let result = unsafe {
            match args.len() {
                0 => {
                    let f: extern "C" fn() -> i64 = std::mem::transmute(fptr);
                    f()
                }
                1 => {
                    let f: extern "C" fn(i64) -> i64 = std::mem::transmute(fptr);
                    f(args[0])
                }
                2 => {
                    let f: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(fptr);
                    f(args[0], args[1])
                }
                3 => {
                    let f: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(fptr);
                    f(args[0], args[1], args[2])
                }
                4 => {
                    let f: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(fptr);
                    f(args[0], args[1], args[2], args[3])
                }
                _ => {
                    eprintln!("too many arguments (max 4)");
                    return None;
                }
            }
        };
        Some(result)
    }
}

// ---------------------------------------------------------------------------
// Capability sandbox demonstration
// ---------------------------------------------------------------------------

/// Demonstrates capability sandboxing at two levels:
///
/// 1. **Compiler level**: The Mog compiler enforces that any `requires`
///    declaration has a matching `.mogdecl` file.  If a plugin declares
///    `requires env` but no `env.mogdecl` exists, compilation fails.
///
/// 2. **Host level**: Even if a plugin compiles, the host can inspect its
///    metadata and refuse to initialise it if the required capabilities
///    are not registered in the VM.
fn demo_capability_sandbox() {
    println!("--- Capability Sandbox Demo ---");
    println!();

    let source = "requires env\n\npub fn get_home() -> int {\n    return 42;\n}\n";

    println!("Compiling a plugin that requires the env capability...");

    match mog::compiler::compile_plugin(source, "cap_test_plugin", "0.1.0") {
        Ok(lib_path) => {
            println!("  Compiled to: {}", lib_path.display());
            println!("  In a real host, we would check the plugin required");
            println!("  capabilities against the VM registered caps before");
            println!("  calling mog_plugin_init().");
            let _ = std::fs::remove_file(&lib_path);
        }
        Err(errs) => {
            // Expected: the compiler rejects `requires env` when there is no
            // env.mogdecl file.  This IS the sandbox in action.
            println!("  Compilation rejected (no capability declaration found):");
            for e in &errs {
                println!("    {e}");
            }
            println!("  -> This demonstrates compiler-level sandboxing:");
            println!("     plugins cannot use undeclared capabilities.");
        }
    }
    println!();
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    println!("=== Mog Plugin Host (Rust) ===");
    println!();

    // Determine the path to math_plugin.mog
    let source_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        // Default: sibling of this example directory
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("..").join("math_plugin.mog")
    });

    println!("Source: {}", source_path.display());
    println!();

    // Read the Mog source
    let source = match std::fs::read_to_string(&source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", source_path.display());
            process::exit(1);
        }
    };

    // --- Step 1: Compile to a plugin shared library ---
    println!("[1/3] Compiling math_plugin.mog to a shared library...");

    let lib_path = match mog::compiler::compile_plugin(&source, "math_plugin", "1.0.0") {
        Ok(path) => {
            println!("      -> {}", path.display());
            println!();
            path
        }
        Err(errors) => {
            eprintln!("Compilation failed:");
            for e in &errors {
                eprintln!("  {e}");
            }
            process::exit(1);
        }
    };

    // --- Step 2: Load the plugin ---
    println!("[2/3] Loading plugin via libloading...");

    let plugin = match MogPlugin::load(&lib_path) {
        Ok(p) => {
            println!(
                "      Loaded: {} v{} ({} exports)",
                p.info_name, p.info_version, p.num_exports
            );
            println!();
            p
        }
        Err(e) => {
            eprintln!("Failed to load plugin: {e}");
            process::exit(1);
        }
    };

    // Print the export list
    println!("Exported functions:");
    for (name, _) in &plugin.exports {
        println!("  - {name}");
    }
    println!();

    // --- Step 3: Call exported functions ---
    println!("[3/3] Calling exported functions:");
    println!();

    // fibonacci(10) = 55
    match plugin.call("fibonacci", &[10]) {
        Some(r) => println!("  fibonacci(10) = {r}"),
        None => eprintln!("  fibonacci: not found"),
    }

    // factorial(7) = 5040
    match plugin.call("factorial", &[7]) {
        Some(r) => println!("  factorial(7)  = {r}"),
        None => eprintln!("  factorial: not found"),
    }

    // gcd(48, 18) = 6
    match plugin.call("gcd", &[48, 18]) {
        Some(r) => println!("  gcd(48, 18)   = {r}"),
        None => eprintln!("  gcd: not found"),
    }

    // sum_of_squares(3, 4) = 25
    match plugin.call("sum_of_squares", &[3, 4]) {
        Some(r) => println!("  sum_of_squares(3, 4) = {r}"),
        None => eprintln!("  sum_of_squares: not found"),
    }

    // Calling a non-existent function
    match plugin.call("nonexistent", &[]) {
        Some(r) => println!("  nonexistent() = {r}  (unexpected)"),
        None => println!("  nonexistent() -> not found (expected)"),
    }

    println!();

    // --- Capability sandbox demo ---
    demo_capability_sandbox();

    // Clean up
    let _ = std::fs::remove_file(&lib_path);

    println!("=== Done ===");
}
