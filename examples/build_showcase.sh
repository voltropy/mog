#!/bin/bash
# Build script for the Mog Showcase with Host Embedding
#
# This compiles showcase.mog to LLVM IR, compiles the C host and runtime,
# then links everything together into a single executable.
set -e

cd "$(dirname "$0")/.."

echo "=== Mog Showcase Build ==="
echo ""

# Ensure build directory exists
mkdir -p build

# Step 1: Compile showcase.mog to LLVM IR
echo "[1/5] Compiling showcase.mog to LLVM IR..."
bun -e "
import { compile } from './src/compiler.ts';
import { readFileSync, writeFileSync } from 'fs';
const source = readFileSync('showcase.mog', 'utf-8');
const r = await compile(source);
if (r.errors.length) {
  console.error('Compilation errors:');
  for (const e of r.errors) {
    console.error('  [' + e.line + ':' + e.column + '] ' + e.message);
  }
  process.exit(1);
}
writeFileSync('build/showcase.ll', r.llvmIR);
console.log('  IR generated (' + r.llvmIR.length + ' bytes)');
"

# Step 2: Compile LLVM IR to object file
echo "[2/5] Compiling LLVM IR to object..."
clang -c -x ir build/showcase.ll -o build/showcase.o

# Step 3: Compile runtime
echo "[3/5] Compiling runtime..."
clang -c runtime/runtime.c -o build/runtime.o
clang -c -Iruntime runtime/mog_vm.c -o build/mog_vm.o

# Step 4: Compile host
echo "[4/5] Compiling host..."
clang -c -Iruntime examples/host.c -o build/host.o

# Step 5: Link everything
echo "[5/5] Linking..."
clang build/showcase.o build/host.o build/runtime.o build/mog_vm.o -o showcase -lm

echo ""
echo "Build successful!"
echo "Run with: ./showcase"
echo ""
