# AlgolScript Language Specification

## Overview

AlgolScript is a high-performance, memory-managed programming language designed for LLM-native operations and numerical computing. It features a rich type system with fixed-width integer and floating-point types, native vector/matrix syntax, and built-in LLM operation capabilities.

## Design Philosophy

- **Simplicity**: Clean, predictable syntax without operator precedence complexity
- **Performance**: Native support for vectorized operations and efficient memory management
- **LLM-Native**: First-class support for large language model operations
- **Type Safety**: Strong static typing with comprehensive type system
- **Memory Safety**: Automatic garbage collection ensures memory safety

## Syntax

### General Structure

AlgolScript programs are composed of blocks, statements, and expressions with clear delimitation.

```algol
// Block structure
{
  statement1
  statement2
  result
}
```

### Statements

```algol
// Variable declaration
let name: type = expression

// Assignment
name = expression

// Return statement
return expression  // or just expression as last in block

// Conditional (when)
condition ? (true_block) : (false_block)

// Expression statement
expression
```

### Comments

```algol
// Single line comment

/*
  Multi-line comment
*/
```

## Type System

### Primitive Types

#### Signed Integers

- `i8` - 8-bit signed integer (-128 to 127)
- `i16` - 16-bit signed integer (-32,768 to 32,767)
- `i32` - 32-bit signed integer (-2,147,483,648 to 2,147,483,647)
- `i64` - 64-bit signed integer
- `i128` - 128-bit signed integer
- `i256` - 256-bit signed integer

#### Unsigned Integers

- `u8` - 8-bit unsigned integer (0 to 255)
- `u16` - 16-bit unsigned integer (0 to 65,535)
- `u32` - 32-bit unsigned integer (0 to 4,294,967,295)
- `u64` - 64-bit unsigned integer
- `u128` - 128-bit unsigned integer
- `u256` - 256-bit unsigned integer

#### Floating Point

- `f8` - 8-bit floating point (minimal precision, optimized for ML inference)
- `f16` - 16-bit floating point (half precision)
- `f32` - 32-bit floating point (single precision)
- `f64` - 64-bit floating point (double precision)
- `f128` - 128-bit floating point (quadruple precision)
- `f256` - 256-bit floating point (octuple precision, extended)

### Composite Types

#### Arrays

```algol
// Declaration
let values: [type; size] = [v1, v2, v3, ...]

// Example
let numbers: [i32; 5] = [1, 2, 3, 4, 5]
let floats: [f32; 3] = [1.5, 2.7, 3.14]

// Access
numbers[0]  // First element
numbers[4]  // Last element (0-indexed)
```

#### Tables

```algol
// Declaration - heterogeneous row storage
let data: table = {
  col1: type1,
  col2: type2,
  col3: type3,
  rows: [
    [v1, v2, v3],
    [v4, v5, v6],
  ]
}

// Alternatively, columnar syntax
let data: table = {
  name: [string],
  age: [i32],
  score: [f32],
  data: [
    ["Alice", "Bob", "Charlie"],
    [25, 30, 28],
    [95.5, 88.0, 92.3]
  ]
}

// Access
data.name[0]  // "Alice"
data.age[1]   // 30
```

#### Strings

Strings are represented as arrays of `u8` (UTF-8 encoded):

```algol
let greeting: [u8] = [72, 101, 108, 108, 111]  // "Hello"

// String literals (syntactic sugar)
let greeting: [u8] = "Hello"

// String operations
let length: i32 = string_len(greeting)  // 5
let substring: [u8] = string_slice(greeting, 0, 5)  // "Hello"
```

### Type Annotations

```algol
// Explicit type
let x: i32 = 42

// Type inference (compiler infers from expression)
let x = 42  // Infers i32

// Function return type
fn add(a: i32, b: i32): i32 {
  a + b
}
```

## Semantics

### Memory Management

AlgolScript uses automatic garbage collection for memory safety:

```algol
// Memory is automatically allocated
let data = allocate_large_array()

// Memory is automatically freed when no longer referenced
// Reference counting + cycle detection
{
  let temp = data
}  // temp goes out of scope, reference decreased
```

**GC Features:**

- Generational garbage collection
- Reference counting for immediate cleanup
- Cycle detection for reference cycles
- Configurable collection triggers

### Block Scoping

Variables have block-lexical scope:

```algol
{
  let x: i32 = 10
  {
    let y: i32 = 20
    // x and y are visible here
    x + y  // 30
  }
  // Only x is visible here, y is out of scope
  x  // 10
  // y  // Error: undefined
}
```

### Lexical Scoping

Functions capture variables from their enclosing scope (closures):

```algol
let multiplier: i32 = 5

fn scale(value: i32): i32 {
  value * multiplier  // Captures multiplier from outer scope
}

scale(10)  // Returns 50
multiplier = 10
scale(10)  // Returns 100 (uses current value)
```

### First-Class Functions

Functions are first-class values:

```algol
// Function as value
let adder = fn(a: i32, b: i32): i32 {
  a + b
}

// Higher-order function
fn apply(f: fn(i32, i32): i32, x: i32, y: i32): i32 {
  f(x, y)
}

apply(adder, 3, 4)  // 7
```

## Expressions

### Operator Syntax

AlgolScript does not enforce operator precedence - parentheses are required for complex expressions:

```algol
// Simple expression
a + b

// Parenthesized expression (required for nesting)
(a + b) * c  // Clear, no ambiguity

// Comparison
a == b
a != b
a < b
a > b
a <= b
a >= b

// Logical
a && b  // AND
a || b  // OR
!a      // NOT
```

### Vector Operations

Native syntax for vector and matrix operations:

```algol
// Vector declaration
let v1: [f32; 3] = [1.0, 2.0, 3.0]
let v2: [f32; 3] = [4.0, 5.0, 6.0]

// Element-wise operations
let sum: [f32; 3] = v1 + v2  // [5.0, 7.0, 9.0]
let diff: [f32; 3] = v1 - v2  // [-3.0, -3.0, -3.0]
let product: [f32; 3] = v1 * v2  // [4.0, 10.0, 18.0]
let quot: [f32; 3] = v1 / v2  // [0.25, 0.4, 0.5]

// Scalar operations
let scaled: [f32; 3] = v1 * 2.0  // [2.0, 4.0, 6.0]

// Dot product
let dot: f32 = dot(v1, v2)  // 32.0

// Matrix operations
let m1: [[f32; 2]; 2] = [[1.0, 2.0], [3.0, 4.0]]
let m2: [[f32; 2]; 2] = [[5.0, 6.0], [7.0, 8.0]]

// Matrix multiplication
let product: [[f32; 2]; 2] = matmul(m1, m2)

// Matrix-vector multiplication
let result: [f32; 2] = matmul(m1, [1.0, 2.0])
```

### Map Operation

Apply operations over collections:

```algol
// Map over array
let numbers: [i32; 5] = [1, 2, 3, 4, 5]
let doubled: [i32; 5] = numbers.map(fn(x): i32 { x * 2 })

// Map with type annotation
let doubled: [i32; 5] = numbers.map(fn(x: i32): i32 { x * 2 })

// Map over table column
let ages: [i32; 3] = [25, 30, 28]
let adult_ages: [i32; 3] = ages.map(fn(age: i32): i32 {
  age * 2
})

// Nested map
let matrix: [[i32; 3]; 2] = [[1, 2, 3], [4, 5, 6]]
let transposed: [i32] = matrix.map(fn(row: [i32; 3]): i32 {
  row.map(fn(x: i32): i32 { x * 2 })
})
```

## LLM Operation API

### Syntax

```algol
llm(
  prompt: expression,
  model_size: expression,
  reasoning_effort: expression,
  context: array_expression
): type
```

### Parameters

#### `prompt`

- **Type**: `[u8]` (string)
- **Description**: The prompt text to send to the LLM

#### `model_size`

- **Type**: `string` (enum)
- **Description**: Size/complexity of the model to use
- **Values**: `"tiny" | "small" | "medium" | "large" | "xlarge"`

#### `reasoning_effort`

- **Type**: `f32`
- **Description**: Computational effort for reasoning (0.0 to 1.0)
- **Description**: Higher values produce more thorough reasoning

#### `context`

- **Type**: `[any]`
- **Description**: Array of context items to include in the request
- **Purpose**: Provides additional data, examples, or references for the LLM

#### Return Type

- **Type**: Specified as type parameter
- **Description**: The expected return type from the LLM

### Examples

```algol
// Basic text completion
let response: [u8] = llm(
  prompt: "Complete this sentence: The future of AI is",
  model_size: "medium",
  reasoning_effort: 0.5,
  context: []
)

// Structured output
let analysis: table = llm(
  prompt: "Analyze the sentiment of the following text and return a table with columns: sentiment, confidence, keywords",
  model_size: "large",
  reasoning_effort: 0.8,
  context: [
    "This product is absolutely amazing and I love it!",
    "Terrible experience, would not recommend."
  ]
)

// With context examples
let classification: [u8] = llm(
  prompt: "Classify the following email as: spam, important, or update",
  model_size: "small",
  reasoning_effort: 0.3,
  context: [
    "Example 1: 'Buy now and save 90%' → spam",
    "Example 2: 'Meeting scheduled for tomorrow' → important",
    "Example 3: 'Your account has been updated' → update"
  ]
)

// Numerical output
let score: f32 = llm(
  prompt: "Rate the complexity of this algorithm from 0.0 to 1.0:",
  model_size: "medium",
  reasoning_effort: 0.5,
  context: ["Binary search algorithm"]
)

// Array output
let suggestions: [u8] = llm(
  prompt: "Suggest 3 improvements for this code",
  model_size: "large",
  reasoning_effort: 0.9,
  context: [
    """
    function foo(x) {
      let y = x
      for i in range(10) {
        y = y + i
      }
      return y
    }
    """
  ]
)
```

### Type Checking for LLM Returns

The compiler validates that the structure matches the expected type:

```algol
// Type-safe LLM calls
let result: i32 = llm(...)  // Compiler expects i32 output
let results: [i32; 5] = llm(...)  // Compiler expects array of 5 integers
let data: table = llm(...)  // Compiler expects table structure

// Mismatched types will error at compile time
let wrong: i32 = llm(...)  // Error if LLM returns non-i32
```

## Functions

### Definition

```algol
fn function_name(param1: type1, param2: type2): return_type {
  // Function body
  result_value  // Implicit return of last expression
}
```

### Examples

```algol
// Simple function
fn add(a: i32, b: i32): i32 {
  a + b
}

// Multi-statement function
fn factorial(n: i32): i32 {
  let result: i32 = 1
  let i: i32 = 1
  {
    result = result * i
    i = i + 1
    condition: i <= n ? ({ continue }) : ({ break })
  }
  result
}

// Recursive function
fn fibonacci(n: i32): i32 {
  condition: n <= 1 ? ({ n }) : ({
    fibonacci(n - 1) + fibonacci(n - 2)
  })
}
```

### Higher-Order Functions

```algol
fn map_array(arr: [i32; n], f: fn(i32): i32): [i32; n] {
  let result: [i32; n] = allocate_array(n)
  let i: i32 = 0
  {
    result[i] = f(arr[i])
    i = i + 1
    condition: i < n ? ({ continue }) : ({ break })
  }
  result
}

// Usage
let numbers: [i32; 5] = [1, 2, 3, 4, 5]
let doubled: [i32; 5] = map_array(numbers, fn(x): i32 { x * 2 })
```

## Control Flow

### Conditional Expression

```algol
let result = condition ? (true_value) : (false_value)

// Nested conditionals
let result = condition1 ? (
  condition2 ? (value_a) : (value_b)
) : (
  condition3 ? (value_c) : (value_d)
)
```

### Loop Pattern

```algol
// While-style loop using blocks
{
  // Initialization
  let i: i32 = 0

  // Loop body
  i = i + 1

  // Condition check
  condition: i < 10 ? ({ continue }) : ({ break })
}

// For-style loop
{
  let i: i32 = 0
  {
    // Loop body
    process(array[i])

    // Increment and condition
    i = i + 1
    condition: i < array_len(array) ? ({ continue }) : ({ break })
  }
}
```

## Error Handling

```algol
// Result type pattern
type Result<T> = {
  ok: bool,
  value: T,
  error: [u8]
}

fn divide(a: f32, b: f32): Result<f32> {
  condition: b == 0.0 ? ({
    { ok: false, value: 0.0, error: "Division by zero" }
  }) : ({
    { ok: true, value: a / b, error: "" }
  })
}

let result = divide(10.0, 2.0)
condition: result.ok ? ({
  result.value  // 5.0
}) : ({
  handle_error(result.error)
})
```

## Standard Library (Overview)

### String Operations

- `string_len(s: [u8]): i32`
- `string_slice(s: [u8], start: i32, end: i32): [u8]`
- `string_concat(s1: [u8], s2: [u8]): [u8]`
- `string_compare(s1: [u8], s2: [u8]): i32`

### Array Operations

- `array_len(arr: [T; n]): i32`
- `array_push(arr: [T], value: T): i32`
- `array_pop(arr: [T]): T`
- `array_slice(arr: [T], start: i32, end: i32): [T]`

### Math Operations

- `abs(x: f32): f32`
- `sqrt(x: f32): f32`
- `pow(x: f32, n: f32): f32`
- `sin(x: f32): f32`
- `cos(x: f32): f32`
- `tan(x: f32): f32`

### Vector Operations

- `dot(a: [f32; n], b: [f32; n]): f32`
- `norm(a: [f32; n]): f32`
- `normalize(a: [f32; n]): [f32; n]`
- `matmul(a: [[f32; m]; n], b: [[f32; p]; m]): [[f32; p]; n]`

## Type Conversion

```algol
// Explicit type conversion
fn to_i32(x: f32): i32 {
  // Converts f32 to i32 (truncates)
}

fn to_f32(x: i32): f32 {
  // Converts i32 to f32
}

fn to_u8(x: i32): u8 {
  // Converts i32 to u8 (checked conversion, error on overflow)
}

// Example
let x: f32 = 3.14
let y: i32 = to_i32(x)  // 3
```

## Example Programs

### Simple Calculator

```algol
fn calculate(a: i32, b: i32, op: [u8]): i32 {
  condition: op == "+" ? ({ a + b })
    : op == "-" ? ({ a - b })
    : op == "*" ? ({ a * b })
    : ({ a / b })
}

let result = calculate(10, 5, "+")  // 15
```

### Vector Processing

```algol
fn process_vectors(v1: [f32; 3], v2: [f32; 3]): f32 {
  let sum: [f32; 3] = v1 + v2
  let diff: [f32; 3] = v1 - v2
  let similarity: f32 = dot(v1, v2) / (norm(v1) * norm(v2))
  similarity
}

let a: [f32; 3] = [1.0, 2.0, 3.0]
let b: [f32; 3] = [2.0, 3.0, 4.0]
let sim: f32 = process_vectors(a, b)
```

### LLM-Powered Analysis

```algol
fn analyze_sentiment(text: [u8]): table {
  let result: table = llm(
    prompt: "Analyze sentiment and extract key information:",
    model_size: "medium",
    reasoning_effort: 0.7,
    context: [
      text,
      """
      Return a table with columns:
      - sentiment: [u8] (positive/negative/neutral)
      - confidence: f32 (0.0 to 1.0)
      - topics: [u8] (comma-separated)
      """
    ]
  )
  result
}

let review: [u8] = "This product exceeded my expectations!"
let analysis = analyze_sentiment(review)
```

## Compilation and Execution

### Compilation Phases

1. **Parsing**: Tokenization and AST construction
2. **Type Checking**: Static type validation
3. **Optimization**: Intermediate representation optimization
4. **Code Generation**: Target code generation (binary, WASM, etc.)

### Execution Model

- JIT (Just-In-Time) compilation optional
- Native binary compilation available
- WebAssembly target support
- Debug mode with runtime checks

## Future Extensions

### Planned Features

- Generic types and polymorphic functions
- Pattern matching
- Async/await for concurrent operations
- Module system and package management
- Metaprogramming macros
- Contract-based programming
- Memory regions for performance-critical code

### Considerations

- SIMD vectorization for numeric operations
- GPU acceleration support
- Distributed computing primitives
- Hardware-specific optimizations
