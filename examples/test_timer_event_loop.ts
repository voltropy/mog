#!/usr/bin/env bun
/**
 * TypeScript host smoke test for Mog async integration against the Rust runtime.
 *
 * Verifies:
 *   - a TypeScript event loop can provide timer scheduling for Mog's timer
 *     capability via `setTimeout`
 *   - the Mog code path can run while JS keeps handling other events
 *   - all() waits for multiple timer futures
 */
import { dlopen, FFIType, JSCallback, ptr } from "bun:ffi";
import { existsSync, mkdirSync, readdirSync, unlinkSync } from "fs";
import { dirname, resolve } from "path";
import { fileURLToPath } from "url";
import { tmpdir } from "os";

type Ptr = ReturnType<typeof ptr>;

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const libExt =
  process.platform === "darwin" ? "dylib" : process.platform === "win32" ? "dll" : "so";
const timeoutMs = 5_000;

function cstr(s: string): [Buffer, Ptr] {
  const buf = Buffer.from(`${s}\0`, "utf-8");
  return [buf, ptr(buf)];
}

function assert(condition: unknown, message: string): void {
  if (!condition) {
    throw new Error(message);
  }
}

function findSharedLibrary(
  candidateDirs: string[],
  stem: string,
  ext: string,
): string | undefined {
  for (const base of candidateDirs) {
    const direct = [resolve(base, `lib${stem}.${ext}`), resolve(base, `${stem}.${ext}`)];
    for (const path of direct) {
      if (existsSync(path)) {
        return path;
      }
    }

    if (!existsSync(base)) {
      continue;
    }

    for (const file of readdirSync(base)) {
      if (
        file.startsWith(`lib${stem}-`) &&
        file.endsWith(`.${ext}`) &&
        file.length > stem.length + 10
      ) {
        return resolve(base, file);
      }
    }
  }
  return undefined;
}

function loadSharedLibrary(library: string, symbols: Record<string, unknown>) {
  return dlopen(library, symbols).symbols;
}

function buildLimits(): Buffer {
  const buffer = Buffer.alloc(24);
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);

  // struct MogLimits {
  //   max_memory: usize;      // offset 0
  //   max_cpu_ms: i32;        // offset 8
  //   max_stack_depth: i32;   // offset 12
  //   initial_memory: usize;  // offset 16
  // }
  view.setBigUint64(0, 64n * 1024n * 1024n, true);
  view.setInt32(8, 0, true);
  view.setInt32(12, 1024, true);
  view.setBigUint64(16, 8n * 1024n * 1024n, true);
  return buffer;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

(async () => {
  process.chdir(projectRoot);

  const compilerLib = findSharedLibrary(
    [resolve(projectRoot, "compiler/target/release"), resolve(projectRoot, "compiler/target/debug")],
    "mog",
    libExt,
  );
  assert(compilerLib, `compiler shared library not found for extension .${libExt}`);

  const runtimeLib = findSharedLibrary(
    [
      resolve(projectRoot, "runtime-rs/target/release"),
      resolve(projectRoot, "runtime-rs/target/debug"),
      resolve(projectRoot, "runtime-rs/target/release/deps"),
      resolve(projectRoot, "runtime-rs/target/debug/deps"),
    ],
    "mog_runtime",
    libExt,
  );
  assert(runtimeLib, `runtime shared library not found for extension .${libExt}`);

  const pluginWorkspace = resolve(tmpdir(), "mog-ts-timer-host");
  mkdirSync(pluginWorkspace, { recursive: true });
  const pluginPath = resolve(pluginWorkspace, `mog_timer_host.${libExt}`);
  if (existsSync(pluginPath)) {
    unlinkSync(pluginPath);
  }

  const mogSource = `
requires timer;

pub async fn run_two_timers() -> int {
  first := timer.setTimeout(45);
  second := timer.setTimeout(65);
  values := await all([first, second]);
  return values[0] + values[1];
}
`;

  const compiler = loadSharedLibrary(compilerLib!, {
    mog_compiler_new: { args: [], returns: FFIType.ptr },
    mog_compiler_free: { args: [FFIType.ptr], returns: FFIType.void },
    mog_compile_plugin: {
      args: [FFIType.ptr, FFIType.ptr, FFIType.ptr, FFIType.ptr, FFIType.ptr],
      returns: FFIType.i32,
    },
  });

  const compilerHandle: Ptr = compiler.mog_compiler_new();
  assert(compilerHandle !== 0, "mog_compiler_new returned NULL");

  const [sourceBuf, sourcePtr] = cstr(mogSource);
  const [pluginPathBuf, pluginPathPtr] = cstr(pluginPath);
  const [pluginNameBuf, pluginNamePtr] = cstr("mog_timer_host");
  const [pluginVersionBuf, pluginVersionPtr] = cstr("0.0.1");
  const compileRc: number = compiler.mog_compile_plugin(
    compilerHandle,
    sourcePtr,
    pluginNamePtr,
    pluginVersionPtr,
    pluginPathPtr,
  );
  compiler.mog_compiler_free(compilerHandle);
  assert(compileRc === 0, `mog_compile_plugin failed (${compileRc})`);
  assert(existsSync(pluginPath), "plugin file was not produced");

  const runtime = loadSharedLibrary(runtimeLib!, {
    mog_vm_new: { args: [], returns: FFIType.ptr },
    mog_vm_free: { args: [FFIType.ptr], returns: FFIType.void },
    mog_vm_set_global: { args: [FFIType.ptr], returns: FFIType.void },
    mog_vm_set_limits: { args: [FFIType.ptr, FFIType.ptr], returns: FFIType.void },
    mog_register_timer_host: { args: [FFIType.ptr], returns: FFIType.i32 },
    mog_set_timer_dispatcher: { args: [FFIType.ptr], returns: FFIType.void },
    mog_clear_timer_dispatcher: { args: [], returns: FFIType.void },
    gc_init: { args: [], returns: FFIType.void },
    mog_loop_new: { args: [], returns: FFIType.ptr },
    mog_loop_set_global: { args: [FFIType.ptr], returns: FFIType.void },
    mog_loop_free: { args: [FFIType.ptr], returns: FFIType.void },
    mog_loop_step: { args: [FFIType.ptr], returns: FFIType.i32 },
    mog_future_is_ready: { args: [FFIType.ptr], returns: FFIType.i32 },
    mog_future_get_result: { args: [FFIType.ptr], returns: FFIType.i64 },
    mog_future_complete: { args: [FFIType.ptr, FFIType.i64], returns: FFIType.void },
    mog_future_free: { args: [FFIType.ptr], returns: FFIType.void },
  });

  const plugin = loadSharedLibrary(pluginPath, {
    mog_plugin_init: { args: [FFIType.ptr], returns: FFIType.i32 },
    mogp_run_two_timers: { args: [], returns: FFIType.ptr },
  });

  let pluginFuture: Ptr = 0;
  const callbackStats = { scheduled: 0, completed: 0 };
  const dispatcher = new JSCallback(
    (_vm: Ptr, futurePtr: Ptr, delayMs: bigint) => {
      callbackStats.scheduled += 1;
      const delay = Number(delayMs);
      setTimeout(() => {
        callbackStats.completed += 1;
        runtime.mog_future_complete(futurePtr, delayMs);
      }, Math.max(0, delay));
    },
    {
      args: [FFIType.ptr, FFIType.ptr, FFIType.i64],
      returns: FFIType.void,
      threadsafe: true,
    },
  );

  let loopPtr: Ptr = 0;
  let vmPtr: Ptr = 0;
  let heartbeat = 0;
  const heartbeatHandle = setInterval(() => {
    heartbeat += 1;
  }, 5);

  try {
    runtime.gc_init();
    vmPtr = runtime.mog_vm_new();
    assert(vmPtr !== 0, "mog_vm_new returned NULL");

    const limits = buildLimits();
    runtime.mog_vm_set_limits(vmPtr, limits);
    runtime.mog_vm_set_global(vmPtr);

    const timerRegisterRc = runtime.mog_register_timer_host(vmPtr);
    assert(timerRegisterRc === 0, `mog_register_timer_host failed (${timerRegisterRc})`);

    runtime.mog_set_timer_dispatcher(dispatcher.ptr);

    loopPtr = runtime.mog_loop_new();
    assert(loopPtr !== 0, "mog_loop_new returned NULL");
    runtime.mog_loop_set_global(loopPtr);

    const initRc: number = plugin.mog_plugin_init(vmPtr);
    assert(initRc === 0, `mog_plugin_init failed (${initRc})`);

    pluginFuture = plugin.mogp_run_two_timers();
    assert(pluginFuture !== 0, "run_two_timers returned NULL future");

    const start = Date.now();
    while (runtime.mog_future_is_ready(pluginFuture) === 0) {
      runtime.mog_loop_step(loopPtr);
      if (Date.now() - start > timeoutMs) {
        throw new Error("timeout waiting for Mog future");
      }
      await sleep(1);
    }

    const result = Number(runtime.mog_future_get_result(pluginFuture));
    assert(result === 110, `expected result 110, got ${result}`);
    assert(callbackStats.scheduled === 2, "timer dispatcher should schedule exactly two timers");
    assert(callbackStats.completed === 2, "timer dispatcher should complete exactly two timers");
    assert(heartbeat > 0, "the TypeScript event loop did not get CPU time while waiting");
    console.log("TypeScript host timer test passed");
  } finally {
    clearInterval(heartbeatHandle);
    runtime?.mog_clear_timer_dispatcher();
    runtime?.mog_loop_free(loopPtr);
    runtime?.mog_vm_free(vmPtr);
    if (pluginFuture !== 0) {
      runtime?.mog_future_free(pluginFuture);
    }
    dispatcher?.close();
    if (existsSync(pluginPath)) {
      unlinkSync(pluginPath);
    }
  }
})();
