// Mog compiler driver.
//
// Orchestrates the full compilation pipeline: tokenise → filter → parse →
// capability loading → semantic analysis → QBE IR generation, and optionally
// continues through QBE → assembler → linker to produce a native binary or
// shared library.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use blake3;

use crate::ast::{Statement, StatementKind};
use crate::capability::{parse_capability_decl, CapabilityDecl};
use crate::token::TokenType;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Code-generation backend.  Only QBE is supported today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Qbe,
}

impl Default for Backend {
    fn default() -> Self {
        Self::Qbe
    }
}

/// Optimisation level passed through to QBE / the system compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    O0,
    O1,
    O2,
}

impl Default for OptLevel {
    fn default() -> Self {
        Self::O0
    }
}

impl OptLevel {
    /// Flag string suitable for passing to `cc` / `clang`.
    fn cc_flag(self) -> &'static str {
        match self {
            OptLevel::O0 => "-O0",
            OptLevel::O1 => "-O1",
            OptLevel::O2 => "-O2",
        }
    }
}

static COMPILER_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn default_qbe_target() -> &'static rqbe::Target {
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "x86_64")]
        {
            &rqbe::amd64::T_AMD64_APPLE
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            &rqbe::arm64::T_ARM64_APPLE
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        &rqbe::amd64::T_AMD64_SYSV
    }
}

/// Options that control a single compilation.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Code-generation backend (currently only QBE).
    pub backend: Backend,
    /// Optimisation level.
    pub optimization: OptLevel,
    /// If set, the final artifact is written here.
    pub output_path: Option<PathBuf>,
    /// When `true`, emit a position-independent shared library instead of an
    /// executable.
    pub plugin_mode: bool,
    /// Plugin name (used for symbol mangling / metadata); required when
    /// `plugin_mode` is `true`.
    pub plugin_name: Option<String>,
    /// Plugin version string.
    pub plugin_version: Option<String>,
    /// Path to the source file being compiled.  Used to resolve capability
    /// declarations relative to the source file's directory.
    pub source_path: Option<PathBuf>,
    /// Whether loop back-edges emit timer-interrupt checks.
    pub loop_interrupt_checks: bool,
    /// If true, emit timer-interrupt checks every _n_ loop iterations based on
    /// estimated per-iteration body cost.
    ///
    /// Disabled by default; enable explicitly via the compiler flag.
    pub adaptive_loop_interrupt_checks: bool,
    /// Target worst-case loop delay when adaptive checking is enabled, in
    /// microseconds.
    pub loop_interrupt_check_target_micros: u64,
    /// Extra object files, C source files, or static libraries to link into
    /// the final binary.
    pub extra_link_objects: Vec<PathBuf>,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            backend: Backend::default(),
            optimization: OptLevel::default(),
            output_path: None,
            plugin_mode: false,
            plugin_name: None,
            plugin_version: None,
            source_path: None,
            loop_interrupt_checks: true,
            adaptive_loop_interrupt_checks: false,
            loop_interrupt_check_target_micros: 1_000,
            extra_link_objects: Vec::new(),
        }
    }
}

/// The result of compiling a single source string (or merged module) down to
/// IR.  This is the *frontend* result — no native code has been emitted yet.
#[derive(Debug, Clone)]
pub struct CompileResult {
    /// QBE intermediate representation (empty string on error).
    pub ir: String,
    /// Fatal errors that prevented code generation.
    pub errors: Vec<String>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
    /// BLAKE3 hash of the compiled plugin binary (hex-encoded).
    /// Only populated by `compile_plugin()`; `None` for regular compilations.
    pub plugin_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Core compilation: source → IR
// ---------------------------------------------------------------------------

/// Compile Mog source code to QBE IR.
///
/// Runs the full frontend pipeline: tokenise, filter whitespace/comments,
/// parse, extract capability declarations, load `.mogdecl` files, run
/// semantic analysis, and (on success) generate QBE IR.
pub fn compile(source: &str, options: &CompileOptions) -> CompileResult {
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // -- 1. Tokenise --------------------------------------------------------
    let tokens = crate::lexer::tokenize(source);

    // -- 2. Filter whitespace & comments ------------------------------------
    let filtered: Vec<_> = tokens
        .into_iter()
        .filter(|t| t.token_type != TokenType::Whitespace && t.token_type != TokenType::Comment)
        .collect();

    if filtered.is_empty() {
        warnings.push("source is empty after filtering whitespace/comments".into());
        return CompileResult {
            ir: String::new(),
            errors,
            warnings,
            plugin_hash: None,
        };
    }

    // -- 3. Parse -----------------------------------------------------------
    let ast: Statement = crate::parser::parse_tokens(&filtered);

    // -- 4. Extract capability declarations from the AST --------------------
    let mut required_caps: Vec<String> = Vec::new();
    let mut optional_caps: Vec<String> = Vec::new();
    extract_capabilities(&ast, &mut required_caps, &mut optional_caps);

    let all_caps: Vec<String> = required_caps
        .iter()
        .chain(optional_caps.iter())
        .cloned()
        .collect();

    // -- 5. Load .mogdecl files for each capability -------------------------
    let capability_decls =
        load_capability_decls(&all_caps, options.source_path.as_ref(), &mut warnings);

    // -- 6. Semantic analysis -----------------------------------------------
    let mut analyzer = crate::analyzer::SemanticAnalyzer::new();
    analyzer.set_capability_decls(capability_decls);

    let semantic_errors = analyzer.analyze(&ast);
    if !semantic_errors.is_empty() {
        errors.extend(semantic_errors.into_iter().map(|e| e.message));
        return CompileResult {
            ir: String::new(),
            errors,
            warnings,
            plugin_hash: None,
        };
    }

    // -- 7. Code generation (QBE IR) ----------------------------------------
    let ir = match options.backend {
        Backend::Qbe => {
            if options.plugin_mode {
                let name = options.plugin_name.as_deref().unwrap_or("plugin");
                let version = options.plugin_version.as_deref().unwrap_or("0.1.0");
                crate::qbe_codegen::generate_plugin_qbe_ir_with_interrupts_and_adaptive(
                    &ast,
                    name,
                    version,
                    options.loop_interrupt_checks,
                    options.adaptive_loop_interrupt_checks,
                    options.loop_interrupt_check_target_micros,
                )
            } else {
                crate::qbe_codegen::generate_qbe_ir_with_caps_and_interrupts_and_adaptive(
                    &ast,
                    analyzer.get_capability_decls(),
                    options.loop_interrupt_checks,
                    options.adaptive_loop_interrupt_checks,
                    options.loop_interrupt_check_target_micros,
                )
            }
        }
    };

    CompileResult {
        ir,
        errors,
        warnings,
        plugin_hash: None,
    }
}

// ---------------------------------------------------------------------------
// Convenience wrapper expected by the FFI layer
// ---------------------------------------------------------------------------

/// Simplified entry-point used by `ffi.rs`.  Compiles with default options and
/// returns either the IR string or a list of error messages.
pub fn compile_simple(source: &str) -> Result<String, Vec<String>> {
    let result = compile(source, &CompileOptions::default());
    if result.errors.is_empty() {
        Ok(result.ir)
    } else {
        Err(result.errors)
    }
}

// ---------------------------------------------------------------------------
// Compile to native binary
// ---------------------------------------------------------------------------

/// Compile Mog source all the way to a native executable.
///
/// 1. Runs the frontend via [`compile`] to obtain QBE IR.
/// 2. Writes the IR to a temporary file.
/// 3. Invokes `qbe` to lower the IR to assembly.
/// 4. Invokes the system assembler (`as`) to produce an object file.
/// 5. Invokes the system C compiler (`cc`) to link with `libmog_runtime.a` and
///    produce a final executable.
///
/// Returns the path to the produced binary on success.
pub fn compile_to_binary(source: &str, options: &CompileOptions) -> Result<PathBuf, Vec<String>> {
    // --- frontend ----------------------------------------------------------
    let result = compile(source, options);
    if !result.errors.is_empty() {
        return Err(result.errors);
    }
    if result.ir.is_empty() {
        return Err(vec!["code generation produced empty IR".into()]);
    }

    // --- temp directory ----------------------------------------------------
    let tmp_index = COMPILER_TMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp = env::temp_dir().join(format!("mog-{}-{}", std::process::id(), tmp_index));
    fs::create_dir_all(&tmp).map_err(|e| vec![format!("failed to create temp dir: {e}")])?;

    let asm_path = tmp.join("out.s");
    let obj_path = tmp.join("out.o");
    let bin_path = options
        .output_path
        .clone()
        .unwrap_or_else(|| tmp.join("a.out"));

    // --- QBE: IR → assembly (in-process via rqbe) ---------------------------
    let asm = rqbe::compile(&result.ir, default_qbe_target())
        .map_err(|e| vec![format!("QBE compilation failed: {e}")])?;
    fs::write(&asm_path, &asm).map_err(|e| vec![format!("failed to write assembly: {e}")])?;

    // --- assembler: assembly → object --------------------------------------
    run_command(
        Command::new("as").arg(&asm_path).arg("-o").arg(&obj_path),
        "as (assembler)",
    )?;

    // --- linker: object → executable ---------------------------------------
    let mut link = cc_command();
    link.arg(&obj_path)
        .arg("-o")
        .arg(&bin_path)
        .arg(options.optimization.cc_flag());

    // Extra objects/libraries from --link flags (compile .c files on the fly)
    let tmp_ref = &tmp;
    for extra in &options.extra_link_objects {
        if extra.extension().is_some_and(|e| e == "c") {
            // Compile .c to .o first
            let c_obj = tmp_ref.join(
                extra
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
                    + ".o",
            );
            run_command(
                Command::new(cc_command().get_program())
                    .arg("-c")
                    .arg(extra)
                    .arg("-o")
                    .arg(&c_obj),
                "cc (compile extra C file)",
            )?;
            link.arg(&c_obj);
        } else if extra.extension().is_some_and(|e| e == "rs") {
            // Compile .rs to static library, then force-load to preserve constructors
            let rs_lib = tmp_ref.join(
                extra
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
                    + ".a",
            );
            run_command(
                Command::new("rustc")
                    .arg("--edition")
                    .arg("2024")
                    .arg("--crate-type")
                    .arg("staticlib")
                    .arg("-o")
                    .arg(&rs_lib)
                    .arg(extra),
                "rustc (compile extra Rust file)",
            )?;
            // Use -force_load to ensure constructors (__mod_init_func) are linked
            link.arg("-Wl,-force_load").arg(&rs_lib);
        } else {
            link.arg(extra);
        }
    }

    // Link against the Mog runtime if we can find it.
    if let Some(rt) = find_runtime_archive() {
        link.arg(&rt);
    }

    // Platform libraries.
    #[cfg(target_os = "macos")]
    {
        link.arg("-lSystem");
    }
    link.arg("-lm");

    run_command(&mut link, "cc (linker)")?;

    // --- clean up temp files (best-effort) ---------------------------------
    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&obj_path);

    Ok(bin_path)
}

// ---------------------------------------------------------------------------
// Plugin compilation (shared library)
// ---------------------------------------------------------------------------

/// Compile Mog source to a shared library suitable for dynamic loading.
///
/// On macOS this produces a `.dylib`; on Linux a `.so`.  The resulting
/// library exposes plugin metadata symbols and uses PIC relocation.
///
/// Returns `(path, blake3_hex)` — the path to the compiled library and its
/// BLAKE3 hash as a lowercase hex string.
pub fn compile_plugin(
    source: &str,
    name: &str,
    version: &str,
) -> Result<(PathBuf, String), Vec<String>> {
    let options = CompileOptions {
        plugin_mode: true,
        plugin_name: Some(name.to_string()),
        plugin_version: Some(version.to_string()),
        ..Default::default()
    };

    // --- frontend ----------------------------------------------------------
    let result = compile(source, &options);
    if !result.errors.is_empty() {
        return Err(result.errors);
    }
    if result.ir.is_empty() {
        return Err(vec!["code generation produced empty IR".into()]);
    }

    // --- temp directory ----------------------------------------------------
    let tmp = env::temp_dir().join(format!("mog-plugin-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| vec![format!("failed to create temp dir: {e}")])?;

    let asm_path = tmp.join(format!("{name}.s"));
    let obj_path = tmp.join(format!("{name}.o"));

    let lib_ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let lib_path = options
        .output_path
        .clone()
        .unwrap_or_else(|| tmp.join(format!("lib{name}.{lib_ext}")));

    // --- QBE: IR → assembly (in-process via rqbe) ---------------------------
    let asm = rqbe::compile(&result.ir, default_qbe_target())
        .map_err(|e| vec![format!("QBE compilation failed: {e}")])?;
    fs::write(&asm_path, &asm).map_err(|e| vec![format!("failed to write assembly: {e}")])?;

    // --- assembler: assembly → PIC object ----------------------------------
    run_command(
        Command::new("as").arg(&asm_path).arg("-o").arg(&obj_path),
        "as (assembler)",
    )?;

    #[cfg(target_os = "macos")]
    let stub_o = {
        // ARM64 Mach-O needs a concrete definition for this TLS-less global at
        // link time because the host resolves it differently than ELF.
        let stub_c = tmp.join("_mog_stub.c");
        let stub_o = tmp.join("_mog_stub.o");
        fs::write(&stub_c, "volatile int mog_interrupt_flag = 0;\n")
            .map_err(|e| vec![format!("failed to write stub: {e}")])?;
        run_command(
            Command::new(cc_command().get_program())
                .arg("-c")
                .arg("-fPIC")
                .arg(&stub_c)
                .arg("-o")
                .arg(&stub_o),
            "cc (compile stub)",
        )?;
        stub_o
    };

    // --- linker: object → shared library -----------------------------------
    let mut link = cc_command();
    link.arg("-shared")
        .arg("-o")
        .arg(&lib_path)
        .arg(&obj_path);

    #[cfg(target_os = "macos")]
    {
        link.arg(&stub_o);
    }

    #[cfg(target_os = "macos")]
    {
        link.arg("-dynamiclib");
    }

    // Extra objects/libraries from --link flags (compile .c/.rs files on the fly)
    let tmp_ref = &tmp;
    for extra in &options.extra_link_objects {
        if extra.extension().is_some_and(|e| e == "c") {
            let c_obj = tmp_ref.join(
                extra
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
                    + ".o",
            );
            run_command(
                Command::new(cc_command().get_program())
                    .arg("-c")
                    .arg(extra)
                    .arg("-o")
                    .arg(&c_obj),
                "cc (compile extra C file)",
            )?;
            link.arg(&c_obj);
        } else if extra.extension().is_some_and(|e| e == "rs") {
            let rs_lib = tmp_ref.join(
                extra
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
                    + ".a",
            );
            run_command(
                Command::new("rustc")
                    .arg("--edition")
                    .arg("2024")
                    .arg("--crate-type")
                    .arg("staticlib")
                    .arg("-o")
                    .arg(&rs_lib)
                    .arg(extra),
                "rustc (compile extra Rust file)",
            )?;
            link.arg(&rs_lib);
        } else {
            link.arg(extra);
        }
    }

    // Do NOT link the Mog runtime into plugins — runtime symbols are resolved
    // dynamically from the host binary at load time.  This ensures the plugin
    // shares the host's global state (VM, event loop, GC heap, etc.).
    #[cfg(target_os = "macos")]
    {
        link.arg("-undefined").arg("dynamic_lookup");
        link.arg("-lSystem");
    }
    link.arg("-lm");

    run_command(&mut link, "cc (shared-library linker)")?;

    // --- clean up temp files (best-effort) ---------------------------------
    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&obj_path);

    // --- compute BLAKE3 hash of the compiled binary -------------------------
    let binary_bytes = fs::read(&lib_path)
        .map_err(|e| vec![format!("failed to read compiled plugin for hashing: {e}")])?;
    let hash_hex = blake3::hash(&binary_bytes).to_hex().to_string();

    Ok((lib_path, hash_hex))
}

// ---------------------------------------------------------------------------
// Multi-file module compilation
// ---------------------------------------------------------------------------

/// Compile an entire Mog module rooted at `root`.
///
/// The directory must contain a `mog.mod` file.  All `.mog` source files are
/// discovered, parsed, and merged before running semantic analysis and code
/// generation as a single unit.
pub fn compile_module(root: &Path, options: &CompileOptions) -> Result<CompileResult, Vec<String>> {
    let mod_file = root.join("mog.mod");
    if !mod_file.exists() {
        return Err(vec![format!("no mog.mod found in {}", root.display())]);
    }

    let mod_contents =
        fs::read_to_string(&mod_file).map_err(|e| vec![format!("failed to read mog.mod: {e}")])?;
    let _module_name = parse_mod_file(&mod_contents);

    // Discover all .mog source files under the module root.
    let sources = discover_mog_files(root)
        .map_err(|e| vec![format!("failed to discover source files: {e}")])?;

    if sources.is_empty() {
        return Err(vec!["no .mog files found in module".into()]);
    }

    // Concatenate all source files (simple merge strategy).  A future
    // implementation should do proper per-package resolution.
    let mut combined = String::new();
    for path in &sources {
        let src = fs::read_to_string(path)
            .map_err(|e| vec![format!("failed to read {}: {e}", path.display())])?;
        combined.push_str(&src);
        combined.push('\n');
    }

    let result = compile(&combined, options);
    if result.errors.is_empty() {
        Ok(result)
    } else {
        Err(result.errors)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recursively extract `RequiresDeclaration` and `OptionalDeclaration`
/// capability names from the AST, descending into blocks.
fn extract_capabilities(stmt: &Statement, required: &mut Vec<String>, optional: &mut Vec<String>) {
    match &stmt.kind {
        StatementKind::RequiresDeclaration { capabilities } => {
            required.extend(capabilities.iter().cloned());
        }
        StatementKind::OptionalDeclaration { capabilities } => {
            optional.extend(capabilities.iter().cloned());
        }
        // Recurse into block-carrying statements.
        StatementKind::Program { statements, .. } | StatementKind::Block { statements, .. } => {
            for s in statements {
                extract_capabilities(s, required, optional);
            }
        }
        StatementKind::FunctionDeclaration { body, .. }
        | StatementKind::AsyncFunctionDeclaration { body, .. } => {
            for s in &body.statements {
                extract_capabilities(s, required, optional);
            }
        }
        StatementKind::Conditional {
            true_branch,
            false_branch,
            ..
        } => {
            for s in &true_branch.statements {
                extract_capabilities(s, required, optional);
            }
            if let Some(fb) = false_branch {
                for s in &fb.statements {
                    extract_capabilities(s, required, optional);
                }
            }
        }
        StatementKind::WhileLoop { body, .. }
        | StatementKind::ForLoop { body, .. }
        | StatementKind::ForEachLoop { body, .. }
        | StatementKind::ForInRange { body, .. } => {
            for s in &body.statements {
                extract_capabilities(s, required, optional);
            }
        }
        StatementKind::TryCatch {
            try_body,
            catch_body,
            ..
        } => {
            for s in &try_body.statements {
                extract_capabilities(s, required, optional);
            }
            for s in &catch_body.statements {
                extract_capabilities(s, required, optional);
            }
        }
        StatementKind::WithBlock { body, .. } => {
            for s in &body.statements {
                extract_capabilities(s, required, optional);
            }
        }
        // Leaf statements — nothing to extract.
        _ => {}
    }
}

/// Search well-known directories for `.mogdecl` files and parse them.
fn load_capability_decls(
    cap_names: &[String],
    source_path: Option<&PathBuf>,
    warnings: &mut Vec<String>,
) -> HashMap<String, CapabilityDecl> {
    let mut decls = HashMap::new();

    // Build a list of directories to search.
    let mut search_paths: Vec<PathBuf> = Vec::new();

    // 1. Relative to the source file being compiled (sibling capabilities/ dir).
    if let Some(src) = source_path {
        // Canonicalize to resolve relative paths like "../showcase.mog".
        let resolved = src.canonicalize().unwrap_or_else(|_| src.clone());
        if let Some(parent) = resolved.parent() {
            search_paths.push(parent.join("capabilities"));
        }
    }

    // 2. Next to the compiler binary (../capabilities).
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            search_paths.push(parent.join("../capabilities"));
        }
    }

    // 3. Relative to cwd.
    if let Ok(cwd) = env::current_dir() {
        search_paths.push(cwd.join("capabilities"));
    }

    for cap_name in cap_names {
        let mut found = false;
        for dir in &search_paths {
            let decl_path = dir.join(format!("{cap_name}.mogdecl"));
            if decl_path.exists() {
                match fs::read_to_string(&decl_path) {
                    Ok(content) => {
                        if let Some(decl) = parse_capability_decl(&content) {
                            if decl.name == *cap_name {
                                decls.insert(cap_name.clone(), decl);
                                found = true;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warnings.push(format!(
                            "failed to read capability declaration {}: {e}",
                            decl_path.display()
                        ));
                    }
                }
            }
        }
        if !found {
            warnings.push(format!("capability declaration not found for '{cap_name}'"));
        }
    }

    decls
}

/// Parse the `mog.mod` file and return the module path string.
fn parse_mod_file(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            return rest.trim().to_string();
        }
    }
    String::new()
}

/// Recursively discover all `.mog` files under `root`.
fn discover_mog_files(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    collect_mog_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_mog_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_mog_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "mog") {
            out.push(path);
        }
    }
    Ok(())
}

/// Run an external command, returning `Ok(())` on success or a descriptive
/// error message on failure.
fn run_command(cmd: &mut Command, label: &str) -> Result<(), Vec<String>> {
    let output = cmd.output().map_err(|e| {
        vec![format!(
            "failed to execute {label}: {e} (is it installed and on PATH?)"
        )]
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut msg = format!("{label} exited with {}", output.status);
        if !stderr.is_empty() {
            msg.push_str(": ");
            msg.push_str(&stderr);
        }
        if !stdout.is_empty() {
            msg.push_str("\n");
            msg.push_str(&stdout);
        }
        return Err(vec![msg]);
    }

    Ok(())
}

/// Return a [`Command`] for the system C compiler.  Respects `$CC`, defaulting
/// to `cc`.
fn cc_command() -> Command {
    let cc = env::var_os("CC").unwrap_or_else(|| "cc".into());
    Command::new(cc)
}

/// Try to locate the Mog runtime archive (`libmog_runtime.a`).
///
/// Falls back through several search strategies.
fn find_runtime_archive() -> Option<PathBuf> {
    // Candidate filenames in priority order.
    let names = ["runtime-rs/target/release/libmog_runtime.a"];

    // 1. Relative to the mog compiler crate's source directory (compile-time).
    {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        for name in &names {
            let p = crate_dir.join("..").join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 2. Relative to the running binary.
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            for depth in ["../../..", "../..", ".."] {
                for name in &names {
                    let p = parent.join(depth).join(name);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }

    // 3. Relative to cwd.
    for prefix in ["", "../", "../../", "../../../"] {
        for name in &names {
            let p = PathBuf::from(format!("{prefix}{name}"));
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 4. MOG_RUNTIME_DIR env var.
    if let Ok(dir) = env::var("MOG_RUNTIME_DIR") {
        for base in ["libmog_runtime.a"] {
            let p = PathBuf::from(&dir).join(base);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options() {
        let opts = CompileOptions::default();
        assert_eq!(opts.backend, Backend::Qbe);
        assert_eq!(opts.optimization, OptLevel::O0);
        assert!(!opts.plugin_mode);
        assert!(opts.output_path.is_none());
    }

    #[test]
    fn compile_empty_source() {
        let result = compile("", &CompileOptions::default());
        // Empty source is not an error — just a warning.
        assert!(result.errors.is_empty());
    }

    #[test]
    fn compile_simple_wrapper() {
        let res = compile_simple("");
        assert!(res.is_ok());
    }

    #[test]
    fn compile_hello_world_ir() {
        let source = r#"fn main() -> int { println("hello world"); return 0; }"#;
        let result = compile(source, &CompileOptions::default());
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.ir.is_empty());
        assert!(
            result.ir.contains("$println_string"),
            "IR should call println_string"
        );
        assert!(
            result.ir.contains("hello world"),
            "IR should contain string constant"
        );
    }

    #[test]
    fn compile_arithmetic_ir() {
        let source = r#"fn main() -> int { x := (2 + 3); println_i64(x); return 0; }"#;
        let result = compile(source, &CompileOptions::default());
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.ir.is_empty());
        assert!(
            result.ir.contains("add"),
            "IR should contain add instruction"
        );
    }

    #[test]
    fn compile_function_call_ir() {
        let source = r#"
            fn square(n: int) -> int { return (n * n); }
            fn main() -> int { result := square(5); println_i64(result); return 0; }
        "#;
        let result = compile(source, &CompileOptions::default());
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.ir.is_empty());
        assert!(
            result.ir.contains("$square"),
            "IR should contain square function"
        );
    }

    #[test]
    fn compile_struct_ir() {
        let source = r#"
            struct Point { x: int; y: int; }
            fn main() -> int {
                p := Point { x: 10, y: 20 };
                println_i64(p.x);
                return 0;
            }
        "#;
        let result = compile(source, &CompileOptions::default());
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.ir.is_empty());
        assert!(
            result.ir.contains("$gc_alloc"),
            "IR should allocate struct via gc_alloc"
        );
    }

    #[test]
    fn compile_control_flow_ir() {
        let source = r#"
            fn main() -> int {
                x := 10;
                if (x > 5) {
                    println("big");
                } else {
                    println("small");
                }
                return 0;
            }
        "#;
        let result = compile(source, &CompileOptions::default());
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.ir.is_empty());
        assert!(
            result.ir.contains("jnz"),
            "IR should contain conditional branch"
        );
    }

    #[test]
    fn compile_while_loop_ir() {
        let source = r#"
            fn main() -> int {
                i := 0;
                while (i < 10) {
                    i = (i + 1);
                }
                println_i64(i);
                return 0;
            }
        "#;
        let result = compile(source, &CompileOptions::default());
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.ir.is_empty());
    }

    #[test]
    fn compile_detects_type_error() {
        let source = r#"fn main() -> int { x := undefined_var; return 0; }"#;
        let result = compile(source, &CompileOptions::default());
        assert!(
            !result.errors.is_empty(),
            "should have errors for undefined variable"
        );
    }

    /// End-to-end: compile to binary and run (requires QBE, as, cc, libmog_runtime.a)
    #[test]
    fn e2e_compile_and_run() {
        let source = r#"fn main() -> int { println("e2e-ok"); return 0; }"#;
        let options = CompileOptions {
            output_path: Some(env::temp_dir().join(format!("mog-e2e-test-{}", std::process::id()))),
            ..Default::default()
        };

        match compile_to_binary(source, &options) {
            Ok(bin_path) => {
                let output = Command::new(&bin_path)
                    .output()
                    .expect("failed to run compiled binary");
                let stdout = String::from_utf8_lossy(&output.stdout);
                assert!(
                    stdout.contains("e2e-ok"),
                    "expected 'e2e-ok' in output, got: {stdout}"
                );
                assert!(
                    output.status.success(),
                    "binary exited with non-zero status"
                );
                let _ = fs::remove_file(&bin_path);
            }
            Err(errors) => {
                // If QBE/runtime aren't available, skip instead of failing
                let msg = errors.join("; ");
                if msg.contains("qbe") || msg.contains("runtime") || msg.contains("not found") {
                    eprintln!("skipping e2e test: {msg}");
                } else {
                    panic!("e2e compilation failed: {msg}");
                }
            }
        }
    }
}
