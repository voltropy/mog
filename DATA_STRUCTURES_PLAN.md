# AlgolScript Data Structure Implementation Plan

**Status**: ✅ Completed

## Overview

This document specifies the implementation of three core data structures with unified access syntax but different memory layouts:

| Type | Layout | Use Case | Access Pattern |
|------|--------|----------|----------------|
| `Map` | Hash table | Sparse, dynamic keys | `map["key"]` or `map.key` |
| AoS | Contiguous structs | Row-major access | `aos[i].field` |
| SoA | Columnar arrays | SIMD, column access | `soa.field[i]` |

## Phase 1: Rename table → Map ✅ COMPLETE

### 1.1 Update Type System ✅
- [x] Rename `TableType` class to `MapType` in `src/types.ts`
- [x] Rename `isTableType()` to `isMapType()`
- [x] Update `table()` factory function to `map()`
- [x] Export new names, keep aliases for backward compatibility

### 1.2 Update Parser ✅
- [x] Change parser to emit `MapLiteral` instead of `TableLiteral`
- [x] Keep accepting `{key: value}` syntax (unchanged)
- [x] Remove `table` as a keyword/type name
- [x] Add `Map` as recognized type identifier

### 1.3 Update Analyzer ✅
- [x] Rename `visitTableLiteral` to `visitMapLiteral`
- [x] Update type checking for Map operations
- [x] Update error messages referencing "table" to say "Map"

### 1.4 Update LLVM Codegen ✅
- [x] Rename `generateTableLiteral` to `generateMapLiteral`
- [x] Update runtime call names from `table_*` to `map_*`
- [x] Keep runtime function implementations (hash table logic unchanged)

### 1.5 Update Runtime ✅
- [x] Rename `OBJ_TABLE` to `OBJ_MAP`
- [x] Rename `OBJ_ENTRY` to `OBJ_MAP_ENTRY`
- [x] Rename functions: `table_new` → `map_new`, `table_get` → `map_get`, `table_set` → `map_set`
- [x] Update GC tracing functions

### 1.6 Update Tests ✅
- [x] Replace all `table` references in tests with `Map`
- [x] Rename test cases and descriptions

### 1.7 Update Documentation ✅
- [x] Update `lang_spec.md` to use `Map` instead of `table`
- [x] Update `README.md` examples
- [x] Update any other markdown files

## Phase 2: Struct Types ✅ COMPLETE

### 2.1 Add Struct Type Definition ✅
- [x] Add to `src/types.ts`:
```typescript
class StructType {
  name: string;
  fields: Map<string, Type>;  // field name → type
  
  constructor(name: string, fields: Map<string, Type>)
  toString(): string
}
```

### 2.2 Add Struct Keyword to Lexer ✅
- [x] Add `STRUCT` token type
- [x] Recognize `struct` keyword

### 2.3 Parse Struct Definitions ✅
- [x] Add to `src/parser.ts`:
```
struct_definition := "struct" IDENTIFIER "{" field_list "}"
field_list := field ("," field)*
field := IDENTIFIER ":" type_annotation
```

- [x] Store struct definitions in parser state
- [x] Add `StructDefinition` AST node type

### 2.4 Parse Struct Literals ✅
- [x] Parse `{field: value, field2: value2}` as struct literal when type is known
- [x] Differentiate from Map literal via context (type annotation)
- [x] Add `StructLiteral` AST node

### 2.5 Analyze Struct Definitions ✅
- [x] Validate no duplicate field names
- [x] Validate field types exist
- [x] Store struct definitions in symbol table (type scope)

### 2.6 Analyze Struct Literals ✅
- [x] Check all fields present (or use defaults)
- [x] Type-check each field value
- [x] Infer struct type from context or literal

### 2.7 Analyze Struct Access ✅
- [x] Parse `expr.field` as `MemberExpression`
- [x] Type-check: verify expr is struct type, field exists
- [x] Return field type

### 2.8 Codegen Struct Types ✅
- [x] Map struct to LLVM struct type: `{field1_type, field2_type, ...}`
- [x] Handle alignment and padding

### 2.9 Codegen Struct Literals ✅
- [x] Allocate struct on stack
- [x] Store each field value
- [x] Return pointer to struct

### 2.10 Codegen Struct Access ✅
- [x] Calculate field offset
- [x] Generate `getelementptr` to field
- [x] Load field value

### 2.11 Codegen Struct Assignment ✅
- [x] Copy struct field-by-field (value semantics)
- [x] Or use `memcpy` for efficiency

## Phase 3: AoS (Array of Structs) ✅ COMPLETE

### 3.1 Add AoS Type ✅
- [x] Add to `src/types.ts`:
```typescript
class AOSType {
  elementType: StructType;
  size: number | null;  // null for dynamic
  
  toString(): string  // [StructName] or [StructName; N]
}
```

### 3.2 Parse AoS Type Annotations ✅
- [x] Parse `[StructName]` as dynamic AoS
- [x] Parse `[StructName; N]` as fixed-capacity AoS
- [x] Store in type system

### 3.3 Parse AoS Literals ✅
- [x] Parse `[]` as empty AoS
- [x] Parse `[{field: val}, {field: val}]` as AoS literal
- [x] Infer element type from first element or annotation

### 3.4 Analyze AoS Types ✅
- [x] Validate struct type exists
- [x] Validate size is positive integer if present

### 3.5 Analyze AoS Literals ✅
- [x] Type-check each element against struct type
- [x] Check all elements have same type

### 3.6 Analyze AoS Access ✅
- [x] Parse `aos[i].field`:
  - `aos[i]` → IndexExpression returns struct instance
  - `.field` → MemberExpression on struct
- [x] Type-check index is integer
- [x] Return field type

### 3.7 Analyze AoS Assignment ✅
- [x] `aos[i].field := value`: type-check value against field type
- [x] `aos[i] := struct_value`: type-check struct value

### 3.8 Add Built-in Functions (AoS) ✅
- [x] `push(aos, element)` - append to end
- [x] `len(aos)` - return element count
- [x] `cap(aos)` - return allocated capacity
- [x] `reserve(aos, n)` - pre-allocate space
- [x] `shrink(aos)` - shrink to fit

### 3.9 Codegen AoS Runtime Type ✅
- [x] Implement in runtime:
```c
typedef struct {
    ObjKind kind;           // OBJ_AOS
    uint32_t refcnt;
    void* data;             // Page-aligned struct array
    uint64_t len;           // Current count
    uint64_t cap;           // Allocated capacity
    uint64_t element_size;  // Size of each struct
} AOSObject;
```

### 3.10 Codegen AoS Allocation ✅
- [x] Allocate AoS header via GC
- [x] Allocate data buffer with `posix_memalign(PAGE_SIZE, size)`
- [x] Store pointer in header

### 3.11 Codegen AoS Access ✅
- [x] Bounds check (optional in release mode)
- [x] Calculate offset: `base + index * element_size`
- [x] Generate `getelementptr` for field access

### 3.12 Codegen AoS push ✅
- [x] Check capacity, reallocate if needed (2x growth)
- [x] Copy element to `data + len * element_size`
- [x] Increment len

### 3.13 Codegen AoS len/cap ✅
- [x] Direct field access on AoS header

### 3.14 GC Integration for AoS ✅
- [x] Trace AoS header as GC object
- [x] Data buffer is owned by header, freed when header collected
- [x] No interior pointers to trace (struct values only)

## Phase 4: SoA (Struct of Arrays) ✅ COMPLETE

### 4.1 Add SoA Type ✅
- [x] Add to `src/types.ts`:
```typescript
class SOAType {
  fields: Map<string, ArrayType>;  // field name → array type
  
  toString(): string  // {field1: [type], field2: [type]}
}
```

### 4.2 Parse SoA Type Definitions ✅
- [x] Implement syntax:
```
struct SoAName {
    field1: [type],
    field2: [type],
    ...
}
```
- [x] Each field must be an array type
- [x] All arrays must have same length (enforced at runtime)

### 4.3 Parse SoA Literals ✅
- [x] Parse literal syntax:
```
{
    field1: [v1, v2, v3],
    field2: [v4, v5, v6]
}
```
- [x] All field arrays must have same length
- [x] Store as SoA literal AST node

### 4.4 Analyze SoA Types ✅
- [x] Validate all fields are array types
- [x] Validate element types are valid

### 4.5 Analyze SoA Literals ✅
- [x] Check all field arrays have same length
- [x] Type-check each element in each array

### 4.6 Analyze SoA Access ✅
- [x] Parse `soa.field[i]`:
  - `soa.field` → MemberExpression returns array
  - `[i]` → IndexExpression on array
- [x] Type-check field exists, index is integer
- [x] Return element type

### 4.7 Analyze SoA Assignment ✅
- [x] `soa.field[i] := value`: type-check value

### 4.8 Add Built-in Functions (SoA) ✅
- [x] `push(soa, element)` - push to all columns
- [x] `len(soa)` - return length (from any column)
- [x] `cap(soa)` - return capacity

### 4.9 Codegen SoA Runtime Type ✅
- [x] Implement in runtime:
```c
typedef struct {
    ObjKind kind;           // OBJ_SOA
    uint32_t refcnt;
    uint64_t num_fields;    // Number of columns
    char** field_names;     // Array of field name strings
    void** columns;         // Array of page-aligned column buffers
    uint64_t* element_sizes; // Size of each element per column
    uint64_t len;           // Current element count
    uint64_t cap;           // Allocated capacity
} SOAObject;
```

### 4.10 Codegen SoA Allocation ✅
- [x] Allocate SoA header via GC
- [x] For each field: allocate column buffer with `posix_memalign(PAGE_SIZE, size)`
- [x] Store column pointers in header

### 4.11 Codegen SoA Access ✅
- [x] Field access: look up column index by field name
- [x] Generate pointer arithmetic for column buffer
- [x] Bounds check

### 4.12 Codegen SoA push ✅
- [x] For each column: check capacity, reallocate if needed
- [x] Copy field value to each column buffer at `data + len * elem_size`
- [x] Increment shared len

### 4.13 GC Integration for SoA ✅
- [x] Trace SoA header as GC object
- [x] Trace each column buffer (owned by header)
- [x] Free all column buffers when header collected

## Phase 5: Integration and Testing ✅ COMPLETE

### 5.1 Update Type Compatibility ✅
- [x] Define rules for Map/Struct/AoS/SoA compatibility
- [x] Generally: no implicit conversions, explicit construction only

### 5.2 Update Analyzer Error Messages ✅
- [x] Ensure all new types have good error messages
- [x] Suggest fixes for common mistakes

### 5.3 Add Comprehensive Tests ✅
- [x] Unit tests for each new type
- [x] Integration tests for combined usage
- [x] Performance tests comparing AoS vs SoA

### 5.4 Add Documentation Examples ✅
- [x] Complete examples in `lang_spec.md`
- [x] Performance guidelines (when to use what)
- [x] Migration guide from old `table` syntax

### 5.5 Optimize Hot Paths ✅
- [x] Inline small struct operations
- [x] Vectorization hints for SoA column operations
- [x] Bounds check elision in release mode

## Dependencies

```
Phase 1 (Map rename)
    ↓
Phase 2 (Struct)
    ↓
Phase 3 (AoS) ← depends on Struct
    ↓
Phase 4 (SoA) ← depends on AoS patterns
    ↓
Phase 5 (Integration)
```

## Notes

- **Page alignment**: Use `posix_memalign` with `sysconf(_SC_PAGESIZE)`
- **Capacity growth**: Start with 8 or 16, double on overflow
- **Type inference**: AoS/SoA literals need type context or explicit annotation
- **Error handling**: Clear messages for length mismatches in SoA
- **Performance**: SoA column operations should be auto-vectorizable by LLVM

---

## Usage Examples

### Map Examples

```algol
// Map literal with string keys
config := {host: "localhost", port: 8080};

// Access patterns
println(config.host);      // dot notation
println(config["port"]);   // bracket notation

// Dynamic keys
key := "host";
println(config[key]);      // runtime key lookup
```

### Struct Examples

```algol
// Struct definition
struct Point {
    x: f64,
    y: f64
}

// Struct literal (type inferred from context)
p: Point := {x: 1.0, y: 2.0};

// Field access
println(p.x);
p.x := 5.0;  // Assignment
```

### AoS (Array of Structs) Examples

```algol
// Define a struct first
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64
}

// AoS type annotation and literal
particles: [Particle] := [
    {x: 0.0, y: 0.0, vx: 1.0, vy: 0.0},
    {x: 1.0, y: 1.0, vx: 0.0, vy: 1.0}
];

// Access: index then field
println(particles[0].x);  // 0.0
particles[0].x := 5.0;

// Built-in functions
push(particles, {x: 2.0, y: 2.0, vx: 0.0, vy: 0.0});
n := len(particles);
c := cap(particles);
```

### SoA (Struct of Arrays) Examples

```algol
// SoA struct definition - each field is an array
struct Particles {
    x: [f64],
    y: [f64],
    vx: [f64],
    vy: [f64]
}

// SoA literal - all arrays same length
particles: Particles := {
    x: [0.0, 1.0, 2.0],
    y: [0.0, 1.0, 2.0],
    vx: [1.0, 0.0, 0.5],
    vy: [0.0, 1.0, 0.5]
};

// Access: field then index (enables SIMD/vectorization)
println(particles.x[0]);  // 0.0
particles.x[0] := 5.0;

// Column operations (vectorized)
for i := 0 to len(particles) {
    particles.x[i] := particles.x[i] + particles.vx[i];
    particles.y[i] := particles.y[i] + particles.vy[i];
}
```

### Performance Comparison

```algol
// AoS: Good for row-major access (process whole struct at once)
for i := 0 to len(particles) {
    updateParticle(particles[i]);  // Cache-friendly per-particle
}

// SoA: Good for column operations/SIMD
for i := 0 to len(particles) {
    particles.x[i] := particles.x[i] * 2.0;  // Sequential x access
}
```

### Migration Guide (table → Map)

```algol
// Old syntax (deprecated)
tbl: table := {a: 1, b: 2};

// New syntax - explicit Map type
m: Map := {a: 1, b: 2};

// Or let type inference handle it
m := {a: 1, b: 2};
```

## Performance Guidelines

| Data Structure | Best For | Memory Layout | Access Pattern |
|---------------|----------|---------------|----------------|
| Map | Sparse data, dynamic keys | Hash table | `map[key]` or `map.key` |
| Struct | Grouped data, fixed fields | Contiguous | `struct.field` |
| AoS | Row-major operations | Array of structs | `aos[i].field` |
| SoA | Column operations, SIMD | Struct of arrays | `soa.field[i]` |

**Rule of thumb**: Use AoS when you frequently access all fields of an element together. Use SoA when you perform vectorized operations on individual fields across many elements.
