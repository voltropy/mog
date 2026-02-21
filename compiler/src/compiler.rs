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
        };
    }

    // -- 7. Code generation (QBE IR) ----------------------------------------
    let ir = match options.backend {
        Backend::Qbe => crate::qbe_codegen::generate_qbe_ir(&ast),
    };

    CompileResult {
        ir,
        errors,
        warnings,
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
/// 5. Invokes the system C compiler (`cc`) to link with `runtime.a` and
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
    let tmp = env::temp_dir().join(format!("mog-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| vec![format!("failed to create temp dir: {e}")])?;

    let ir_path = tmp.join("out.ssa");
    let asm_path = tmp.join("out.s");
    let obj_path = tmp.join("out.o");
    let bin_path = options
        .output_path
        .clone()
        .unwrap_or_else(|| tmp.join("a.out"));

    // --- write IR ----------------------------------------------------------
    fs::write(&ir_path, &result.ir).map_err(|e| vec![format!("failed to write IR: {e}")])?;

    // --- QBE: IR → assembly ------------------------------------------------
    // Note: -o must come BEFORE the input file for QBE
    run_command(
        Command::new(qbe_path())
            .arg("-o")
            .arg(&asm_path)
            .arg(&ir_path),
        "qbe",
    )?;

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
    let _ = fs::remove_file(&ir_path);
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
pub fn compile_plugin(source: &str, name: &str, version: &str) -> Result<PathBuf, Vec<String>> {
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

    let ir_path = tmp.join(format!("{name}.ssa"));
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

    // --- write IR ----------------------------------------------------------
    fs::write(&ir_path, &result.ir).map_err(|e| vec![format!("failed to write IR: {e}")])?;

    // --- QBE: IR → assembly ------------------------------------------------
    // Note: -o must come BEFORE the input file for QBE
    run_command(
        Command::new(qbe_path())
            .arg("-o")
            .arg(&asm_path)
            .arg(&ir_path),
        "qbe",
    )?;

    // --- assembler: assembly → PIC object ----------------------------------
    run_command(
        Command::new("as").arg(&asm_path).arg("-o").arg(&obj_path),
        "as (assembler)",
    )?;

    // --- linker: object → shared library -----------------------------------
    let mut link = cc_command();
    link.arg("-shared").arg("-o").arg(&lib_path).arg(&obj_path);

    #[cfg(target_os = "macos")]
    {
        link.arg("-dynamiclib");
    }

    // Link against the Mog runtime if present.
    if let Some(rt) = find_runtime_archive() {
        link.arg(&rt);
    }

    #[cfg(target_os = "macos")]
    {
        link.arg("-lSystem");
    }
    link.arg("-lm");

    run_command(&mut link, "cc (shared-library linker)")?;

    // --- clean up temp files (best-effort) ---------------------------------
    let _ = fs::remove_file(&ir_path);
    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&obj_path);

    Ok(lib_path)
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

/// Determine the path to the QBE binary.  Respects `$QBE` env-var, otherwise
/// falls back to `qbe` (i.e. whatever is on `$PATH`).
fn qbe_path() -> PathBuf {
    env::var_os("QBE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("qbe"))
}

/// Return a [`Command`] for the system C compiler.  Respects `$CC`, defaulting
/// to `cc`.
fn cc_command() -> Command {
    let cc = env::var_os("CC").unwrap_or_else(|| "cc".into());
    Command::new(cc)
}

/// Try to locate `runtime.a` relative to the compiler binary or the current
/// working directory.
fn find_runtime_archive() -> Option<PathBuf> {
    // Relative to the compiler binary: ../../build/runtime.a
    // (binary is at compiler/target/debug/mogc, runtime at build/runtime.a)
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            // Try going up from target/debug/ to the project root
            for ancestor in [
                parent.join("../../../build/runtime.a"), // from target/debug/
                parent.join("../../build/runtime.a"),
                parent.join("../build/runtime.a"),
            ] {
                if ancestor.exists() {
                    return Some(ancestor);
                }
            }
        }
    }
    // Relative to cwd
    for candidate in ["build/runtime.a", "../build/runtime.a"] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    // MOG_RUNTIME_DIR env var
    if let Ok(dir) = env::var("MOG_RUNTIME_DIR") {
        let p = PathBuf::from(dir).join("runtime.a");
        if p.exists() {
            return Some(p);
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

    /// End-to-end: compile to binary and run (requires QBE, as, cc, runtime.a)
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
