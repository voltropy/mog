#!/bin/bash

LLC="/opt/homebrew/opt/llvm/bin/llc"
RUNTIME="./build/runtime.a"

echo "Testing all .algol files..."
echo ""

for algol_file in *.algol; do
	if [ ! -f "$algol_file" ]; then
		continue
	fi

	basename=$(basename "$algol_file" .algol)
	echo "=== Testing $algol_file ==="

	# Compile
	./compile "$algol_file" >/dev/null 2>&1
	if [ $? -ne 0 ]; then
		echo "  FAILED: Compilation error"
		continue
	fi

	# LLVM IR to object
	$LLC -filetype=obj build/temp.ll -o "build/${basename}.o" 2>&1 | grep -i error
	if [ ${PIPESTATUS[0]} -ne 0 ]; then
		echo "  FAILED: LLVM IR compilation error"
		continue
	fi

	# Link
	clang "build/${basename}.o" "$RUNTIME" -o "$basename" 2>&1 | grep -i error
	if [ ${PIPESTATUS[0]} -ne 0 ]; then
		echo "  FAILED: Linking error"
		continue
	fi

	# Run
	./"$basename" >/dev/null 2>&1
	exit_code=$?

	echo "  Exit code: $exit_code"
	if [ $exit_code -eq 0 ]; then
		echo "  SUCCESS"
	else
		echo "  Compiled and ran with exit code $exit_code"
	fi
	echo ""
done

echo "Done!"
