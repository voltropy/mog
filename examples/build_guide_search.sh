#!/bin/bash
# Build script for the guide search example
set -e

cd "$(dirname "$0")/.."

echo "=== Guide Search Example Build ==="
echo ""

mkdir -p build

# Step 1: Compile Mog to LLVM IR
echo "[1/5] Compiling guide_search.mog to LLVM IR..."
bun -e "
import { compile } from './src/compiler.ts';
import { readFileSync, writeFileSync } from 'fs';
const source = readFileSync('examples/guide_search.mog', 'utf-8');
const r = await compile(source);
if (r.errors.length) {
  console.error('Compilation errors:');
  for (const e of r.errors) {
    console.error('  [' + e.line + ':' + e.column + '] ' + e.message);
  }
  process.exit(1);
}
writeFileSync('build/guide_search.ll', r.llvmIR);
console.log('  IR generated (' + r.llvmIR.length + ' bytes)');
"

# Step 2: Compile LLVM IR to object file (-O1 for coroutine lowering)
echo "[2/5] Compiling LLVM IR to object..."
clang -O1 -c -x ir build/guide_search.ll -o build/guide_search.o

# Step 3: Compile runtime (use pre-built if available)
echo "[3/5] Compiling runtime..."
clang -c runtime/runtime.c -o build/runtime.o
clang -c -Iruntime runtime/mog_vm.c -o build/mog_vm.o
clang -c -Iruntime runtime/mog_async.c -o build/mog_async.o
clang -c -Iruntime runtime/posix_host.c -o build/posix_host.o

# Step 4: Compile host
echo "[4/5] Compiling host..."
clang -c -Iruntime examples/guide_search_host.c -o build/guide_search_host.o

# Step 5: Link
echo "[5/5] Linking..."
clang build/guide_search.o build/guide_search_host.o build/runtime.o \
	build/mog_vm.o build/mog_async.o build/posix_host.o -o guide_search -lm

if [ -f guide_search ]; then
	echo ""
	echo "Build successful!"
	echo "Run with: ./guide_search"
	echo ""
else
	echo "Build FAILED"
	exit 1
fi
