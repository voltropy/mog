# Mog Array Literal Feature Test Report

## Executive Summary

Comprehensive testing of the array literal feature in Mog revealed **4 major bugs** that need to be fixed. Basic 1D array literals work correctly, but 2D arrays, function parameters, and complex conditions have issues.

## Test Environment

- Compiler: Mog at `/Users/ted/me/mog`
- Command: `bun run src/index.ts <file.mog>`
- Platform: macOS arm64

---

## Test Results by Category

### 1. 1D Array Literals: PARTIALLY WORKING

| Test | Status | Notes |
|------|--------|-------|
| Basic literal `[1, 2, 3]` | PASS | Works correctly |
| Element access `arr[0]` | PASS | Works correctly |
| Element assignment `arr[0] := 100` | PASS | Works correctly |
| Multiple accesses | PASS | Works correctly |
| Print values with `println_i64()` | PASS | Works correctly |
| Simple condition with array | PASS | Works correctly |
| **Compound condition `&&` with arrays** | **FAIL** | Bug #3: LLVM type mismatch |

### 2. 2D Array Literals: NOT WORKING

| Test | Status | Notes |
|------|--------|-------|
| Basic 2D literal `[[1,2],[3,4]]` | FAIL | Bug #1: Type mismatch |
| 2D element access `mat[0][0]` | FAIL | Depends on Bug #1 |
| 2D element assignment | FAIL | Depends on Bug #1 |
| Nested loops with 2D array | FAIL | Depends on Bug #1 |

### 3. Arrays in Loops: MOSTLY WORKING

| Test | Status | Notes |
|------|--------|-------|
| Sum array elements | PASS | Works correctly |
| Modify array in loop | PASS | Works correctly |
| Array loop with condition | PASS | Works correctly |
| Nested loops (1D only) | PASS | Works correctly |

### 4. Arrays as Function Parameters: NOT WORKING

| Test | Status | Notes |
|------|--------|-------|
| Pass array to function | FAIL | Bug #2: LLVM type error |
| Return array element | FAIL | Depends on Bug #2 |
| Modify array in function | FAIL | Depends on Bug #2 |

---

## Bugs Found

### Bug #1: 2D Array Literal Type Inference Failure

**Severity:** HIGH
**Location:** `analyzer.ts:visitArrayLiteral()`

**Problem:**
The `visitArrayLiteral` function only creates a 1D array type with `[node.elements.length]`. For 2D arrays like `[[1,2],[3,4]]`, it infers the type as `[[i64[2]][2]]` but the declared type `i64[][]` becomes `ArrayType(i64, [])` (rank 0).

**Error Message:**
```
Type mismatch: cannot assign [[i64[2]][2]] to [i64[]]
```

**Test Case:**
```mog
fn main() -> i64 {
  matrix: i64[][] = [[1, 2], [3, 4]];  // ERROR
  return 0;
}
```

**Root Cause:**
The `visitArrayLiteral` function needs to recursively compute the nested array dimensions instead of just using the top-level element count.

---

### Bug #2: Array Parameter Passing - LLVM Type Error

**Severity:** HIGH
**Location:** `llvm_codegen.ts`

**Problem:**
When passing an array to a function, the code generator passes an `i64` value instead of a `ptr` type.

**Error Message:**
```
error: '%10' defined with type 'i64' but expected 'ptr'
  %11 = call i64 @array_get(ptr %10, i64 0)
```

**Test Case:**
```mog
fn getFirst(arr: i64[]) -> i64 {
  return arr[0];
}

fn main() -> i64 {
  arr: i64[] = [42, 100, 200];
  result: i64 = getFirst(arr);  // ERROR at call site
  return 0;
}
```

**Root Cause:**
The LLVM code generation for function calls with array arguments is passing the array metadata (size/capacity) instead of the data pointer.

---

### Bug #3: Compound Conditions with Arrays - LLVM Type Mismatch

**Severity:** HIGH
**Location:** `llvm_codegen.ts:generateBinaryExpression()`

**Problem:**
When using `&&` operator with array element comparisons, the code generator uses `and i64` but the operands are `i1` (boolean).

**Error Message:**
```
error: '%16' defined with type 'i1' but expected 'i64'
  %21 = and i64 %16, %20
```

**Test Case:**
```mog
fn main() -> i64 {
  arr: i64[] = [1, 2, 3];
  if (arr[0] == 1 && arr[1] == 2) {  // ERROR
    println_i64(100);
  }
  return 0;
}
```

**Root Cause:**
The logical operators `&&` and `||` should use short-circuit evaluation or at least ensure both operands are cast to the same type before applying bitwise/logical AND.

---

### Bug #4: 3D+ Array Support

**Severity:** MEDIUM
**Location:** Related to Bug #1

**Problem:**
3D arrays fail with the same type inference issue as 2D arrays.

**Error Message:**
```
Type mismatch: cannot assign [[[i64[2]][2]][2]] to [i64[][]]
```

**Root Cause:**
Same as Bug #1 - recursive dimension inference missing.

---

## Working Test Cases

### Basic 1D Array Usage
```mog
fn main() -> i64 {
  arr: i64[] = [1, 2, 3, 4, 5];
  x: i64 = arr[0];
  arr[1] := 100;
  println_i64(arr[1]);
  return 0;
}
```

### Array in Loop
```mog
fn main() -> i64 {
  arr: i64[] = [0, 1, 2, 3, 4];
  sum: i64 = 0;
  i: i64 = 0;
  while (i < 5) {
    sum := sum + arr[i];
    i := i + 1;
  }
  println_i64(sum);  // Output: 10
  return 0;
}
```

---

## Recommendations

### Priority 1: Fix 2D Array Type Inference (Bug #1)

Modify `visitArrayLiteral` in `analyzer.ts` to recursively determine array dimensions.

### Priority 2: Fix Array Parameter Passing (Bug #2)

Update the LLVM code generator to correctly pass array pointers to functions.

### Priority 3: Fix Logical Operators with Boolean Results (Bug #3)

Ensure comparison results are properly typed when used with logical operators.

### Priority 4: Add More Comprehensive Tests

Create a test suite with edge cases:
- Empty arrays
- Single-element arrays
- Very large arrays
- Arrays with expressions as elements
- Multi-dimensional arrays once fixed
