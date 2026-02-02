#!/bin/bash
# Build script for compiling AlgolScript programs with Cosmopolitan Libc
# Usage: ./build_cosmo.sh input.algol [output.com]

set -e

# Check if input file is provided
if [ -z "$1" ]; then
	echo "Usage: $0 input.algol [output.com]"
	echo "Example: $0 test.algol test.com"
	exit 1
fi

INPUT="$1"
BASENAME="${INPUT%.algol}"
OUTPUT="${2:-$BASENAME.com}"

echo "Compiling $INPUT to $OUTPUT with Cosmopolitan Libc..."

# Compile AlgolScript to LLVM IR
echo "Step 1: Generating LLVM IR..."
bun run src/index.ts "$INPUT" >"$BASENAME.ll"

# Optimize IR (optional)
echo "Step 2: Optimizing IR..."
opt -O3 "$BASENAME.ll" -o "$BASENAME.bc" || true
# If opt fails, just use the unoptimized version
if [ ! -f "$BASENAME.bc" ]; then
	echo "Warning: Optimization failed, using unoptimized IR"
	cp "$BASENAME.ll" "$BASENAME.bc"
fi

# Compile to object file (x86-64 target)
echo "Step 3: Compiling to object file..."
llc -filetype=obj -march=x86-64 "$BASENAME.bc" -o "$BASENAME.o"

# Check if Cosmopolitan directory exists
COSMO_DIR="${COSMO_DIR:-/Users/ted/me/algolscript/cosmopc/cosmopolitan}"

if [ ! -d "$COSMO_DIR/build/bootstrap" ]; then
	echo "Warning: Cosmopolitan not found at $COSMO_DIR"
	echo "Attempting to compile with system clang with POSIX support..."
	# Fallback to system compilation with POSIX support
	clang "$BASENAME.o" -L./runtime -lalgol_runtime -o "$OUTPUT"
else
	echo "Step 4: Linking with Cosmopolitan Libc..."
	# Link with Cosmopolitan using lld
	lld -o "$OUTPUT" \
		-nostdlib \
		-fuse-ld=lld \
		-gc-sections \
		-z max-page-size=0x1000 \
		-T "$COSMO_DIR/build/bootstrap/ape.lds" \
		"$COSMO_DIR/build/bootstrap/crt.o" \
		"$COSMO_DIR/build/bootstrap/ape-no-modify-self.o" \
		"$BASENAME.o" \
		"$COSMO_DIR/build/bootstrap/cosmopolitan.a" \
		-L./runtime -lalgol_runtime || {
		echo "Note: Cosmopolitan linking failed, trying system linker with fallback..."
		clang "$BASENAME.o" -L./runtime -lalgol_runtime -o "$OUTPUT"
	}
fi

# Make executable
chmod +x "$OUTPUT"

echo "✓ Build complete: $OUTPUT"
echo ""
echo "To run:"
echo "  ./$OUTPUT"

# Optional: clean up intermediate files
# rm -f "$BASENAME.ll" "$BASENAME.bc" "$BASENAME.o"
