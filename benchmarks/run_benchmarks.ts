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
  mean_ms: number;
  median_ms: number;
  min_ms: number;
  max_ms: number;
  stddev_ms: number;
  samples: number[];
}

function stats(label: string, samples: number[]): TimingResult {
  const sorted = [...samples].sort((a, b) => a - b);
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  const median = sorted[Math.floor(sorted.length / 2)];
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  const variance = samples.reduce((s, x) => s + (x - mean) ** 2, 0) / samples.length;
  const stddev = Math.sqrt(variance);
  return { label, mean_ms: mean, median_ms: median, min_ms: min, max_ms: max, stddev_ms: stddev, samples };
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

function benchmarkShell(label: string, cmd: string, cwd?: string): TimingResult {
  // Warmup
  for (let i = 0; i < WARMUP; i++) timeShell(cmd, cwd);
  // Measure
  const samples: number[] = [];
  for (let i = 0; i < ITERATIONS; i++) {
    samples.push(timeShell(cmd, cwd));
  }
  return stats(label, samples);
}

// ---------------------------------------------------------------------------
// Mog compiler — import phases directly for fine-grained timing
// ---------------------------------------------------------------------------

import { tokenize } from "../src/lexer";
import { parseTokens } from "../src/parser";
import { SemanticAnalyzer } from "../src/analyzer";
import { generateLLVMIR } from "../src/llvm_codegen";

interface MogPhaseTimings {
  lex_ms: number;
  parse_ms: number;
  analyze_ms: number;
  codegen_ms: number;
  total_frontend_ms: number;
}

function mogFrontend(source: string): { timings: MogPhaseTimings; llvmIR: string } {
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

  return {
    timings: {
      lex_ms: t1 - t0,
      parse_ms: t2 - t1,
      analyze_ms: t3 - t2,
      codegen_ms: t4 - t3,
      total_frontend_ms: t4 - t0,
    },
    llvmIR,
  };
}

// ---------------------------------------------------------------------------
// Benchmark: Mog phases
// ---------------------------------------------------------------------------

function benchmarkMogPhases(size: string, source: string) {
  const results: TimingResult[] = [];

  // Warmup
  for (let i = 0; i < WARMUP; i++) mogFrontend(source);

  // Collect per-phase timings
  const lexSamples: number[] = [];
  const parseSamples: number[] = [];
  const analyzeSamples: number[] = [];
  const codegenSamples: number[] = [];
  const totalSamples: number[] = [];

  for (let i = 0; i < ITERATIONS; i++) {
    const { timings } = mogFrontend(source);
    lexSamples.push(timings.lex_ms);
    parseSamples.push(timings.parse_ms);
    analyzeSamples.push(timings.analyze_ms);
    codegenSamples.push(timings.codegen_ms);
    totalSamples.push(timings.total_frontend_ms);
  }

  results.push(stats(`mog/${size}/lex`, lexSamples));
  results.push(stats(`mog/${size}/parse`, parseSamples));
  results.push(stats(`mog/${size}/analyze`, analyzeSamples));
  results.push(stats(`mog/${size}/codegen_ir`, codegenSamples));
  results.push(stats(`mog/${size}/frontend_total`, totalSamples));

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Mog LLVM backend (clang on IR at different opt levels)
// ---------------------------------------------------------------------------

function benchmarkMogBackend(size: string, llvmIR: string) {
  const irPath = `${BUILD_DIR}/bench_${size}.ll`;
  writeFileSync(irPath, llvmIR);
  const results: TimingResult[] = [];

  for (const opt of ["-O0", "-O1", "-O2"]) {
    const objPath = `${BUILD_DIR}/bench_${size}_${opt.replace("-", "")}.o`;
    const cmd = `clang ${opt} -c -x ir ${irPath} -o ${objPath}`;
    results.push(benchmarkShell(`mog/${size}/clang_${opt}`, cmd));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Mog end-to-end (frontend + clang backend)
// ---------------------------------------------------------------------------

function benchmarkMogE2E(size: string, source: string) {
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
    results.push(stats(`mog/${size}/e2e_${opt}`, samples));
  }

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Go
// ---------------------------------------------------------------------------

function benchmarkGo(size: string) {
  const src = `benchmarks/go/${size}.go`;
  const out = `${BUILD_DIR}/go_${size}`;
  const results: TimingResult[] = [];

  // Go has limited optimization flags; gcflags -N -l disables optimization
  results.push(benchmarkShell(`go/${size}/build_default`, `go build -o ${out} ${src}`));
  results.push(benchmarkShell(`go/${size}/build_noopt`, `go build -gcflags='-N -l' -o ${out} ${src}`));

  return results;
}

// ---------------------------------------------------------------------------
// Benchmark: Rust
// ---------------------------------------------------------------------------

function benchmarkRust(size: string) {
  const src = `benchmarks/rust/${size}.rs`;
  const out = `${BUILD_DIR}/rust_${size}`;
  const results: TimingResult[] = [];

  // Debug (no optimization)
  results.push(benchmarkShell(`rust/${size}/debug`, `rustc --edition 2021 ${src} -o ${out}`));
  // Release with different opt levels
  for (const opt of ["1", "2", "3"]) {
    results.push(benchmarkShell(`rust/${size}/opt_O${opt}`, `rustc --edition 2021 -C opt-level=${opt} ${src} -o ${out}`));
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

function printTable(title: string, results: TimingResult[]) {
  console.log(`\n${"=".repeat(80)}`);
  console.log(`  ${title}`);
  console.log(`${"=".repeat(80)}`);
  console.log(
    `  ${"Benchmark".padEnd(35)} ${"Median".padStart(10)} ${"Mean".padStart(10)} ${"Min".padStart(10)} ${"Max".padStart(10)} ${"StdDev".padStart(10)}`
  );
  console.log(`  ${"-".repeat(75)}`);
  for (const r of results) {
    console.log(
      `  ${r.label.padEnd(35)} ${fmtMs(r.median_ms).padStart(10)} ${fmtMs(r.mean_ms).padStart(10)} ${fmtMs(r.min_ms).padStart(10)} ${fmtMs(r.max_ms).padStart(10)} ${fmtMs(r.stddev_ms).padStart(10)}`
    );
  }
}

function printComparison(mogResults: TimingResult[], goResults: TimingResult[], rustResults: TimingResult[]) {
  console.log(`\n${"=".repeat(80)}`);
  console.log(`  CROSS-LANGUAGE COMPARISON (median, end-to-end compile to object/binary)`);
  console.log(`${"=".repeat(80)}`);
  console.log(
    `  ${"Size".padEnd(10)} ${"Mog -O0".padStart(12)} ${"Mog -O1".padStart(12)} ${"Mog -O2".padStart(12)} ${"Go default".padStart(12)} ${"Go noopt".padStart(12)} ${"Rust debug".padStart(12)} ${"Rust -O2".padStart(12)}`
  );
  console.log(`  ${"-".repeat(78)}`);

  for (const size of SIZES) {
    const mogO0 = mogResults.find(r => r.label === `mog/${size}/e2e_-O0`);
    const mogO1 = mogResults.find(r => r.label === `mog/${size}/e2e_-O1`);
    const mogO2 = mogResults.find(r => r.label === `mog/${size}/e2e_-O2`);
    const goDef = goResults.find(r => r.label === `go/${size}/build_default`);
    const goNo = goResults.find(r => r.label === `go/${size}/build_noopt`);
    const rustDbg = rustResults.find(r => r.label === `rust/${size}/debug`);
    const rustO2 = rustResults.find(r => r.label === `rust/${size}/opt_O2`);

    console.log(
      `  ${size.padEnd(10)} ${fmtMs(mogO0?.median_ms ?? 0).padStart(12)} ${fmtMs(mogO1?.median_ms ?? 0).padStart(12)} ${fmtMs(mogO2?.median_ms ?? 0).padStart(12)} ${fmtMs(goDef?.median_ms ?? 0).padStart(12)} ${fmtMs(goNo?.median_ms ?? 0).padStart(12)} ${fmtMs(rustDbg?.median_ms ?? 0).padStart(12)} ${fmtMs(rustO2?.median_ms ?? 0).padStart(12)}`
    );
  }

  // Ratios
  console.log(`\n  Ratios (vs Mog -O1 end-to-end, lower = faster):`);
  console.log(`  ${"-".repeat(60)}`);
  for (const size of SIZES) {
    const mogO1 = mogResults.find(r => r.label === `mog/${size}/e2e_-O1`)?.median_ms ?? 1;
    const goDef = goResults.find(r => r.label === `go/${size}/build_default`)?.median_ms ?? 1;
    const rustDbg = rustResults.find(r => r.label === `rust/${size}/debug`)?.median_ms ?? 1;
    const rustO2 = rustResults.find(r => r.label === `rust/${size}/opt_O2`)?.median_ms ?? 1;

    console.log(
      `  ${size.padEnd(10)} Mog -O1: 1.00x   Go: ${(goDef / mogO1).toFixed(2)}x   Rust debug: ${(rustDbg / mogO1).toFixed(2)}x   Rust -O2: ${(rustO2 / mogO1).toFixed(2)}x`
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

  // Line counts
  console.log("\nProgram sizes:");
  for (const size of SIZES) {
    const mogLines = readFileSync(`benchmarks/mog/${size}.mog`, "utf-8").split("\n").length;
    const goLines = readFileSync(`benchmarks/go/${size}.go`, "utf-8").split("\n").length;
    const rustLines = readFileSync(`benchmarks/rust/${size}.rs`, "utf-8").split("\n").length;
    console.log(`  ${size}: Mog ${mogLines} lines, Go ${goLines} lines, Rust ${rustLines} lines`);
  }

  const allMogResults: TimingResult[] = [];
  const allGoResults: TimingResult[] = [];
  const allRustResults: TimingResult[] = [];

  // --- Mog benchmarks ---
  for (const size of SIZES) {
    const source = readFileSync(`benchmarks/mog/${size}.mog`, "utf-8");

    console.log(`\nBenchmarking Mog ${size}...`);

    // Phase breakdown
    const phaseResults = benchmarkMogPhases(size, source);
    allMogResults.push(...phaseResults);
    printTable(`Mog Frontend Phases — ${size}`, phaseResults);

    // Backend (clang on IR)
    const { llvmIR } = mogFrontend(source);
    const backendResults = benchmarkMogBackend(size, llvmIR);
    allMogResults.push(...backendResults);
    printTable(`Mog Backend (clang on IR) — ${size}`, backendResults);

    // End-to-end
    const e2eResults = benchmarkMogE2E(size, source);
    allMogResults.push(...e2eResults);
    printTable(`Mog End-to-End — ${size}`, e2eResults);
  }

  // --- Go benchmarks ---
  for (const size of SIZES) {
    console.log(`\nBenchmarking Go ${size}...`);
    const goResults = benchmarkGo(size);
    allGoResults.push(...goResults);
    printTable(`Go — ${size}`, goResults);
  }

  // --- Rust benchmarks ---
  for (const size of SIZES) {
    console.log(`\nBenchmarking Rust ${size}...`);
    const rustResults = benchmarkRust(size);
    allRustResults.push(...rustResults);
    printTable(`Rust — ${size}`, rustResults);
  }

  // --- Cross-language comparison ---
  printComparison(allMogResults, allGoResults, allRustResults);

  // --- Mog frontend-only comparison ---
  console.log(`\n${"=".repeat(80)}`);
  console.log(`  MOG FRONTEND-ONLY (no LLVM backend) — what embedding would need`);
  console.log(`${"=".repeat(80)}`);
  for (const size of SIZES) {
    const fe = allMogResults.find(r => r.label === `mog/${size}/frontend_total`);
    if (fe) {
      console.log(`  ${size.padEnd(10)} ${fmtMs(fe.median_ms).padStart(10)} median  (lex+parse+analyze+codegen_ir)`);
    }
  }

  // Write raw JSON
  const allResults = { mog: allMogResults, go: allGoResults, rust: allRustResults };
  writeFileSync(`${BUILD_DIR}/benchmark_results.json`, JSON.stringify(allResults, null, 2));
  console.log(`\nRaw results written to ${BUILD_DIR}/benchmark_results.json`);
}

main().catch(console.error);
