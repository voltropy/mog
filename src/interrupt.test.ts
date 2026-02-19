import { describe, test, expect } from "bun:test"
import { compile } from "./compiler"
import { spawnSync } from "child_process"
import { writeFileSync, mkdirSync, rmSync } from "fs"
import { join } from "path"
import { tmpdir } from "os"

describe("Cooperative Interrupt", () => {
  describe("IR Generation - interrupt checks at loop back-edges", () => {
    test("while loop has interrupt check", async () => {
      const source = `fn main() -> int {
  x: i64 = 0;
  while (x < 10) {
    x = x + 1;
  }
  return x;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      expect(result.llvmIR).toContain("@mog_interrupt_flag")
      expect(result.llvmIR).toContain("load volatile i32, ptr @mog_interrupt_flag")
      expect(result.llvmIR).toContain("; Cooperative interrupt check")
    })

    test("for loop has interrupt check", async () => {
      const source = `fn main() -> int {
  sum: i64 = 0;
  for i := 1 to 10 {
    sum = sum + i;
  }
  return sum;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      expect(result.llvmIR).toContain("load volatile i32, ptr @mog_interrupt_flag")
    })

    test("for-in range loop has interrupt check", async () => {
      const source = `fn main() -> int {
  sum: i64 = 0;
  for i in 0..10 {
    sum = sum + i;
  }
  return sum;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      expect(result.llvmIR).toContain("load volatile i32, ptr @mog_interrupt_flag")
    })

    test("for-each loop has interrupt check", async () => {
      const source = `fn main() -> int {
  arr := [1, 2, 3, 4, 5];
  sum: i64 = 0;
  for v in arr {
    sum = sum + v;
  }
  return sum;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      expect(result.llvmIR).toContain("load volatile i32, ptr @mog_interrupt_flag")
    })

    test("interrupt check branches to ret i64 -99 in non-void function", async () => {
      const source = `fn main() -> int {
  x: i64 = 0;
  while (x < 10) {
    x = x + 1;
  }
  return x;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      expect(result.llvmIR).toContain("ret i64 -99")
    })

    test("interrupt check branches to exit(99) in void function (no main)", async () => {
      const source = `x: i64 = 0;
while (x < 10) {
  x = x + 1;
}
print(x);`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      // void program() uses exit(99) since it can't return a value
      expect(result.llvmIR).toContain("call void @exit(i32 99)")
    })

    test("nested loops have multiple interrupt checks", async () => {
      const source = `fn main() -> int {
  sum: i64 = 0;
  for i := 1 to 5 {
    for j := 1 to 5 {
      sum = sum + (i * j);
    }
  }
  return sum;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      // Should have at least 2 interrupt checks (one for each loop)
      const checkCount = (result.llvmIR.match(/; Cooperative interrupt check/g) || []).length
      expect(checkCount).toBeGreaterThanOrEqual(2)
    })

    test("normal loops still work correctly (no false interrupts)", async () => {
      const source = `fn main() -> int {
  sum: i64 = 0;
  for i := 1 to 100 {
    sum = sum + i;
  }
  return sum;
}`
      const result = await compile(source)
      expect(result.errors.length).toBe(0)
      // The interrupt flag is external, so when not set (default = 0), loops run normally
      expect(result.llvmIR).toContain("@mog_interrupt_flag = external global i32")
    })
  })

  describe("Runtime - interrupt flag behavior", () => {
    async function compileAndRunWithHost(mogSource: string, hostSource: string): Promise<{ exitCode: number, stdout: string, stderr: string }> {
      const sourceDir = join(tmpdir(), `mog-interrupt-test-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`)

      try {
        mkdirSync(sourceDir, { recursive: true })

        const mogFile = join(sourceDir, "test.mog")
        const llFile = join(sourceDir, "test.ll")
        const oFile = join(sourceDir, "test.o")
        const hostFile = join(sourceDir, "host.c")
        const hostOFile = join(sourceDir, "host.o")
        const exeFile = join(sourceDir, "test")
        const runtimePath = join(process.cwd(), "build", "runtime.a")
        const runtimeInclude = join(process.cwd(), "runtime")
        const llcPath = "/opt/homebrew/opt/llvm/bin/llc"

        writeFileSync(mogFile, mogSource)
        writeFileSync(hostFile, hostSource)

        const result = await compile(mogSource)
        if (result.errors.length > 0) {
          throw new Error(`Compile errors: ${result.errors.map((e: any) => e.message).join(", ")}`)
        }
        writeFileSync(llFile, result.llvmIR)

        // Compile Mog to object (need -O1 for coroutine lowering)
        const llcResult = spawnSync(llcPath, ["-filetype=obj", llFile, "-o", oFile], { stdio: "pipe" })
        if (llcResult.status !== 0) {
          throw new Error(`llc failed: ${llcResult.stderr}`)
        }

        // Compile host C
        const hostCompile = spawnSync("clang", ["-c", `-I${runtimeInclude}`, hostFile, "-o", hostOFile], { stdio: "pipe" })
        if (hostCompile.status !== 0) {
          throw new Error(`host compile failed: ${hostCompile.stderr}`)
        }

        // Link
        const link = spawnSync("clang", [oFile, hostOFile, runtimePath, "-o", exeFile, "-lm"], { stdio: "pipe" })
        if (link.status !== 0 && !link.stderr.toString().includes("no platform load command")) {
          throw new Error(`link failed: ${link.stderr}`)
        }

        // Run with timeout
        const run = spawnSync(exeFile, [], { stdio: "pipe", timeout: 5000 })
        return {
          exitCode: run.status ?? -1,
          stdout: run.stdout ? run.stdout.toString() : "",
          stderr: run.stderr ? run.stderr.toString() : "",
        }
      } finally {
        rmSync(sourceDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 })
      }
    }

    test("infinite loop terminates when interrupt flag is pre-set", async () => {
      const mogSource = `fn main() -> int {
  x: i64 = 0;
  while (1) {
    x = x + 1;
  }
  return 0;
}`
      // Host sets interrupt flag before program runs
      const hostSource = `
#include "mog.h"
extern volatile int mog_interrupt_flag;

__attribute__((constructor))
static void setup(void) {
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);
    // Pre-set the interrupt flag: the loop should exit immediately
    mog_interrupt_flag = 1;
}
`
      const result = await compileAndRunWithHost(mogSource, hostSource)
      // program_user returns -99, main truncates to i32: -99
      // Shell exit codes are 0-255, so -99 becomes 157 (256-99)
      expect(result.exitCode).toBe(157) // -99 as unsigned byte
    })

    test("normal program runs fine when interrupt flag is not set", async () => {
      const mogSource = `fn main() -> int {
  sum: i64 = 0;
  for i := 1 to 100 {
    sum = sum + i;
  }
  return 0;
}`
      const hostSource = `
#include "mog.h"
__attribute__((constructor))
static void setup(void) {
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);
}
`
      const result = await compileAndRunWithHost(mogSource, hostSource)
      expect(result.exitCode).toBe(0)
    })

    test("timeout arms interrupt after delay", async () => {
      const mogSource = `fn main() -> int {
  x: i64 = 0;
  while (1) {
    x = x + 1;
  }
  return 0;
}`
      // Host arms a 50ms timeout — the infinite loop should be interrupted
      const hostSource = `
#include "mog.h"
__attribute__((constructor))
static void setup(void) {
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);
    MogLimits limits = { .max_memory = 0, .max_cpu_ms = 50, .max_stack_depth = 0 };
    mog_vm_set_limits(vm, &limits);
}
`
      const result = await compileAndRunWithHost(mogSource, hostSource)
      // Should exit with interrupt code, not hang forever
      expect(result.exitCode).toBe(157) // -99 as unsigned byte
    })

    test("mog_request_interrupt API works", async () => {
      const mogSource = `fn main() -> int {
  x: i64 = 0;
  while (1) {
    x = x + 1;
  }
  return 0;
}`
      const hostSource = `
#include "mog.h"
__attribute__((constructor))
static void setup(void) {
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);
    mog_request_interrupt();
}
`
      const result = await compileAndRunWithHost(mogSource, hostSource)
      expect(result.exitCode).toBe(157)
    })

    test("mog_clear_interrupt resets the flag", async () => {
      const mogSource = `fn main() -> int {
  sum: i64 = 0;
  for i := 1 to 10 {
    sum = sum + i;
  }
  return sum;
}`
      // Set interrupt, then clear it — program should run normally
      const hostSource = `
#include "mog.h"
__attribute__((constructor))
static void setup(void) {
    MogVM *vm = mog_vm_new();
    mog_vm_set_global(vm);
    mog_request_interrupt();
    mog_clear_interrupt();
}
`
      const result = await compileAndRunWithHost(mogSource, hostSource)
      // sum(1..10) = 55, exit code = 55 (truncated to i32)
      expect(result.exitCode).toBe(55)
    })
  })
})
