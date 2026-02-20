#!/bin/bash
# Build script for the Mog Plugin Example
#
# Compiles math_plugin.mog into a shared library (.dylib), then builds a
# C host program that loads and calls the plugin at runtime.
#
# Usage:
#   ./build_plugin.sh
#   ./plugin_host ./math_plugin.dylib
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$SCRIPT_DIR/../.."

echo "=== Building Math Plugin ==="
echo ""

# Step 1: Compile math_plugin.mog to a shared library via compilePluginToSharedLib.
# This runs the full pipeline: Mog -> LLVM IR -> llc (PIC) -> clang -dynamiclib.
echo "[1/3] Compiling math_plugin.mog to math_plugin.dylib..."
bun -e "
import { compilePluginToSharedLib } from '$ROOT_DIR/src/compiler.ts';
import { readFileSync } from 'fs';
const source = readFileSync('$SCRIPT_DIR/math_plugin.mog', 'utf-8');
const result = await compilePluginToSharedLib(
  source,
  'math_plugin',
  '$SCRIPT_DIR/math_plugin.dylib',
  '1.0.0'
);
if (result.errors.length > 0) {
  console.error('Compile errors:');
  for (const e of result.errors) {
    console.error('  [' + e.line + ':' + e.column + '] ' + e.message);
  }
  process.exit(1);
}
console.log('  math_plugin.dylib generated');
"

# Step 2: Compile the runtime (needed for the host program).
echo "[2/3] Compiling runtime..."
mkdir -p "$ROOT_DIR/build"
clang -c "$ROOT_DIR/runtime/runtime.c" -o "$ROOT_DIR/build/runtime.o"
clang -c -I"$ROOT_DIR/runtime" "$ROOT_DIR/runtime/mog_vm.c" -o "$ROOT_DIR/build/mog_vm.o"
clang -c -I"$ROOT_DIR/runtime" "$ROOT_DIR/runtime/mog_plugin.c" -o "$ROOT_DIR/build/mog_plugin.o"

# Step 3: Build the host program that loads the plugin.
echo "[3/3] Building plugin_host..."
clang -o "$SCRIPT_DIR/plugin_host" \
	"$SCRIPT_DIR/plugin_host.c" \
	"$ROOT_DIR/build/runtime.o" \
	"$ROOT_DIR/build/mog_vm.o" \
	"$ROOT_DIR/build/mog_plugin.o" \
	-I"$ROOT_DIR/runtime" \
	-lSystem -lm

echo ""
echo "=== Build Complete ==="
echo ""
echo "Run with:"
echo "  $SCRIPT_DIR/plugin_host $SCRIPT_DIR/math_plugin.dylib"
echo ""
echo "Expected output:"
echo "  fibonacci(10) = 55"
echo "  factorial(7) = 5040"
echo "  sum_of_squares(3, 4) = 25"
echo "  gcd(48, 18) = 6"
