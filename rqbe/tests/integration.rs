//! Integration tests for rqbe.
//!
//! Uses the QBE test suite from vendor/qbe-1.2/test/. Each .ssa file
//! embeds a C driver and expected output. We:
//!   1. Parse the .ssa file to extract QBE IL, C driver, and expected output
//!   2. Compile the QBE IL via rqbe::compile()
//!   3. Assemble the output, link with the C driver, run, and check output
//!
//! These tests will fail until the implementation is complete — that's expected.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rqbe::Target;

/// Build a Target suitable for the current platform.
///
/// On macOS/arm64, this is the arm64-apple target.
fn make_target() -> Target {
    Target {
        name: "arm64",
        apple: cfg!(target_os = "macos"),
        gpr0: 0,
        ngpr: 28,
        fpr0: 32,
        nfpr: 30,
        rglob: 0,
        nrglob: 0,
        rsave: &[],
        nrsave: [0, 0],
    }
}

/// Sections extracted from a QBE .ssa test file.
struct SsaTestFile {
    /// The QBE IL source (everything outside driver/output blocks).
    qbe_source: String,
    /// The embedded C driver (between `# >>> driver` and `# <<<`).
    c_driver: String,
    /// The expected stdout output (between `# >>> output` and `# <<<`).
    expected_output: String,
}

/// Parse a QBE .ssa test file, extracting the IL source, C driver, and expected output.
///
/// The format uses comment blocks:
///   # >>> driver
///   # <C code>
///   # <<<
///   # >>> output
///   # <expected output>
///   # <<<
///
/// Lines in driver/output blocks have their leading `# ` stripped.
/// Lines that are just `#` (trailing space already stripped) become empty lines.
fn parse_ssa_file(contents: &str) -> SsaTestFile {
    let mut qbe_lines = Vec::new();
    let mut driver_lines = Vec::new();
    let mut output_lines = Vec::new();

    #[derive(PartialEq)]
    enum Section {
        Qbe,
        Driver,
        Output,
    }

    let mut section = Section::Qbe;

    for line in contents.lines() {
        match section {
            Section::Qbe => {
                if line.starts_with("# >>> driver") {
                    section = Section::Driver;
                } else if line.starts_with("# >>> output") {
                    section = Section::Output;
                } else {
                    qbe_lines.push(line);
                }
            }
            Section::Driver => {
                if line.starts_with("# <<<") {
                    section = Section::Qbe;
                } else {
                    // Strip leading "# " or just "#"
                    let stripped = line
                        .strip_prefix("# ")
                        .unwrap_or_else(|| line.strip_prefix('#').unwrap_or(line));
                    driver_lines.push(stripped);
                }
            }
            Section::Output => {
                if line.starts_with("# <<<") {
                    section = Section::Qbe;
                } else {
                    let stripped = line
                        .strip_prefix("# ")
                        .unwrap_or_else(|| line.strip_prefix('#').unwrap_or(line));
                    output_lines.push(stripped);
                }
            }
        }
    }

    SsaTestFile {
        qbe_source: qbe_lines.join("\n"),
        c_driver: driver_lines.join("\n"),
        expected_output: output_lines.join("\n"),
    }
}

/// Resolve the path to a QBE test .ssa file.
fn ssa_test_path(test_name: &str) -> PathBuf {
    // Integration tests run from the crate root, so ../vendor is correct.
    let p = PathBuf::from(format!("../vendor/qbe-1.2/test/{}.ssa", test_name));
    assert!(p.exists(), "Test file not found: {}", p.display());
    p
}

/// Run a full integration test for a .ssa file from the QBE test suite.
///
/// Steps:
///   1. Read and parse the .ssa file
///   2. Compile the QBE IL to assembly via rqbe::compile()
///   3. Write the assembly to a temp file
///   4. If a C driver is present, write it to a temp file
///   5. Assemble + link with `cc`
///   6. Run the resulting binary, capturing stdout and exit code
///   7. Compare stdout against expected output (if any), or check exit code == 0
fn run_ssa_test(test_name: &str) {
    let ssa_path = ssa_test_path(test_name);
    let contents = fs::read_to_string(&ssa_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", ssa_path.display(), e));

    let test_file = parse_ssa_file(&contents);

    // Step 2: Compile QBE IL to assembly.
    let target = make_target();
    let asm = rqbe::compile(&test_file.qbe_source, &target)
        .unwrap_or_else(|e| panic!("rqbe::compile failed for {}: {}", test_name, e));

    // Step 3-4: Write temp files.
    let tmp_dir = std::env::temp_dir().join(format!("rqbe_test_{}", test_name));
    fs::create_dir_all(&tmp_dir).expect("Failed to create temp dir");

    let asm_path = tmp_dir.join("out.s");
    let drv_path = tmp_dir.join("driver.c");
    let exe_path = tmp_dir.join("test_exe");

    fs::write(&asm_path, &asm).unwrap_or_else(|e| panic!("Failed to write assembly: {}", e));

    // Step 5: Compile and link.
    let mut cc_args: Vec<&str> = vec!["-g", "-o"];
    let exe_str = exe_path.to_str().unwrap();
    let asm_str = asm_path.to_str().unwrap();
    let drv_str = drv_path.to_str().unwrap();

    cc_args.push(exe_str);

    let has_driver = !test_file.c_driver.trim().is_empty();
    if has_driver {
        fs::write(&drv_path, &test_file.c_driver)
            .unwrap_or_else(|e| panic!("Failed to write C driver: {}", e));
        cc_args.push(drv_str);
    }

    cc_args.push(asm_str);

    let cc_output = Command::new("cc")
        .args(&cc_args)
        .output()
        .expect("Failed to run cc (C compiler)");

    assert!(
        cc_output.status.success(),
        "cc failed for {}:\nstdout: {}\nstderr: {}",
        test_name,
        String::from_utf8_lossy(&cc_output.stdout),
        String::from_utf8_lossy(&cc_output.stderr),
    );

    // Step 6: Run the binary with args "a b c" (matching QBE's test.sh).
    let run_output = Command::new(&exe_path)
        .args(&["a", "b", "c"])
        .output()
        .unwrap_or_else(|e| panic!("Failed to run test binary for {}: {}", test_name, e));

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    let exit_code = run_output.status.code().unwrap_or(-1);

    // Step 7: Compare results.
    let has_expected_output = !test_file.expected_output.trim().is_empty();

    if has_expected_output {
        // Compare stdout line-by-line against expected output.
        let actual = stdout.trim_end();
        let expected = test_file.expected_output.trim_end();
        assert_eq!(
            actual, expected,
            "Output mismatch for test '{}'\n--- expected ---\n{}\n--- actual ---\n{}",
            test_name, expected, actual
        );
    } else {
        // No expected output: just check that exit code is 0.
        assert_eq!(
            exit_code, 0,
            "Test '{}' exited with code {} (expected 0)\nstdout: {}",
            test_name, exit_code, stdout
        );
    }

    // Cleanup temp files.
    let _ = fs::remove_dir_all(&tmp_dir);
}

// ---------------------------------------------------------------------------
// Parsing infrastructure tests
// ---------------------------------------------------------------------------

#[test]
fn test_parse_ssa_file_format() {
    let input = r#"
export
function w $add(w %a, w %b) {
@start
    %c =w add %a, %b
    ret %c
}

# >>> driver
# int add(int, int);
# int main() { return !(add(1, 2) == 3); }
# <<<

# >>> output
# hello world
# <<<
"#;

    let parsed = parse_ssa_file(input);

    assert!(parsed.qbe_source.contains("function w $add"));
    assert!(parsed.qbe_source.contains("ret %c"));
    assert!(!parsed.qbe_source.contains(">>> driver"));
    assert!(!parsed.qbe_source.contains(">>> output"));

    assert!(parsed.c_driver.contains("int add(int, int)"));
    assert!(parsed.c_driver.contains("int main()"));

    assert_eq!(parsed.expected_output.trim(), "hello world");
}

#[test]
fn test_parse_ssa_file_no_output() {
    let input = r#"
function w $f() {
@start
    ret 0
}

# >>> driver
# int f(void);
# int main() { return f(); }
# <<<
"#;

    let parsed = parse_ssa_file(input);
    assert!(parsed.c_driver.contains("int f(void)"));
    assert!(parsed.expected_output.trim().is_empty());
}

#[test]
fn test_parse_ssa_file_no_driver() {
    let input = r#"
function w $main() {
@start
    ret 0
}
"#;

    let parsed = parse_ssa_file(input);
    assert!(parsed.c_driver.trim().is_empty());
    assert!(parsed.expected_output.trim().is_empty());
    assert!(parsed.qbe_source.contains("function w $main"));
}

#[test]
fn test_ssa_test_files_exist() {
    // Verify that the QBE test files we reference actually exist.
    let test_dir = Path::new("../vendor/qbe-1.2/test");
    assert!(test_dir.exists(), "QBE test directory not found");

    let required = [
        "sum", "eucl", "collatz", "prime", "queen", "mandel", "abi1", "abi2", "abi3", "abi4",
        "abi5", "abi6", "abi7", "abi8", "fpcnv", "double", "load1", "load2", "load3", "mem1",
        "mem2", "mem3", "isel1", "isel2", "isel3", "spill1", "rega1",
    ];

    for name in &required {
        let p = test_dir.join(format!("{}.ssa", name));
        assert!(p.exists(), "Missing test file: {}.ssa", name);
    }
}

// ---------------------------------------------------------------------------
// Full integration tests — one per .ssa file
// ---------------------------------------------------------------------------
// These call run_ssa_test() which compiles QBE IL, assembles, links, runs,
// and checks output. They will fail until the implementation is complete.

// --- Basic algorithms ---

#[test]
#[ignore]
fn test_sum() {
    run_ssa_test("sum");
}

#[test]
#[ignore]
fn test_eucl() {
    run_ssa_test("eucl");
}

#[test]
#[ignore]
fn test_euclc() {
    run_ssa_test("euclc");
}

#[test]
#[ignore]
fn test_collatz() {
    run_ssa_test("collatz");
}

#[test]
#[ignore]
fn test_prime() {
    run_ssa_test("prime");
}

#[test]
#[ignore]
fn test_queen() {
    run_ssa_test("queen");
}

#[test]
#[ignore]
fn test_mandel() {
    run_ssa_test("mandel");
}

#[test]
#[ignore]
fn test_cprime() {
    run_ssa_test("cprime");
}

// --- ABI tests ---

#[test]
#[ignore]
fn test_abi1() {
    run_ssa_test("abi1");
}

#[test]
#[ignore]
fn test_abi2() {
    run_ssa_test("abi2");
}

#[test]
#[ignore]
fn test_abi3() {
    run_ssa_test("abi3");
}

#[test]
#[ignore]
fn test_abi4() {
    run_ssa_test("abi4");
}

#[test]
#[ignore]
fn test_abi5() {
    run_ssa_test("abi5");
}

#[test]
#[ignore]
fn test_abi6() {
    run_ssa_test("abi6");
}

#[test]
#[ignore]
fn test_abi7() {
    run_ssa_test("abi7");
}

#[test]
#[ignore]
fn test_abi8() {
    run_ssa_test("abi8");
}

// --- Floating point ---

#[test]
#[ignore]
fn test_fpcnv() {
    run_ssa_test("fpcnv");
}

#[test]
#[ignore]
fn test_double() {
    run_ssa_test("double");
}

// --- Load/store tests ---

#[test]
#[ignore]
fn test_load1() {
    run_ssa_test("load1");
}

#[test]
#[ignore]
fn test_load2() {
    run_ssa_test("load2");
}

#[test]
#[ignore]
fn test_load3() {
    run_ssa_test("load3");
}

#[test]
#[ignore]
fn test_ldbits() {
    run_ssa_test("ldbits");
}

#[test]
#[ignore]
fn test_ldhoist() {
    run_ssa_test("ldhoist");
}

// --- Memory tests ---

#[test]
#[ignore]
fn test_mem1() {
    run_ssa_test("mem1");
}

#[test]
#[ignore]
fn test_mem2() {
    run_ssa_test("mem2");
}

#[test]
#[ignore]
fn test_mem3() {
    run_ssa_test("mem3");
}

// --- Instruction selection ---

#[test]
#[ignore]
fn test_isel1() {
    run_ssa_test("isel1");
}

#[test]
#[ignore]
fn test_isel2() {
    run_ssa_test("isel2");
}

#[test]
#[ignore]
fn test_isel3() {
    run_ssa_test("isel3");
}

// --- Register allocation and spilling ---

#[test]
#[ignore]
fn test_spill1() {
    run_ssa_test("spill1");
}

#[test]
#[ignore]
fn test_rega1() {
    run_ssa_test("rega1");
}

// --- Comparison / conditional ---

#[test]
#[ignore]
fn test_cmp1() {
    run_ssa_test("cmp1");
}

// --- Constant folding ---

#[test]
#[ignore]
fn test_fold1() {
    run_ssa_test("fold1");
}

// --- Control flow ---

#[test]
#[ignore]
fn test_loop() {
    run_ssa_test("loop");
}

#[test]
#[ignore]
fn test_philv() {
    run_ssa_test("philv");
}

// --- Miscellaneous ---

#[test]
#[ignore]
fn test_align() {
    run_ssa_test("align");
}

#[test]
#[ignore]
fn test_conaddr() {
    run_ssa_test("conaddr");
}

#[test]
#[ignore]
fn test_cup() {
    run_ssa_test("cup");
}

#[test]
#[ignore]
fn test_dark() {
    run_ssa_test("dark");
}

#[test]
#[ignore]
fn test_dynalloc() {
    run_ssa_test("dynalloc");
}

#[test]
#[ignore]
fn test_echo() {
    run_ssa_test("echo");
}

#[test]
#[ignore]
fn test_env() {
    run_ssa_test("env");
}

#[test]
#[ignore]
fn test_fixarg() {
    run_ssa_test("fixarg");
}

#[test]
#[ignore]
fn test_max() {
    run_ssa_test("max");
}

#[test]
#[ignore]
fn test_puts10() {
    run_ssa_test("puts10");
}

#[test]
#[ignore]
fn test_strcmp() {
    run_ssa_test("strcmp");
}

#[test]
#[ignore]
fn test_strspn() {
    run_ssa_test("strspn");
}

#[test]
#[ignore]
fn test_tls() {
    run_ssa_test("tls");
}

#[test]
#[ignore]
fn test_vararg1() {
    run_ssa_test("vararg1");
}

#[test]
#[ignore]
fn test_vararg2() {
    run_ssa_test("vararg2");
}
