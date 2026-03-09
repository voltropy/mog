// Mog compilation benchmark runner.
//
// Measures compilation speed across Mog, Go, and Rust for equivalent programs
// at three sizes (tiny, medium, large).  Replaces the old TypeScript benchmark.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::process;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use mog::compiler::{compile, compile_to_binary, CompileOptions};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum BenchMode {
    Full,
    HostStop,
    InterruptChecks,
}

struct Args {
    iterations: usize,
    warmup: usize,
    mode: BenchMode,
    adaptive_loop_interrupt_checks: bool,
    loop_interrupt_check_target_micros: u64,
}

fn parse_args() -> Args {
    let mut args = Args {
        iterations: 5,
        warmup: 2,
        mode: BenchMode::Full,
        adaptive_loop_interrupt_checks: false,
        loop_interrupt_check_target_micros: 1_000,
    };
    let argv: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--host-stop-bench" => {
                args.mode = BenchMode::HostStop;
            }
            "--interrupt-check-bench" => {
                args.mode = BenchMode::InterruptChecks;
            }
            "--adaptive-loop-interrupt-checks" => {
                args.adaptive_loop_interrupt_checks = true;
            }
            "--adaptive-loop-interrupt-target-micros" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!(
                        "error: --adaptive-loop-interrupt-target-micros requires a number of microseconds"
                    );
                    process::exit(1);
                }
                args.loop_interrupt_check_target_micros = match argv[i].parse::<u64>() {
                    Ok(v) if v > 0 => v,
                    Ok(_) => {
                        eprintln!(
                            "error: --adaptive-loop-interrupt-target-micros must be greater than 0"
                        );
                        process::exit(1);
                    }
                    Err(_) => {
                        eprintln!(
                            "error: --adaptive-loop-interrupt-target-micros requires a numeric value"
                        );
                        process::exit(1);
                    }
                };
            }
            "--iterations" => {
                i += 1;
                args.iterations = argv[i].parse().expect("--iterations requires a number");
            }
            "--warmup" => {
                i += 1;
                args.warmup = argv[i].parse().expect("--warmup requires a number");
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: mog-bench [--iterations N] [--warmup N] [--host-stop-bench] [--interrupt-check-bench] [--adaptive-loop-interrupt-checks] [--adaptive-loop-interrupt-target-micros N]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown flag: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }
    args
}

#[derive(Clone, Copy)]
struct HostLimits {
    max_memory: usize,
    max_cpu_ms: i32,
    initial_memory: usize,
}

struct HostStopCase {
    label: String,
    source: String,
    limits: HostLimits,
    stop_guard: Duration,
}

struct InterruptCheckCase {
    label: String,
    source_path: PathBuf,
    source: String,
}

const HOST_C_TEMPLATE: &str = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct MogVM MogVM;

typedef struct {
    int32_t tag;
    int32_t pad;
    union {
        int64_t i;
        double f;
        int64_t b;
        const char* s;
        const void* handle;
        const char* error;
    } data;
} MogValue;

typedef struct {
    size_t max_memory;
    int max_cpu_ms;
    int max_stack_depth;
    size_t initial_memory;
} MogLimits;

extern MogVM* mog_vm_new(void);
extern void mog_vm_set_global(MogVM* vm);
extern void mog_vm_set_limits(MogVM* vm, const MogLimits* limits);

static void setup_mog_limits(MogVM* vm) {
    const MogLimits limits = {
        (size_t){{MAX_MEMORY}},
        {{MAX_CPU_MS}},
        1024,
        (size_t){{INITIAL_MEMORY}},
    };
    mog_vm_set_limits(vm, &limits);
}

__attribute__((constructor))
static void setup_mog_vm(void) {
    MogVM* vm = mog_vm_new();
    if (!vm) {
        fprintf(stderr, "bench-host: mog_vm_new failed\n");
        exit(1);
    }
    setup_mog_limits(vm);
    mog_vm_set_global(vm);
}
"#;

static HOST_STOP_COUNTER: AtomicU32 = AtomicU32::new(0);
static INTERRUPT_CHECK_COUNTER: AtomicU32 = AtomicU32::new(0);

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(std::borrow::ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn host_source_for(limits: HostLimits) -> String {
    HOST_C_TEMPLATE
        .replace("{{MAX_MEMORY}}", &limits.max_memory.to_string())
        .replace("{{MAX_CPU_MS}}", &limits.max_cpu_ms.to_string())
        .replace("{{INITIAL_MEMORY}}", &limits.initial_memory.to_string())
}

fn compile_hosted_binary(source: &str, limits: HostLimits, label: &str) -> Option<PathBuf> {
    let counter = HOST_STOP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let host_path = std::env::temp_dir().join(format!("mog-stop-host-{pid}-{counter}.c"));
    let out_path = std::env::temp_dir().join(format!("mog-stop-bin-{pid}-{counter}.out"));

    fs::write(&host_path, host_source_for(limits)).unwrap();

    let options = CompileOptions {
        output_path: Some(out_path.clone()),
        source_path: Some(project_root()),
        extra_link_objects: vec![host_path.clone()],
        ..Default::default()
    };

    match compile_to_binary(source, &options) {
        Ok(path) => Some(path),
        Err(errors) => {
            let msg = errors.join("; ");
            if msg.contains("qbe") || msg.contains("runtime") || msg.contains("not found") {
                eprintln!("skipping host-stop binary '{label}': {msg}");
                None
            } else {
                panic!("compile_to_binary failed for '{label}': {msg}");
            }
        }
    }
}

fn run_with_wall_timeout(executable: &Path, timeout: Duration) -> (Duration, bool, Option<i32>) {
    let mut child = Command::new(executable)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to launch host-stop binary: {e}"));

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return (start.elapsed(), false, status.code());
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return (start.elapsed(), true, None);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn time_host_timeout_binary(executable: &Path, stop_limit: Duration, run_guard: Duration) -> f64 {
    let (elapsed, timed_out, exit_code) = run_with_wall_timeout(executable, run_guard);
    assert!(!timed_out, "benchmark case exceeded wall timeout: elapsed={:?}", elapsed);
    assert!(
        exit_code.is_some(),
        "benchmark case terminated by signal before stop: elapsed={:?}",
        elapsed
    );
    assert!(
        elapsed < stop_limit,
        "benchmark case stopped slower than budget: elapsed={:?}",
        elapsed
    );
    elapsed.as_secs_f64() * 1000.0
}

fn time_host_stop_case(
    case: &HostStopCase,
    warmup: usize,
    iterations: usize,
) -> Option<BenchResult> {
    let binary = compile_hosted_binary(&case.source, case.limits, &case.label)?;

    let stop_limit = case.stop_guard;
    let run_guard = stop_limit * 6;
    let case_label = case.label.clone();
    let path = binary;

    Some(run_bench(
        &case_label,
        1,
        warmup,
        iterations,
        move || time_host_timeout_binary(&path, stop_limit, run_guard),
    ))
}

fn compile_interrupt_binary(
    case: &InterruptCheckCase,
    loop_interrupt_checks: bool,
    adaptive_loop_interrupt_checks: bool,
    loop_interrupt_check_target_micros: u64,
) -> Option<PathBuf> {
    let counter = INTERRUPT_CHECK_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let variant = if loop_interrupt_checks {
        if adaptive_loop_interrupt_checks {
            "checked-adaptive"
        } else {
            "checked"
        }
    } else {
        "unchecked"
    };
    let out_path = std::env::temp_dir().join(format!("mog-interrupt-bench-{pid}-{counter}-{variant}.out"));

    let options = CompileOptions {
        output_path: Some(out_path.clone()),
        source_path: Some(case.source_path.clone()),
        loop_interrupt_checks,
        adaptive_loop_interrupt_checks,
        loop_interrupt_check_target_micros,
        ..Default::default()
    };

    match compile_to_binary(&case.source, &options) {
        Ok(path) => Some(path),
        Err(errors) => {
            let msg = errors.join("; ");
            if msg.contains("qbe") || msg.contains("runtime") || msg.contains("not found") {
                eprintln!(
                    "skipping interrupt-check binary '{} ({variant})': {msg}",
                    case.label
                );
                None
            } else {
                panic!(
                    "compile_to_binary failed for interrupt-check case '{} ({variant})': {msg}",
                    case.label
                );
            }
        }
    }
}

fn time_binary_run(executable: &Path) -> f64 {
    let start = Instant::now();
    let status = Command::new(executable)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("failed to launch benchmark binary {}: {e}", executable.display()));
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    assert!(
        status.success(),
        "benchmark binary failed: path={} status={status:?}",
        executable.display()
    );
    elapsed
}

fn time_interrupt_check_case(
    case: &InterruptCheckCase,
    warmup: usize,
    iterations: usize,
    adaptive_loop_interrupt_checks: bool,
    loop_interrupt_check_target_micros: u64,
) -> Option<InterruptCheckResult> {
    let checked_binary = compile_interrupt_binary(
        case,
        true,
        adaptive_loop_interrupt_checks,
        loop_interrupt_check_target_micros,
    )?;
    let unchecked_binary =
        compile_interrupt_binary(case, false, false, loop_interrupt_check_target_micros)?;
    let lines = count_lines(&case.source);

    let checked_label = format!("{} (checks)", case.label);
    let checked_result = run_bench(&checked_label, lines, warmup, iterations, {
        let checked_binary = checked_binary;
        move || time_binary_run(&checked_binary)
    });

    let unchecked_label = format!("{} (no checks)", case.label);
    let unchecked_result = run_bench(&unchecked_label, lines, warmup, iterations, {
        let unchecked_binary = unchecked_binary;
        move || time_binary_run(&unchecked_binary)
    });

    Some(InterruptCheckResult {
        label: case.label.clone(),
        checked: checked_result,
        unchecked: unchecked_result,
    })
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Stats {
    mean: f64,
    median: f64,
    min: f64,
    max: f64,
    stddev: f64,
}

fn compute_stats(samples: &[f64]) -> Stats {
    assert!(!samples.is_empty());
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<f64>() / n;
    let variance = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n;
    let stddev = variance.sqrt();
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if sorted.len() % 2 == 1 {
        sorted[sorted.len() / 2]
    } else {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    };
    Stats {
        mean,
        median,
        min: sorted[0],
        max: *sorted.last().unwrap(),
        stddev,
    }
}

// ---------------------------------------------------------------------------
// Timing helpers
// ---------------------------------------------------------------------------

fn time_mog_frontend(source: &str, source_path: &Path) -> f64 {
    let opts = CompileOptions {
        source_path: Some(source_path.to_path_buf()),
        ..Default::default()
    };
    let start = Instant::now();
    let result = compile(source, &opts);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    if !result.errors.is_empty() {
        eprintln!(
            "  warning: mog frontend errors for {}: {:?}",
            source_path.display(),
            result.errors
        );
    }
    elapsed
}

fn time_mog_e2e(source: &str, source_path: &Path, output: &Path) -> f64 {
    let opts = CompileOptions {
        source_path: Some(source_path.to_path_buf()),
        output_path: Some(output.to_path_buf()),
        ..Default::default()
    };
    let start = Instant::now();
    let res = compile_to_binary(source, &opts);
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    if let Err(errs) = res {
        eprintln!(
            "  warning: mog e2e errors for {}: {:?}",
            source_path.display(),
            errs
        );
    }
    elapsed
}

fn time_go_build(go_src: &Path, output: &Path) -> f64 {
    let start = Instant::now();
    let status = Command::new("go")
        .args(["build", "-o"])
        .arg(output)
        .arg(go_src)
        .output()
        .expect("failed to run `go build`");
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    if !status.status.success() {
        eprintln!(
            "  warning: go build failed for {}: {}",
            go_src.display(),
            String::from_utf8_lossy(&status.stderr)
        );
    }
    elapsed
}

fn time_rustc(rs_src: &Path, output: &Path) -> f64 {
    let start = Instant::now();
    let status = Command::new("rustc")
        .args(["-O", "-o"])
        .arg(output)
        .arg(rs_src)
        .output()
        .expect("failed to run `rustc`");
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    if !status.status.success() {
        eprintln!(
            "  warning: rustc failed for {}: {}",
            rs_src.display(),
            String::from_utf8_lossy(&status.stderr)
        );
    }
    elapsed
}

fn count_lines(text: &str) -> usize {
    text.chars().filter(|&c| c == '\n').count()
}

fn load_host_stop_cases(bench_dir: &Path) -> Vec<HostStopCase> {
    vec![
        (
            "host-stop-tiny-spin",
            "host-stop-spin.mog",
            HostLimits {
                max_memory: 32 * 1024 * 1024,
                max_cpu_ms: 120,
                initial_memory: 8 * 1024 * 1024,
            },
            Duration::from_millis(700),
        ),
        (
            "host-stop-memory-leak",
            "host-stop-memory.mog",
            HostLimits {
                max_memory: 64 * 1024,
                max_cpu_ms: 180,
                initial_memory: 32 * 1024,
            },
            Duration::from_millis(900),
        ),
        (
            "host-stop-async-suspend",
            "host-stop-async.mog",
            HostLimits {
                max_memory: 16 * 1024 * 1024,
                max_cpu_ms: 120,
                initial_memory: 4 * 1024 * 1024,
            },
            Duration::from_millis(900),
        ),
    ]
    .into_iter()
    .map(|(label, file, limits, stop_guard)| {
        let path = bench_dir.join("mog").join(file);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read benchmark source {}: {e}", path.display()));
        HostStopCase {
            label: label.to_string(),
            source,
            limits,
            stop_guard,
        }
    })
    .collect()
}

fn load_interrupt_check_cases(bench_dir: &Path) -> Vec<InterruptCheckCase> {
    vec![
        ("tight while loop", "interrupt-tight-while.mog"),
        ("tight for-range loop", "interrupt-tight-for-range.mog"),
        ("work-heavy while loop", "interrupt-work-heavy-while.mog"),
    ]
    .into_iter()
    .map(|(label, file)| {
        let path = bench_dir.join("mog").join(file);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read benchmark source {}: {e}", path.display()));
        InterruptCheckCase {
            label: label.to_string(),
            source_path: path,
            source,
        }
    })
    .collect()
}

// ---------------------------------------------------------------------------
// Benchmark entry
// ---------------------------------------------------------------------------

struct BenchResult {
    label: String,
    lines: usize,
    stats: Stats,
}

struct InterruptCheckResult {
    label: String,
    checked: BenchResult,
    unchecked: BenchResult,
}

impl BenchResult {
    fn us_per_line(&self) -> f64 {
        if self.lines == 0 {
            0.0
        } else {
            self.stats.median * 1000.0 / self.lines as f64
        }
    }
}

impl InterruptCheckResult {
    fn checked_median_ms(&self) -> f64 {
        self.checked.stats.median
    }

    fn unchecked_median_ms(&self) -> f64 {
        self.unchecked.stats.median
    }

    fn delta_ms(&self) -> f64 {
        self.checked_median_ms() - self.unchecked_median_ms()
    }

    fn slowdown_ratio(&self) -> f64 {
        self.checked_median_ms() / self.unchecked_median_ms()
    }

    fn slowdown_pct(&self) -> f64 {
        (self.slowdown_ratio() - 1.0) * 100.0
    }
}

fn run_bench(
    label: &str,
    lines: usize,
    warmup: usize,
    iterations: usize,
    mut f: impl FnMut() -> f64,
) -> BenchResult {
    // Warmup runs (discarded).
    for _ in 0..warmup {
        f();
    }
    // Measured runs.
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        samples.push(f());
    }
    BenchResult {
        label: label.to_string(),
        lines,
        stats: compute_stats(&samples),
    }
}

// ---------------------------------------------------------------------------
// Printing
// ---------------------------------------------------------------------------

fn print_header(title: &str) {
    println!();
    println!("{title}");
    println!("{}", "=".repeat(title.len()));
    println!(
        "{:<30} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "benchmark", "mean ms", "median ms", "min ms", "max ms", "stddev ms", "us/line"
    );
    println!("{}", "-".repeat(100));
}

fn print_row(r: &BenchResult) {
    println!(
        "{:<30} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>10.1} {:>10.1}",
        r.label,
        r.stats.mean,
        r.stats.median,
        r.stats.min,
        r.stats.max,
        r.stats.stddev,
        r.us_per_line(),
    );
}

fn print_interrupt_check_header(title: &str) {
    println!();
    println!("{title}");
    println!("{}", "=".repeat(title.len()));
    println!(
        "{:<30} {:>12} {:>14} {:>12} {:>12} {:>12}",
        "benchmark",
        "checked ms",
        "unchecked ms",
        "delta ms",
        "slowdown x",
        "slowdown %"
    );
    println!("{}", "-".repeat(100));
}

fn print_interrupt_check_row(r: &InterruptCheckResult) {
    println!(
        "{:<30} {:>12.3} {:>14.3} {:>12.3} {:>12.3} {:>12.1}",
        r.label,
        r.checked_median_ms(),
        r.unchecked_median_ms(),
        r.delta_ms(),
        r.slowdown_ratio(),
        r.slowdown_pct(),
    );
}

fn run_host_stop_suite(args: &Args, bench_dir: &Path) {
    let stop_cases = load_host_stop_cases(bench_dir);
    let mut stop_results: Vec<BenchResult> = Vec::new();
    let mut skipped = 0usize;

    println!(
        "host-stop benchmark iterations={}  warmup={}",
        args.iterations, args.warmup
    );

    for case in &stop_cases {
        match time_host_stop_case(case, args.warmup, args.iterations) {
            Some(result) => stop_results.push(result),
            None => skipped += 1,
        }
    }

    if stop_results.is_empty() {
        eprintln!("no host-stop benchmark cases ran (all skipped)");
    } else {
        print_header("host-stop worst-case latency");
        for result in &stop_results {
            print_row(result);
        }
    }

    if skipped > 0 {
        eprintln!("skipped {skipped} host-stop benchmark case(s)");
    }

    println!();
    println!("done.");
}

fn run_interrupt_check_suite(args: &Args, bench_dir: &Path) {
    let cases = load_interrupt_check_cases(bench_dir);
    let mut results: Vec<InterruptCheckResult> = Vec::new();
    let mut skipped = 0usize;
    let checks_mode = if args.adaptive_loop_interrupt_checks {
        format!("adaptive (target {}us)", args.loop_interrupt_check_target_micros)
    } else {
        "every loop edge".to_string()
    };

    println!(
        "interrupt-check benchmark iterations={}  warmup={}  mode={checks_mode}",
        args.iterations, args.warmup
    );
    println!("timing binary execution only; each case is compiled once with checks and once without");

    for case in &cases {
        match time_interrupt_check_case(
            case,
            args.warmup,
            args.iterations,
            args.adaptive_loop_interrupt_checks,
            args.loop_interrupt_check_target_micros,
        ) {
            Some(result) => results.push(result),
            None => skipped += 1,
        }
    }

    if results.is_empty() {
        eprintln!("no interrupt-check benchmark cases ran (all skipped)");
    } else {
        let header = if args.adaptive_loop_interrupt_checks {
            "adaptive loop interrupt-check slowdown"
        } else {
            "loop-edge interrupt-check slowdown"
        };
        print_interrupt_check_header(header);
        for result in &results {
            print_interrupt_check_row(result);
        }
    }

    if skipped > 0 {
        eprintln!("skipped {skipped} interrupt-check benchmark case(s)");
    }

    println!();
    println!("done.");
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();

    // Resolve project root (benchmarks crate lives at <root>/benchmarks).
    let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sizes = ["tiny", "medium", "large"];

    println!(
        "mog-bench  iterations={}  warmup={}",
        args.iterations, args.warmup
    );

    match args.mode {
        BenchMode::HostStop => {
            run_host_stop_suite(&args, &bench_dir);
            return;
        }
        BenchMode::InterruptChecks => {
            run_interrupt_check_suite(&args, &bench_dir);
            return;
        }
        BenchMode::Full => {}
    }

    // Pre-read all source files.
    let mog_sources: Vec<(String, String, PathBuf)> = sizes
        .iter()
        .map(|s| {
            let path = bench_dir.join(format!("mog/{s}.mog"));
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            (s.to_string(), src, path)
        })
        .collect();

    let go_sources: Vec<(String, PathBuf)> = sizes
        .iter()
        .map(|s| {
            let path = bench_dir.join(format!("go/{s}.go"));
            assert!(path.exists(), "missing {}", path.display());
            (s.to_string(), path)
        })
        .collect();

    let rust_sources: Vec<(String, PathBuf, String)> = sizes
        .iter()
        .map(|s| {
            let path = bench_dir.join(format!("rust/{s}.rs"));
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            (s.to_string(), path, src)
        })
        .collect();

    // Collect all results grouped by category.
    let mut frontend_results: Vec<BenchResult> = Vec::new();
    let mut mog_e2e_results: Vec<BenchResult> = Vec::new();
    let mut go_results: Vec<BenchResult> = Vec::new();
    let mut rustc_results: Vec<BenchResult> = Vec::new();

    for (size, src, path) in &mog_sources {
        let lines = count_lines(src);

        let src_fe = src.clone();
        let path_fe = path.clone();
        let r = run_bench(
            &format!("mog frontend {size}"),
            lines,
            args.warmup,
            args.iterations,
            || time_mog_frontend(&src_fe, &path_fe),
        );
        frontend_results.push(r);

        let src_e2e = src.clone();
        let path_e2e = path.clone();
        let out = PathBuf::from(format!("/tmp/mog_bench_mog_{size}"));
        let r = run_bench(
            &format!("mog e2e {size}"),
            lines,
            args.warmup,
            args.iterations,
            || time_mog_e2e(&src_e2e, &path_e2e, &out),
        );
        mog_e2e_results.push(r);
    }

    for (size, path) in &go_sources {
        let go_src = fs::read_to_string(path).unwrap();
        let lines = count_lines(&go_src);
        let out = PathBuf::from(format!("/tmp/mog_bench_go_{size}"));
        let p = path.clone();
        let r = run_bench(
            &format!("go build {size}"),
            lines,
            args.warmup,
            args.iterations,
            || time_go_build(&p, &out),
        );
        go_results.push(r);
    }

    for (size, path, src) in &rust_sources {
        let lines = count_lines(src);
        let out = PathBuf::from(format!("/tmp/mog_bench_rust_{size}"));
        let p = path.clone();
        let r = run_bench(
            &format!("rustc {size}"),
            lines,
            args.warmup,
            args.iterations,
            || time_rustc(&p, &out),
        );
        rustc_results.push(r);
    }

    // -- Print results -------------------------------------------------------

    // Per-size tables.
    for (i, size) in sizes.iter().enumerate() {
        let title = format!("{size} programs");
        print_header(&title);
        print_row(&frontend_results[i]);
        print_row(&mog_e2e_results[i]);
        print_row(&go_results[i]);
        print_row(&rustc_results[i]);
    }

    // Frontend-only section.
    print_header("Mog frontend only");
    for r in &frontend_results {
        print_row(r);
    }

    // Cross-language comparison (median, end-to-end).
    println!();
    let title = "Cross-language comparison (median end-to-end ms)";
    println!("{title}");
    println!("{}", "=".repeat(title.len()));
    println!(
        "{:<10} {:>12} {:>12} {:>12}",
        "size", "mog e2e", "go build", "rustc"
    );
    println!("{}", "-".repeat(50));
    for i in 0..sizes.len() {
        println!(
            "{:<10} {:>12.1} {:>12.1} {:>12.1}",
            sizes[i],
            mog_e2e_results[i].stats.median,
            go_results[i].stats.median,
            rustc_results[i].stats.median,
        );
    }

    println!();
    println!("done.");
}
