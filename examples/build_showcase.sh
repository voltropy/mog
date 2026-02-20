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
echo "[1/6] Compiling showcase.mog to LLVM IR..."
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

# Step 2: Compile LLVM IR to object file (needs -O1 for coroutine lowering)
echo "[2/6] Compiling LLVM IR to object..."
clang -O1 -c -x ir build/showcase.ll -o build/showcase.o

# Step 3: Compile runtime
echo "[3/6] Compiling runtime..."
clang -c runtime/runtime.c -o build/runtime.o
clang -c -Iruntime runtime/mog_vm.c -o build/mog_vm.o
clang -c -Iruntime runtime/mog_async.c -o build/mog_async.o
clang -c -Iruntime runtime/posix_host.c -o build/posix_host.o
clang -c -Iruntime runtime/mog_plugin.c -o build/mog_plugin.o

# Step 4: Compile host
echo "[4/6] Compiling host..."
clang -c -Iruntime examples/host.c -o build/host.o

# Step 5: Link everything
echo "[5/6] Linking..."
clang build/showcase.o build/host.o build/runtime.o build/mog_vm.o \
	build/mog_async.o build/posix_host.o build/mog_plugin.o -o showcase -lm

# Step 6: Verify
echo "[6/6] Verifying..."
if [ -f showcase ]; then
	echo ""
	echo "Build successful!"
	echo "Run with: ./showcase"
	echo ""
else
	echo "Build FAILED - no executable produced"
	exit 1
fi
