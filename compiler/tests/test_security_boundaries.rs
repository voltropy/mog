//! Security-focused integration tests for hostile Mog programs.
//!
//! These tests compile Mog binaries with a temporary host that sets VM limits
//! and verify that the host regains control under CPU timeout and memory
//! pressure, including async-suspended execution.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use mog::compiler::{compile_to_binary, CompileOptions};

struct HostLimits {
    max_memory: usize,
    max_cpu_ms: i32,
    initial_memory: usize,
}

struct RunOutcome {
    elapsed: Duration,
    timed_out: bool,
    exit_code: Option<i32>,
    signal: Option<i32>,
}

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

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
        fprintf(stderr, "security-harness: mog_vm_new failed\n");
        exit(1);
    }
    setup_mog_limits(vm);
    mog_vm_set_global(vm);
}
"#;

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
    let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let host_path = std::env::temp_dir().join(format!("mog-sec-host-{pid}-{counter}.c"));
    let out_path = std::env::temp_dir().join(format!("mog-sec-bin-{pid}-{counter}.out"));

    eprintln!("compiling security harness {label}: {out_path:?}");

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
                eprintln!(
                    "skipping security binary '{label}': missing toolchain/runtime ({msg})"
                );
                None
            } else {
                panic!("compile_to_binary failed for '{label}': {msg}");
            }
        }
    }
}

fn run_with_timeout(executable: &Path, timeout: Duration) -> RunOutcome {
    let mut child = Command::new(executable)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to launch guest binary: {e}"));

    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            #[cfg(unix)]
            let signal = status.signal();
            #[cfg(not(unix))]
            let signal = None;
            return RunOutcome {
                elapsed: start.elapsed(),
                timed_out: false,
                exit_code: status.code(),
                signal,
            };
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let status = child.wait().unwrap();
            #[cfg(unix)]
            let signal = status.signal();
            #[cfg(not(unix))]
            let signal = None;
            return RunOutcome {
                elapsed: start.elapsed(),
                timed_out: true,
                exit_code: status.code(),
                signal,
            };
        }

        thread::sleep(Duration::from_millis(5));
    }
}

fn expect_fast_stop(source: &str, limits: HostLimits, source_name: &str, stop_guard: Duration) {
    let binary = match compile_hosted_binary(source, limits, source_name) {
        Some(path) => path,
        None => return,
    };

    let run = run_with_timeout(&binary, stop_guard);
    assert!(
        !run.timed_out,
        "host did not regain control for {source_name}: elapsed={:?}",
        run.elapsed
    );
    assert!(
        run.exit_code.is_some(),
        "guest process was terminated by signal for {source_name}: elapsed={:?}, signal={:?}",
        run.elapsed,
        run.signal
    );
    assert!(
        run.elapsed < stop_guard,
        "host stop path for {source_name} exceeded limit: elapsed={:?}",
        run.elapsed
    );
}

fn expect_fast_error_exit(
    source: &str,
    limits: HostLimits,
    source_name: &str,
    stop_guard: Duration,
) -> Option<i32> {
    let binary = match compile_hosted_binary(source, limits, source_name) {
        Some(path) => path,
        None => return None,
    };

    let run = run_with_timeout(&binary, stop_guard);
    assert!(
        !run.timed_out,
        "host did not regain control for {source_name}: elapsed={:?}",
        run.elapsed
    );
    let exit_code = match run.exit_code {
        Some(code) => code,
        None => panic!("guest process was terminated by signal for {source_name}: elapsed={:?}", run.elapsed),
    };
    assert!(
        exit_code != 0,
        "guest program should fail for {source_name}: exit_code={exit_code}"
    );
    assert!(
        run.elapsed < stop_guard,
        "host stop path for {source_name} exceeded limit: elapsed={:?}",
        run.elapsed
    );

    Some(exit_code)
}

#[test]
fn hostile_spin_loop_is_cut_off_by_host_timeout() {
    let source = r#"
fn main() -> int {
while 1 {
    }
    return 0;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 32 * 1024 * 1024,
            max_cpu_ms: 120,
            initial_memory: 8 * 1024 * 1024,
        },
        "spin-loop",
        Duration::from_millis(700),
    );
}

#[test]
fn hostile_memory_pressure_is_stopped_without_process_corruption() {
    let source = r#"
fn main() -> int {
    arr := [0];
    i := 0;
while 1 {
        arr.push(i);
        i = i + 1;
    }
    return 0;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 64 * 1024,
            max_cpu_ms: 300,
            initial_memory: 32 * 1024,
        },
        "memory-pressure",
        Duration::from_millis(700),
    );
}

#[test]
fn hostile_spin_with_recursive_burn_can_still_be_stopped() {
    let source = r#"
fn expensive(x: int) -> int {
    if x <= 0 {
        return 0;
    }
    return expensive(x - 1) + expensive(x - 1);
}

fn main() -> int {
    while 1 {
        tmp := expensive(10);
    }
    return 0;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 16 * 1024 * 1024,
            max_cpu_ms: 100,
            initial_memory: 4 * 1024 * 1024,
        },
        "recursive-burn",
        Duration::from_millis(900),
    );
}

#[test]
fn memory_can_be_reclaimed_and_host_still_stops_near_limit() {
    let source = r#"
fn main() -> int {
    while 1 {
        i := 0;
        sink := [0];
        while i < 2048 {
            sink.push(i);
            i = i + 1;
        }
    }
    return 0;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 64 * 1024,
            max_cpu_ms: 180,
            initial_memory: 32 * 1024,
        },
        "reclaimable-alloc",
        Duration::from_millis(900),
    );
}

#[test]
fn async_process_sleep_never_blocks_forever_with_host_timeout() {
    let source = r#"
requires process

async fn main() -> int {
    sleep_ms: int = await process.sleep(30000);
    return sleep_ms;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 16 * 1024 * 1024,
            max_cpu_ms: 120,
            initial_memory: 4 * 1024 * 1024,
        },
        "async-sleep",
        Duration::from_millis(700),
    );
}

#[test]
fn async_process_never_finishes_already_suspended_case_is_stoppable() {
    let source = r#"
requires process

async fn main() -> int {
    while 1 {
        sleep_ms: int = await process.sleep(30000);
        if sleep_ms < 0 {
            return 0;
        }
    }
    return 0;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 24 * 1024 * 1024,
            max_cpu_ms: 120,
            initial_memory: 8 * 1024 * 1024,
        },
        "async-never-finishes",
        Duration::from_millis(900),
    );
}

#[test]
fn nested_hostile_timeout_paths_are_repeatedly_resilient() {
    let source = r#"
fn main() -> int {
    i := 0;
    while 1 {
        j := 0;
        while j < 4096 {
            j = j + 1;
        }
        i = i + 1;
    }
    return 0;
}"#;

    expect_fast_stop(
        source,
        HostLimits {
            max_memory: 8 * 1024 * 1024,
            max_cpu_ms: 120,
            initial_memory: 2 * 1024 * 1024,
        },
        "nested-timeout",
        Duration::from_millis(700),
    );
}

#[test]
fn array_access_beyond_end_stops_program_with_error() {
    let source = r#"
fn main() -> int {
    arr := [10, 20, 30];
    return arr[3];
}"#;

    let Some(exit_code) = expect_fast_error_exit(
        source,
        HostLimits {
            max_memory: 16 * 1024 * 1024,
            max_cpu_ms: 200,
            initial_memory: 4 * 1024 * 1024,
        },
        "array-out-of-bounds",
        Duration::from_millis(500),
    ) else {
        return;
    };

    assert!(exit_code > 0);
    assert_eq!(exit_code, 2);
}
