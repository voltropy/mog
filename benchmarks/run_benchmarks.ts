#!/usr/bin/env bun
/**
 * Mog Compiler Benchmark Suite
 *
 * Measures compilation speed across Mog, Go, and Rust for equivalent programs
 * at three sizes (tiny, medium, large) and multiple optimization levels.
 *
 * Usage:
 *   bun benchmarks/run_benchmarks.ts [--iterations N] [--warmup N]
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync } from "fs";
import { execSync } from "child_process";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const ITERATIONS = parseInt(process.argv.find((_, i, a) => a[i - 1] === "--iterations") ?? "5");
const WARMUP = parseInt(process.argv.find((_, i, a) => a[i - 1] === "--warmup") ?? "2");
const BUILD_DIR = "benchmarks/build";
const SIZES = ["tiny", "medium", "large"] as const;

if (!existsSync(BUILD_DIR)) mkdirSync(BUILD_DIR, { recursive: true });

// ---------------------------------------------------------------------------
// Timing helpers
// ---------------------------------------------------------------------------

interface TimingResult {
  label: string;
  lines: number;
  mean_ms: number;
  median_ms: number;
  min_ms: number;
  max_ms: number;
  stddev_ms: number;
  samples: number[];
}

function stats(label: string, samples: number[], lines: number = 0): TimingResult {
  const sorted = [...samples].sort((a, b) => a - b);
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  const median = sorted[Math.floor(sorted.length / 2)];
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  const variance = samples.reduce((s, x) => s + (x - mean) ** 2, 0) / samples.length;
  const stddev = Math.sqrt(variance);
  return { label, lines, mean_ms: mean, median_ms: median, min_ms: min, max_ms: max, stddev_ms: stddev, samples };
}

function timeShell(cmd: string, cwd?: string): number {
  const start = performance.now();
  execSync(cmd, { cwd: cwd ?? ".", stdio: "pipe" });
  return performance.now() - start;
}

function benchmark(label: string, fn: () => void): TimingResult {
  // Warmup
  for (let i = 0; i < WARMUP; i++) fn();
  // Measure
  const samples: number[] = [];
  for (let i = 0; i < ITERATIONS; i++) {
    const start = performance.now();
    fn();
    samples.push(performance.now() - start);
  }
  return stats(label, samples);
}

function benchmarkShell(label: string, cmd: string, lines: number = 0, cwd?: string): TimingResult {
  // Warmup
  for (let i = 0; i < WARMUP; i++) timeShell(cmd, cwd);
  // Measure
  const samples: number[] = [];
  for (let i = 0; i < ITERATIONS; i++) {
    samples.push(timeShell(cmd, cwd));
  }
  return stats(label, samples, lines);
}

// ---------------------------------------------------------------------------
// Mog compiler — import phases directly for fine-grained timing
// ---------------------------------------------------------------------------

import { tokenize } from "../src/lexer";
import { parseTokens } from "../src/parser";
import { SemanticAnalyzer } from "../src/analyzer";
import { generateLLVMIR } from "../src/llvm_codegen";
import { generateQBEIR } from "../src/qbe_codegen";

const QBE_BIN = "/opt/homebrew/bin/qbe";

interface MogPhaseTimings {
  lex_ms: number;
  parse_ms: number;
  analyze_ms: number;
  codegen_llvm_ms: number;
  codegen_qbe_ms: number;
  total_frontend_ms: number;
}

function mogFrontend(source: string): { timings: MogPhaseTimings; llvmIR: string; qbeIR: string } {
  const t0 = performance.now();
  const tokens = tokenize(source);
  const filteredTokens = tokens.filter((t: any) => t.type !== "WHITESPACE" && t.type !== "COMMENT");
  const t1 = performance.now();

  const ast = parseTokens(filteredTokens);
  const t2 = performance.now();

  const analyzer = new SemanticAnalyzer();
  const errors = analyzer.analyze(ast);
  const t3 = performance.now();

  if (errors.length > 0) {
    throw new Error(`Analysis errors: ${errors.map((e: any) => e.message).join(", ")}`);
  }

  const llvmIR = generateLLVMIR(ast);
  const t4 = performance.now();

  // Re-parse for QBE (generateLLVMIR may mutate AST state)
  const ast2 = parseTokens(filteredTokens);
  const analyzer2 = new SemanticAnalyzer();
  analyzer2.analyze(ast2);
  const qbeIR = generateQBEIR(ast2);
  const t5 = performance.now();

  return {
    timings: {
      lex_ms: t1 - t0,
      parse_ms: t2 - t1,
      analyze_ms: t3 - t2,
      codegen_llvm_ms: t4 - t3,
      codegen_qbe_ms: t5 - t4,
      total_frontend_ms: t4 - t0,
    },
    llvmIR,
    qbeIR,
  };
}

// ---------------------------------------------------------------------------
// Benchmark: Mog phases
// ---------------------------------------------------------------------------

function benchmarkMogPhases(size: string, source: string, lines: number) {
  const results: TimingResult[] = [];

  // Warmup
  for (let i = 0; i < WARMUP; i++) mogFrontend(source);

  // Collect per-phase timings
  const lexSamples: number[] = [];
  const parseSamples: number[] = [];
  const analyzeSamples: number[] = [];
  const codegenLlvmSamples: number[] = [];
  const codegenQbeSamples: number[] = [];
  const totalSamples: number[] = [];

  for (let i = 0; i < ITERATIONS; i++) {
    const { timings } = mogFrontend(source);
    lexSamples.push(timings.lex_ms);
    parseSamples.push(timings.parse_ms);
    analyzeSamples.push(timings.analyze_ms);
    codegenLlvmSamples.push(timings.codegen_llvm_ms);
    codegenQbeSamples.push(timings.codegen_qbe_ms);
    totalSamples.push(timings.total_frontend_ms);
  }

  results.push(stats(`mog/${size}/lex`, lexSamples, lines));
  results.push(stats(`mog/${size}/parse`, parseSamples, lines));
  results.push(stats(`mog/${size}/analyze`, analyzeSamples, lines));
  results.push(stats(`mog/${size}/codegen_llvm_ir`, codegenLlvmSamples, lines));
  results.push(stats(`mog/${size}/codegen_qbe_ir`, codegenQbeSamples, lines));
  results.push(stats(`mog/${size}/frontend_total`, totalSamples, lines));

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Mog LLVM backend (clang on IR at different opt levels)
// ---------------------------------------------------------------------------

function benchmarkMogBackend(size: string, llvmIR: string, lines: number) {
  const irPath = `${BUILD_DIR}/bench_${size}.ll`;
  writeFileSync(irPath, llvmIR);
  const results: TimingResult[] = [];

  for (const opt of ["-O0", "-O1", "-O2"]) {
    const objPath = `${BUILD_DIR}/bench_${size}_${opt.replace("-", "")}.o`;
    const cmd = `clang ${opt} -c -x ir ${irPath} -o ${objPath}`;
    results.push(benchmarkShell(`mog/${size}/clang_${opt}`, cmd, lines));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Mog QBE backend (qbe → asm, then cc at different opt levels)
// ---------------------------------------------------------------------------

function benchmarkMogQBEBackend(size: string, qbeIR: string, lines: number) {
  const ssaPath = `${BUILD_DIR}/bench_${size}.ssa`;
  const asmPath = `${BUILD_DIR}/bench_${size}_qbe.s`;
  writeFileSync(ssaPath, qbeIR);
  const results: TimingResult[] = [];

  // QBE itself (always one pass — QBE has no optimization levels)
  results.push(benchmarkShell(
    `mog/${size}/qbe`,
    `${QBE_BIN} -t arm64_apple -o ${asmPath} ${ssaPath}`,
    lines
  ));

  // cc assembling the QBE output at different opt levels
  for (const opt of ["-O0", "-O1", "-O2"]) {
    const objPath = `${BUILD_DIR}/bench_${size}_qbe_${opt.replace("-", "")}.o`;
    const cmd = `cc ${opt} -c ${asmPath} -o ${objPath}`;
    results.push(benchmarkShell(`mog/${size}/qbe+cc_${opt}`, cmd, lines));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Mog end-to-end with QBE (frontend + qbe + cc)
// ---------------------------------------------------------------------------

function benchmarkMogQBEE2E(size: string, source: string, lines: number) {
  const results: TimingResult[] = [];
  const ssaPath = `${BUILD_DIR}/bench_${size}_e2e.ssa`;
  const asmPath = `${BUILD_DIR}/bench_${size}_e2e_qbe.s`;

  for (const opt of ["-O0", "-O1", "-O2"]) {
    const objPath = `${BUILD_DIR}/bench_${size}_qbe_e2e.o`;

    // Warmup
    for (let i = 0; i < WARMUP; i++) {
      const tokens = tokenize(source);
      const filtered = tokens.filter((t: any) => t.type !== "WHITESPACE" && t.type !== "COMMENT");
      const ast = parseTokens(filtered);
      const analyzer = new SemanticAnalyzer();
      analyzer.analyze(ast);
      const qbeIR = generateQBEIR(ast);
      writeFileSync(ssaPath, qbeIR);
      execSync(`${QBE_BIN} -t arm64_apple -o ${asmPath} ${ssaPath}`, { stdio: "pipe" });
      execSync(`cc ${opt} -c ${asmPath} -o ${objPath}`, { stdio: "pipe" });
    }

    const samples: number[] = [];
    for (let i = 0; i < ITERATIONS; i++) {
      const start = performance.now();
      const tokens = tokenize(source);
      const filtered = tokens.filter((t: any) => t.type !== "WHITESPACE" && t.type !== "COMMENT");
      const ast = parseTokens(filtered);
      const analyzer = new SemanticAnalyzer();
      analyzer.analyze(ast);
      const qbeIR = generateQBEIR(ast);
      writeFileSync(ssaPath, qbeIR);
      execSync(`${QBE_BIN} -t arm64_apple -o ${asmPath} ${ssaPath}`, { stdio: "pipe" });
      execSync(`cc ${opt} -c ${asmPath} -o ${objPath}`, { stdio: "pipe" });
      samples.push(performance.now() - start);
    }
    results.push(stats(`mog/${size}/qbe_e2e_${opt}`, samples, lines));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Mog end-to-end with LLVM (frontend + clang backend)
// ---------------------------------------------------------------------------

function benchmarkMogE2E(size: string, source: string, lines: number) {
  const results: TimingResult[] = [];

  for (const opt of ["-O0", "-O1", "-O2"]) {
    const irPath = `${BUILD_DIR}/bench_${size}.ll`;
    const objPath = `${BUILD_DIR}/bench_${size}_e2e.o`;

    // Warmup
    for (let i = 0; i < WARMUP; i++) {
      const { llvmIR } = mogFrontend(source);
      writeFileSync(irPath, llvmIR);
      execSync(`clang ${opt} -c -x ir ${irPath} -o ${objPath}`, { stdio: "pipe" });
    }

    const samples: number[] = [];
    for (let i = 0; i < ITERATIONS; i++) {
      const start = performance.now();
      const { llvmIR } = mogFrontend(source);
      writeFileSync(irPath, llvmIR);
      execSync(`clang ${opt} -c -x ir ${irPath} -o ${objPath}`, { stdio: "pipe" });
      samples.push(performance.now() - start);
    }
    results.push(stats(`mog/${size}/e2e_${opt}`, samples, lines));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Go
// ---------------------------------------------------------------------------

function benchmarkGo(size: string, lines: number) {
  const src = `benchmarks/go/${size}.go`;
  const out = `${BUILD_DIR}/go_${size}`;
  const results: TimingResult[] = [];

  // Go has limited optimization flags; gcflags -N -l disables optimization
  results.push(benchmarkShell(`go/${size}/build_default`, `go build -o ${out} ${src}`, lines));
  results.push(benchmarkShell(`go/${size}/build_noopt`, `go build -gcflags='-N -l' -o ${out} ${src}`, lines));

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Rust
// ---------------------------------------------------------------------------

function benchmarkRust(size: string, lines: number) {
  const src = `benchmarks/rust/${size}.rs`;
  const out = `${BUILD_DIR}/rust_${size}`;
  const results: TimingResult[] = [];

  // Debug (no optimization)
  results.push(benchmarkShell(`rust/${size}/debug`, `rustc --edition 2021 ${src} -o ${out}`, lines));
  // Release with different opt levels
  for (const opt of ["1", "2", "3"]) {
    results.push(benchmarkShell(`rust/${size}/opt_O${opt}`, `rustc --edition 2021 -C opt-level=${opt} ${src} -o ${out}`, lines));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Pretty printing
// ---------------------------------------------------------------------------

function fmtMs(ms: number): string {
  if (ms < 1) return `${(ms * 1000).toFixed(0)}µs`;
  if (ms < 1000) return `${ms.toFixed(1)}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

function fmtUsPerLine(ms: number, lines: number): string {
  if (lines <= 0) return "—";
  const usPerLine = (ms * 1000) / lines;
  if (usPerLine < 1) return `${(usPerLine * 1000).toFixed(0)}ns/l`;
  if (usPerLine < 1000) return `${usPerLine.toFixed(1)}µs/l`;
  return `${(usPerLine / 1000).toFixed(1)}ms/l`;
}

function printTable(title: string, results: TimingResult[]) {
  console.log(`\n${"=".repeat(90)}`);
  console.log(`  ${title}`);
  console.log(`${"=".repeat(90)}`);
  console.log(
    `  ${"Benchmark".padEnd(35)} ${"Median".padStart(10)} ${"Mean".padStart(10)} ${"Min".padStart(10)} ${"Max".padStart(10)} ${"StdDev".padStart(10)} ${"µs/line".padStart(10)}`
  );
  console.log(`  ${"-".repeat(85)}`);
  for (const r of results) {
    console.log(
      `  ${r.label.padEnd(35)} ${fmtMs(r.median_ms).padStart(10)} ${fmtMs(r.mean_ms).padStart(10)} ${fmtMs(r.min_ms).padStart(10)} ${fmtMs(r.max_ms).padStart(10)} ${fmtMs(r.stddev_ms).padStart(10)} ${fmtUsPerLine(r.median_ms, r.lines).padStart(10)}`
    );
  }
}

function printComparison(mogResults: TimingResult[], goResults: TimingResult[], rustResults: TimingResult[]) {
  const W = 130;
  console.log(`\n${"=".repeat(W)}`);
  console.log(`  CROSS-LANGUAGE COMPARISON (median, end-to-end compile to object/binary)`);
  console.log(`${"=".repeat(W)}`);

  // --- Absolute times ---
  const hdr = [
    "Size", "LLVM -O0", "LLVM -O1", "LLVM -O2",
    "QBE -O0", "QBE -O1", "QBE -O2",
    "Go default", "Go noopt", "Rust debug", "Rust -O2",
  ];
  console.log(`  ${hdr[0].padEnd(10)} ${hdr.slice(1).map(h => h.padStart(12)).join(" ")}`);
  console.log(`  ${"-".repeat(W - 4)}`);

  for (const size of SIZES) {
    const get = (label: string) => mogResults.find(r => r.label === label) ?? goResults.find(r => r.label === label) ?? rustResults.find(r => r.label === label);
    const vals = [
      get(`mog/${size}/e2e_-O0`), get(`mog/${size}/e2e_-O1`), get(`mog/${size}/e2e_-O2`),
      get(`mog/${size}/qbe_e2e_-O0`), get(`mog/${size}/qbe_e2e_-O1`), get(`mog/${size}/qbe_e2e_-O2`),
      get(`go/${size}/build_default`), get(`go/${size}/build_noopt`),
      get(`rust/${size}/debug`), get(`rust/${size}/opt_O2`),
    ];
    console.log(`  ${size.padEnd(10)} ${vals.map(v => fmtMs(v?.median_ms ?? 0).padStart(12)).join(" ")}`);
  }

  // --- µs/line ---
  console.log(`\n  Compilation speed (µs per source line, median):`);
  console.log(`  ${"-".repeat(W - 4)}`);
  console.log(`  ${hdr[0].padEnd(10)} ${hdr.slice(1).map(h => h.padStart(12)).join(" ")}`);
  console.log(`  ${"-".repeat(W - 4)}`);

  for (const size of SIZES) {
    const get = (label: string) => mogResults.find(r => r.label === label) ?? goResults.find(r => r.label === label) ?? rustResults.find(r => r.label === label);
    const vals = [
      get(`mog/${size}/e2e_-O0`), get(`mog/${size}/e2e_-O1`), get(`mog/${size}/e2e_-O2`),
      get(`mog/${size}/qbe_e2e_-O0`), get(`mog/${size}/qbe_e2e_-O1`), get(`mog/${size}/qbe_e2e_-O2`),
      get(`go/${size}/build_default`), get(`go/${size}/build_noopt`),
      get(`rust/${size}/debug`), get(`rust/${size}/opt_O2`),
    ];
    const fmt = (r: TimingResult | undefined) => r ? fmtUsPerLine(r.median_ms, r.lines) : "—";
    console.log(`  ${size.padEnd(10)} ${vals.map(v => fmt(v).padStart(12)).join(" ")}`);
  }

  // --- LLVM vs QBE head-to-head ---
  console.log(`\n  LLVM vs QBE Backend (end-to-end, median):`);
  console.log(`  ${"-".repeat(80)}`);
  console.log(`  ${"Size".padEnd(10)} ${"LLVM -O0".padStart(12)} ${"QBE -O0".padStart(12)} ${"Speedup".padStart(10)} ${"LLVM -O1".padStart(12)} ${"QBE -O1".padStart(12)} ${"Speedup".padStart(10)}`);
  console.log(`  ${"-".repeat(80)}`);
  for (const size of SIZES) {
    const llvmO0 = mogResults.find(r => r.label === `mog/${size}/e2e_-O0`)?.median_ms ?? 1;
    const qbeO0 = mogResults.find(r => r.label === `mog/${size}/qbe_e2e_-O0`)?.median_ms ?? 1;
    const llvmO1 = mogResults.find(r => r.label === `mog/${size}/e2e_-O1`)?.median_ms ?? 1;
    const qbeO1 = mogResults.find(r => r.label === `mog/${size}/qbe_e2e_-O1`)?.median_ms ?? 1;
    console.log(
      `  ${size.padEnd(10)} ${fmtMs(llvmO0).padStart(12)} ${fmtMs(qbeO0).padStart(12)} ${(llvmO0 / qbeO0).toFixed(2).padStart(8)}x ${fmtMs(llvmO1).padStart(12)} ${fmtMs(qbeO1).padStart(12)} ${(llvmO1 / qbeO1).toFixed(2).padStart(8)}x`
    );
  }

  // --- Ratios vs Mog QBE -O0 (fastest Mog backend) ---
  console.log(`\n  Ratios (vs Mog QBE -O0 end-to-end, lower = faster):`);
  console.log(`  ${"-".repeat(90)}`);
  for (const size of SIZES) {
    const qbeO0 = mogResults.find(r => r.label === `mog/${size}/qbe_e2e_-O0`)?.median_ms ?? 1;
    const llvmO1 = mogResults.find(r => r.label === `mog/${size}/e2e_-O1`)?.median_ms ?? 1;
    const goDef = goResults.find(r => r.label === `go/${size}/build_default`)?.median_ms ?? 1;
    const rustDbg = rustResults.find(r => r.label === `rust/${size}/debug`)?.median_ms ?? 1;
    const rustO2 = rustResults.find(r => r.label === `rust/${size}/opt_O2`)?.median_ms ?? 1;

    console.log(
      `  ${size.padEnd(10)} QBE -O0: 1.00x   LLVM -O1: ${(llvmO1 / qbeO0).toFixed(2)}x   Go: ${(goDef / qbeO0).toFixed(2)}x   Rust debug: ${(rustDbg / qbeO0).toFixed(2)}x   Rust -O2: ${(rustO2 / qbeO0).toFixed(2)}x`
    );
  }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  console.log("Mog Compiler Benchmark Suite");
  console.log(`Iterations: ${ITERATIONS}, Warmup: ${WARMUP}`);
  console.log(`Bun: ${Bun.version}, Go: ${execSync("go version").toString().trim()}`);
  console.log(`Rust: ${execSync("rustc --version").toString().trim()}`);
  console.log(`Clang: ${execSync("clang --version").toString().trim().split("\n")[0]}`);
  console.log(`QBE: ${execSync(`${QBE_BIN} -h 2>&1 || true`).toString().trim().split("\n")[0]}`);

  // Line counts
  const lineCounts: Record<string, { mog: number; go: number; rust: number }> = {};
  console.log("\nProgram sizes:");
  for (const size of SIZES) {
    const mogLines = readFileSync(`benchmarks/mog/${size}.mog`, "utf-8").split("\n").length;
    const goLines = readFileSync(`benchmarks/go/${size}.go`, "utf-8").split("\n").length;
    const rustLines = readFileSync(`benchmarks/rust/${size}.rs`, "utf-8").split("\n").length;
    lineCounts[size] = { mog: mogLines, go: goLines, rust: rustLines };
    console.log(`  ${size}: Mog ${mogLines} lines, Go ${goLines} lines, Rust ${rustLines} lines`);
  }

  const allMogResults: TimingResult[] = [];
  const allGoResults: TimingResult[] = [];
  const allRustResults: TimingResult[] = [];

  // --- Mog benchmarks ---
  for (const size of SIZES) {
    const source = readFileSync(`benchmarks/mog/${size}.mog`, "utf-8");
    const lines = lineCounts[size].mog;

    console.log(`\nBenchmarking Mog ${size}...`);

    // Phase breakdown
    const phaseResults = benchmarkMogPhases(size, source, lines);
    allMogResults.push(...phaseResults);
    printTable(`Mog Frontend Phases — ${size} (${lines} lines)`, phaseResults);

    // Generate IR for backend benchmarks
    const { llvmIR, qbeIR } = mogFrontend(source);

    // LLVM Backend (clang on IR)
    const backendResults = benchmarkMogBackend(size, llvmIR, lines);
    allMogResults.push(...backendResults);
    printTable(`Mog LLVM Backend (clang on IR) — ${size} (${lines} lines)`, backendResults);

    // QBE Backend (qbe + cc)
    const qbeBackendResults = benchmarkMogQBEBackend(size, qbeIR, lines);
    allMogResults.push(...qbeBackendResults);
    printTable(`Mog QBE Backend (qbe + cc) — ${size} (${lines} lines)`, qbeBackendResults);

    // LLVM End-to-end
    const e2eResults = benchmarkMogE2E(size, source, lines);
    allMogResults.push(...e2eResults);
    printTable(`Mog LLVM End-to-End — ${size} (${lines} lines)`, e2eResults);

    // QBE End-to-end
    const qbeE2eResults = benchmarkMogQBEE2E(size, source, lines);
    allMogResults.push(...qbeE2eResults);
    printTable(`Mog QBE End-to-End — ${size} (${lines} lines)`, qbeE2eResults);
  }

  // --- Go benchmarks ---
  for (const size of SIZES) {
    const lines = lineCounts[size].go;
    console.log(`\nBenchmarking Go ${size}...`);
    const goResults = benchmarkGo(size, lines);
    allGoResults.push(...goResults);
    printTable(`Go — ${size} (${lines} lines)`, goResults);
  }

  // --- Rust benchmarks ---
  for (const size of SIZES) {
    const lines = lineCounts[size].rust;
    console.log(`\nBenchmarking Rust ${size}...`);
    const rustResults = benchmarkRust(size, lines);
    allRustResults.push(...rustResults);
    printTable(`Rust — ${size} (${lines} lines)`, rustResults);
  }

  // --- Cross-language comparison ---
  printComparison(allMogResults, allGoResults, allRustResults);

  // --- Mog frontend-only comparison ---
  console.log(`\n${"=".repeat(90)}`);
  console.log(`  MOG FRONTEND-ONLY (no backend) — what embedding would need`);
  console.log(`${"=".repeat(90)}`);
  for (const size of SIZES) {
    const fe = allMogResults.find(r => r.label === `mog/${size}/frontend_total`);
    if (fe) {
      console.log(`  ${size.padEnd(10)} ${fmtMs(fe.median_ms).padStart(10)} median  ${fmtUsPerLine(fe.median_ms, fe.lines).padStart(10)}  (lex+parse+analyze+codegen_llvm_ir)`);
    }
  }

  // --- LLVM vs QBE codegen comparison ---
  console.log(`\n${"=".repeat(90)}`);
  console.log(`  CODEGEN IR GENERATION: LLVM vs QBE (median)`);
  console.log(`${"=".repeat(90)}`);
  console.log(`  ${"Size".padEnd(10)} ${"LLVM IR".padStart(12)} ${"QBE IR".padStart(12)} ${"Speedup".padStart(10)}`);
  console.log(`  ${"-".repeat(50)}`);
  for (const size of SIZES) {
    const llvm = allMogResults.find(r => r.label === `mog/${size}/codegen_llvm_ir`);
    const qbe = allMogResults.find(r => r.label === `mog/${size}/codegen_qbe_ir`);
    if (llvm && qbe) {
      const speedup = llvm.median_ms / qbe.median_ms;
      console.log(`  ${size.padEnd(10)} ${fmtMs(llvm.median_ms).padStart(12)} ${fmtMs(qbe.median_ms).padStart(12)} ${speedup.toFixed(2).padStart(8)}x`);
    }
  }

  // Write raw JSON
  const allResults = { mog: allMogResults, go: allGoResults, rust: allRustResults };
  writeFileSync(`${BUILD_DIR}/benchmark_results.json`, JSON.stringify(allResults, null, 2));
  console.log(`\nRaw results written to ${BUILD_DIR}/benchmark_results.json`);
}

main().catch(console.error);
