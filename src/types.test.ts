import { describe, test, expect } from "bun:test"
import * as types from "./types"
import {
  i8,
  i16,
  i32,
  i64,
  i128,
  i256,
  u8,
  u16,
  u32,
  u64,
  u128,
  u256,
  f8,
  f16,
  f32,
  f64,
  f128,
  f256,
  voidType,
  stringType,
  IntegerType,
  UnsignedType,
  FloatType,
  ArrayType,
  MapType,
  VoidType,
  StructType,
  AOSType,
  SOAType,
  TypeVar,
  TypeInferenceContext,
  array,
  map,
  isIntegerType,
  isUnsignedType,
  isFloatType,
  isArrayType,
  isMapType,
  isVoidType,
  isStructType,
  isAOSType,
  isSOAType,
  isNumericType,
  isSigned,
  sameType,
  canCoerce,
  getCommonType,
  checkBounds,
  inferType,
  compatibleTypes,
} from "./types"

// Helper functions for creating Map-based struct fields
function createFieldMap(fields: { name: string; type: types.Type }[]): Map<string, types.Type> {
  const map = new Map<string, types.Type>()
  for (const f of fields) {
    map.set(f.name, f.type)
  }
  return map
}

function createSoAFieldMap(fields: { name: string; type: ArrayType }[]): Map<string, ArrayType> {
  const map = new Map<string, ArrayType>()
  for (const f of fields) {
    map.set(f.name, f.type)
  }
  return map
}

function createSOAType(structName: string, fields: { name: string; type: any }[], capacity: number | null = null): types.SOAType {
  const structType = new types.StructType(structName, createFieldMap(fields))
  return new types.SOAType(structType, capacity)
}

describe("Type Constructors", () => {
  describe("Integer Types", () => {
    test("i8 constructor creates integer type with 8 bits", () => {
      expect(i8.kind).toBe("i8")
      expect(i8.bits).toBe(8)
      expect(i8.isSigned()).toBe(true)
    })

    test("i16 constructor creates integer type with 16 bits", () => {
      expect(i16.kind).toBe("i16")
      expect(i16.bits).toBe(16)
      expect(i16.isSigned()).toBe(true)
    })

    test("i32 constructor creates integer type with 32 bits", () => {
      expect(i32.kind).toBe("i32")
      expect(i32.bits).toBe(32)
      expect(i32.isSigned()).toBe(true)
    })

    test("i64 constructor creates integer type with 64 bits", () => {
      expect(i64.kind).toBe("i64")
      expect(i64.bits).toBe(64)
      expect(i64.isSigned()).toBe(true)
    })

    test("i128 constructor creates integer type with 128 bits", () => {
      expect(i128.kind).toBe("i128")
      expect(i128.bits).toBe(128)
      expect(i128.isSigned()).toBe(true)
    })

    test("i256 constructor creates integer type with 256 bits", () => {
      expect(i256.kind).toBe("i256")
      expect(i256.bits).toBe(256)
      expect(i256.isSigned()).toBe(true)
    })

    test("integer type min and max values are correct", () => {
      expect(i8.minValue).toBe(-128n)
      expect(i8.maxValue).toBe(127n)
      expect(i16.minValue).toBe(-32768n)
      expect(i16.maxValue).toBe(32767n)
    })
  })

  describe("Unsigned Types", () => {
    test("u8 constructor creates unsigned type with 8 bits", () => {
      expect(u8.kind).toBe("u8")
      expect(u8.bits).toBe(8)
      expect(u8.isSigned()).toBe(false)
    })

    test("u16 constructor creates unsigned type with 16 bits", () => {
      expect(u16.kind).toBe("u16")
      expect(u16.bits).toBe(16)
      expect(u16.isSigned()).toBe(false)
    })

    test("u32 constructor creates unsigned type with 32 bits", () => {
      expect(u32.kind).toBe("u32")
      expect(u32.bits).toBe(32)
      expect(u32.isSigned()).toBe(false)
    })

    test("u64 constructor creates unsigned type with 64 bits", () => {
      expect(u64.kind).toBe("u64")
      expect(u64.bits).toBe(64)
      expect(u64.isSigned()).toBe(false)
    })

    test("u128 constructor creates unsigned type with 128 bits", () => {
      expect(u128.kind).toBe("u128")
      expect(u128.bits).toBe(128)
      expect(u128.isSigned()).toBe(false)
    })

    test("u256 constructor creates unsigned type with 256 bits", () => {
      expect(u256.kind).toBe("u256")
      expect(u256.bits).toBe(256)
      expect(u256.isSigned()).toBe(false)
    })

    test("unsigned type min and max values are correct", () => {
      expect(u8.minValue).toBe(0n)
      expect(u8.maxValue).toBe(255n)
      expect(u16.minValue).toBe(0n)
      expect(u16.maxValue).toBe(65535n)
    })
  })

  describe("Float Types", () => {
    test("f8 constructor creates float type with 8 bits", () => {
      expect(f8.kind).toBe("f8")
      expect(f8.bits).toBe(8)
    })

    test("f16 constructor creates float type with 16 bits", () => {
      expect(f16.kind).toBe("f16")
      expect(f16.bits).toBe(16)
    })

    test("f32 constructor creates float type with 32 bits", () => {
      expect(f32.kind).toBe("f32")
      expect(f32.bits).toBe(32)
    })

    test("f64 constructor creates float type with 64 bits", () => {
      expect(f64.kind).toBe("f64")
      expect(f64.bits).toBe(64)
    })

    test("f128 constructor creates float type with 128 bits", () => {
      expect(f128.kind).toBe("f128")
      expect(f128.bits).toBe(128)
    })

    test("f256 constructor creates float type with 256 bits", () => {
      expect(f256.kind).toBe("f256")
      expect(f256.bits).toBe(256)
    })
  })

  describe("Array Types", () => {
    test("array function creates basic array type", () => {
      const arr = array(i32, [])
      expect(arr.elementType).toBe(i32)
      expect(arr.rank).toBe(0)
    })

    test("array with dimensions creates multi-dimensional array", () => {
      const arr = array(i32, [3, 4])
      expect(arr.rank).toBe(2)
      expect(arr.dimensions).toEqual([3, 4])
    })

    test("stringType is u8 array", () => {
      expect(stringType.elementType).toBe(u8)
      expect(stringType.rank).toBe(0)
      expect(stringType.isString).toBe(false)
    })

    test("1D array of u8 is detected as string", () => {
      const str = array(u8, [10])
      expect(str.isString).toBe(true)
    })

    test("2D array of u8 is not a string", () => {
      const arr = array(u8, [10, 20])
      expect(arr.isString).toBe(false)
    })
  })

  describe("Table Types", () => {
    test("map function creates map type", () => {
      const tbl = map(i32, f64)
      expect(tbl.keyType).toBe(i32)
      expect(tbl.valueType).toBe(f64)
    })
  })

  describe("Void Type", () => {
    test("voidType is singleton", () => {
      expect(voidType).toBeInstanceOf(VoidType)
    })
  })
})

describe("Type Guards", () => {
  test("isIntegerType identifies integer types", () => {
    expect(isIntegerType(i32)).toBe(true)
    expect(isIntegerType(i64)).toBe(true)
    expect(isIntegerType(u32)).toBe(false)
    expect(isIntegerType(f32)).toBe(false)
  })

  test("isUnsignedType identifies unsigned types", () => {
    expect(isUnsignedType(u32)).toBe(true)
    expect(isUnsignedType(u64)).toBe(true)
    expect(isUnsignedType(i32)).toBe(false)
    expect(isUnsignedType(f32)).toBe(false)
  })

  test("isFloatType identifies float types", () => {
    expect(isFloatType(f32)).toBe(true)
    expect(isFloatType(f64)).toBe(true)
    expect(isFloatType(i32)).toBe(false)
    expect(isFloatType(u32)).toBe(false)
  })

  test("isArrayType identifies array types", () => {
    expect(isArrayType(array(i32, []))).toBe(true)
    expect(isArrayType(array(i32, [3]))).toBe(true)
    expect(isArrayType(i32)).toBe(false)
  })

  test("isMapType identifies map types", () => {
    expect(isMapType(map(i32, f64))).toBe(true)
    expect(isMapType(i32)).toBe(false)
  })

  test("isVoidType identifies void type", () => {
    expect(isVoidType(voidType)).toBe(true)
    expect(isVoidType(i32)).toBe(false)
  })

  test("isNumericType identifies numeric types", () => {
    expect(isNumericType(i32)).toBe(true)
    expect(isNumericType(u32)).toBe(true)
    expect(isNumericType(f32)).toBe(true)
    expect(isNumericType(array(i32, []))).toBe(false)
    expect(isNumericType(voidType)).toBe(false)
  })
})

describe("isSigned", () => {
  test("integer types are signed", () => {
    expect(isSigned(i8)).toBe(true)
    expect(isSigned(i16)).toBe(true)
    expect(isSigned(i32)).toBe(true)
    expect(isSigned(i64)).toBe(true)
    expect(isSigned(i128)).toBe(true)
    expect(isSigned(i256)).toBe(true)
  })

  test("unsigned types are not signed", () => {
    expect(isSigned(u8)).toBe(false)
    expect(isSigned(u16)).toBe(false)
    expect(isSigned(u32)).toBe(false)
    expect(isSigned(u64)).toBe(false)
    expect(isSigned(u128)).toBe(false)
    expect(isSigned(u256)).toBe(false)
  })
})

describe("sameType", () => {
  test("same integer types are equal", () => {
    expect(sameType(i32, i32)).toBe(true)
    expect(sameType(i64, i64)).toBe(true)
  })

  test("different integer types are not equal", () => {
    expect(sameType(i32, i64)).toBe(false)
    expect(sameType(i8, i32)).toBe(false)
  })

  test("same unsigned types are equal", () => {
    expect(sameType(u32, u32)).toBe(true)
    expect(sameType(u64, u64)).toBe(true)
  })

  test("different unsigned types are not equal", () => {
    expect(sameType(u32, u64)).toBe(false)
  })

  test("same float types are equal", () => {
    expect(sameType(f32, f32)).toBe(true)
    expect(sameType(f64, f64)).toBe(true)
  })

  test("different float types are not equal", () => {
    expect(sameType(f32, f64)).toBe(false)
  })

  test("integer and unsigned of same bits are not equal", () => {
    expect(sameType(i32, u32)).toBe(false)
  })

  test("array types with same element and dimensions are equal", () => {
    const arr1 = array(i32, [3, 4])
    const arr2 = array(i32, [3, 4])
    expect(sameType(arr1, arr2)).toBe(true)
  })

  test("array types with different dimensions are not equal", () => {
    const arr1 = array(i32, [3, 4])
    const arr2 = array(i32, [4, 3])
    expect(sameType(arr1, arr2)).toBe(false)
  })

  test("array types with different element types are not equal", () => {
    const arr1 = array(i32, [3, 4])
    const arr2 = array(f32, [3, 4])
    expect(sameType(arr1, arr2)).toBe(false)
  })

  test("map types with same key and value are equal", () => {
    const tbl1 = map(i32, f64)
    const tbl2 = map(i32, f64)
    expect(sameType(tbl1, tbl2)).toBe(true)
  })

  test("map types with different key or value are not equal", () => {
    const tbl1 = map(i32, f64)
    const tbl2 = map(i32, f32)
    const tbl3 = map(u32, f64)
    expect(sameType(tbl1, tbl2)).toBe(false)
    expect(sameType(tbl1, tbl3)).toBe(false)
  })

  test("void types are equal", () => {
    expect(sameType(voidType, voidType)).toBe(true)
  })

  test("different type categories are not equal", () => {
    expect(sameType(i32, f32)).toBe(false)
    expect(sameType(i32, array(i32, []))).toBe(false)
    expect(sameType(i32, voidType)).toBe(false)
  })
})

describe("canCoerce (strict mode - no implicit coercion)", () => {
  test("only same type can coerce", () => {
    expect(canCoerce(i32, i32)).toBe(true)
    expect(canCoerce(u32, u32)).toBe(true)
    expect(canCoerce(f32, f32)).toBe(true)
  })

  test("different types cannot coerce", () => {
    expect(canCoerce(i8, i16)).toBe(false)
    expect(canCoerce(i32, i64)).toBe(false)
    expect(canCoerce(u32, i32)).toBe(false)
    expect(canCoerce(i32, f32)).toBe(false)
    expect(canCoerce(array(i32, []), array(i64, []))).toBe(false)
    expect(canCoerce(map(i32, f64), map(i32, f32))).toBe(false)
  })
})

describe("getCommonType", () => {
  test("same type returns the type itself", () => {
    expect(getCommonType(i32, i32)).toBe(i32)
    expect(getCommonType(u32, u32)).toBe(u32)
    expect(getCommonType(f32, f32)).toBe(f32)
  })

  test("void type returns the other type", () => {
    expect(getCommonType(voidType, i32)).toBe(i32)
    expect(getCommonType(i32, voidType)).toBe(i32)
    expect(getCommonType(voidType, voidType)).toBe(voidType)
  })

  test("larger integer type for two integers", () => {
    expect(getCommonType(i8, i16)).toBe(i16)
    expect(getCommonType(i16, i32)).toBe(i32)
    expect(getCommonType(i32, i64)).toBe(i64)
    expect(getCommonType(i64, i256)).toBe(i256)
    expect(getCommonType(i16, i8)).toBe(i16)
  })

  test("larger unsigned type for two unsigned", () => {
    expect(getCommonType(u8, u16)).toBe(u16)
    expect(getCommonType(u16, u32)).toBe(u32)
    expect(getCommonType(u32, u64)).toBe(u64)
    expect(getCommonType(u16, u8)).toBe(u16)
  })

  test("larger float type for two floats", () => {
    expect(getCommonType(f8, f16)).toBe(f16)
    expect(getCommonType(f16, f32)).toBe(f32)
    expect(getCommonType(f32, f64)).toBe(f64)
    expect(getCommonType(f16, f8)).toBe(f16)
  })

  test("integer and float returns float", () => {
    expect(getCommonType(i32, f32)).toBe(f32)
    expect(getCommonType(i32, f64)).toBe(f64)
  })

  test("unsigned and float returns float", () => {
    expect(getCommonType(u32, f32)).toBe(f32)
    expect(getCommonType(u64, f64)).toBe(f64)
  })

  test("integer and unsigned returns signed integer with enough bits", () => {
    const result = getCommonType(i8, u8)
    expect(result).toBeInstanceOf(IntegerType)
    if (result) expect((result as IntegerType).bits).toBeGreaterThanOrEqual(8)

    const result2 = getCommonType(i16, u8)
    expect(result2).toBeInstanceOf(IntegerType)
    if (result2) expect((result2 as IntegerType).kind).toBe("i16")

    const result3 = getCommonType(i8, u16)
    expect(result3).toBeInstanceOf(IntegerType)
    if (result3) expect((result3 as IntegerType).bits).toBeGreaterThanOrEqual(16)
  })

  test("non-numeric types return null", () => {
    expect(getCommonType(array(i32, []), i32)).toBe(null)
    expect(getCommonType(map(i32, f64), i32)).toBe(null)
    expect(getCommonType(array(i32, []), map(u8, u32))).toBe(null)
  })

  test("void type behavior in getCommonType", () => {
    expect(getCommonType(voidType, i32)).toBe(i32)
    expect(getCommonType(i32, voidType)).toBe(i32)
    const result = getCommonType(voidType, array(i32, []))
    expect(result).toBeInstanceOf(ArrayType)
    if (result) {
      expect(sameType(result, array(i32, []))).toBe(true)
    }
  })

  test("equal bit sizes of int and unsigned return signed", () => {
    const result = getCommonType(i32, u32)
    expect(result).toBeInstanceOf(IntegerType)
    if (result) expect((result as IntegerType).kind).toBe("i32")
  })
})

describe("checkBounds", () => {
  test("values within bounds return true", () => {
    expect(checkBounds(i8, 0n)).toBe(true)
    expect(checkBounds(i8, 127n)).toBe(true)
    expect(checkBounds(i8, -128n)).toBe(true)
    expect(checkBounds(u8, 0n)).toBe(true)
    expect(checkBounds(u8, 255n)).toBe(true)
    expect(checkBounds(i32, 100000n)).toBe(true)
  })

  test("values at boundaries return true", () => {
    expect(checkBounds(i8, i8.maxValue)).toBe(true)
    expect(checkBounds(i8, i8.minValue)).toBe(true)
    expect(checkBounds(u8, u8.maxValue)).toBe(true)
    expect(checkBounds(u8, u8.minValue)).toBe(true)
  })

  test("values exceeding max return false", () => {
    expect(checkBounds(i8, 128n)).toBe(false)
    expect(checkBounds(i8, 1000n)).toBe(false)
    expect(checkBounds(i16, 40000n)).toBe(false)
    expect(checkBounds(u8, 256n)).toBe(false)
    expect(checkBounds(u8, 1000n)).toBe(false)
  })

  test("values below min return false for signed", () => {
    expect(checkBounds(i8, -129n)).toBe(false)
    expect(checkBounds(i8, -1000n)).toBe(false)
    expect(checkBounds(i16, -40000n)).toBe(false)
  })

  test("negative values return false for unsigned", () => {
    expect(checkBounds(u8, -1n)).toBe(false)
    expect(checkBounds(u16, -1n)).toBe(false)
    expect(checkBounds(u32, -100n)).toBe(false)
  })
})

describe("TypeVar", () => {
  test("TypeVar with no constraints satisfies all types", () => {
    const tv = new TypeVar("T")
    expect(tv.satisfies(i32)).toBe(true)
    expect(tv.satisfies(f64)).toBe(true)
    expect(tv.satisfies(array(i32, []))).toBe(true)
  })

  test("TypeVar with constraint satisfies only exact types (strict mode)", () => {
    const tv = new TypeVar("T", [{ type: i32 }])
    expect(tv.satisfies(i8)).toBe(false)
    expect(tv.satisfies(i16)).toBe(false)
    expect(tv.satisfies(i32)).toBe(true)
    expect(tv.satisfies(i64)).toBe(false)
    expect(tv.satisfies(f32)).toBe(false)
  })

  test("TypeVar with multiple constraints (strict mode)", () => {
    const tv = new TypeVar("T", [{ type: i32 }, { type: f64 }])
    expect(tv.satisfies(i8)).toBe(false)
    expect(tv.satisfies(i16)).toBe(false)
    expect(tv.satisfies(i32)).toBe(true)
    expect(tv.satisfies(f32)).toBe(false)
    expect(tv.satisfies(f64)).toBe(true)
    expect(tv.satisfies(i64)).toBe(false)
    expect(tv.satisfies(u32)).toBe(false)
  })
})

describe("TypeInferenceContext", () => {
  test("bind and lookup types", () => {
    const ctx = new TypeInferenceContext()
    ctx.bind("x", i32)
    ctx.bind("y", f64)
    expect(ctx.lookup("x")).toBe(i32)
    expect(ctx.lookup("y")).toBe(f64)
    expect(ctx.lookup("z")).toBe(null)
  })

  test("infer returns type from expression", () => {
    const ctx = new TypeInferenceContext()
    expect(ctx.infer({ type: i32 })).toBe(i32)
    expect(ctx.infer({ type: null })).toBe(null)
    expect(ctx.infer({})).toBe(null)
  })

  test("unify same types returns true", () => {
    const ctx = new TypeInferenceContext()
    expect(ctx.unify(i32, i32)).toBe(true)
    expect(ctx.unify(f64, f64)).toBe(true)
  })

  test("unify coercible types returns false (strict mode)", () => {
    const ctx = new TypeInferenceContext()
    expect(ctx.unify(i8, i32)).toBe(false)
    expect(ctx.unify(u8, i16)).toBe(false)
    expect(ctx.unify(i32, f64)).toBe(false)
  })

  test("unify non-coercible types returns false", () => {
    const ctx = new TypeInferenceContext()
    expect(ctx.unify(i32, u32)).toBe(false)
  })

  test("unify with one-way coercion returns false (strict mode)", () => {
    const ctx = new TypeInferenceContext()
    expect(ctx.unify(i32, f32)).toBe(false)
  })

  test("unify with TypeVar looks up bindings (strict mode - no coercion)", () => {
    const ctx = new TypeInferenceContext()
    ctx.bind("T", i32)
    const tv = new TypeVar("T")
    expect(ctx.unify(tv, i32)).toBe(true)
    expect(ctx.unify(tv, i64)).toBe(false)
  })

  test("unify with unbound TypeVar returns false", () => {
    const ctx = new TypeInferenceContext()
    const tv = new TypeVar("X")
    expect(ctx.unify(tv, i32)).toBe(false)
  })

  test("clear removes all bindings", () => {
    const ctx = new TypeInferenceContext()
    ctx.bind("x", i32)
    ctx.bind("y", f64)
    ctx.clear()
    expect(ctx.lookup("x")).toBe(null)
    expect(ctx.lookup("y")).toBe(null)
  })
})

describe("inferType", () => {
  test("bigint literals infer as i32", () => {
    const ctx = new TypeInferenceContext()
    expect(inferType(ctx, 42n)).toBe(i32)
    expect(inferType(ctx, 1000000000n)).toBe(i32)
  })

  test("integer numbers infer as i32", () => {
    const ctx = new TypeInferenceContext()
    expect(inferType(ctx, 42)).toBe(i32)
    expect(inferType(ctx, -100)).toBe(i32)
    expect(inferType(ctx, 0)).toBe(i32)
  })

  test("floating point numbers infer as f64", () => {
    const ctx = new TypeInferenceContext()
    expect(inferType(ctx, 3.14)).toBe(f64)
    expect(inferType(ctx, -0.5)).toBe(f64)
    expect(inferType(ctx, 1.1)).toBe(f64)
  })

  test("whole numbers infer as i32 even with decimal notation", () => {
    const ctx = new TypeInferenceContext()
    expect(inferType(ctx, 1.0)).toBe(i32)
    expect(inferType(ctx, 42.0)).toBe(i32)
  })

  test("strings infer as stringType", () => {
    const ctx = new TypeInferenceContext()
    expect(inferType(ctx, "hello")).toBe(stringType)
    expect(inferType(ctx, "")).toBe(stringType)
  })

  test("empty array infers as i32 array", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(i32)
      expect(result.dimensions).toEqual([])
    } else {
      throw new Error("Expected array type")
    }
  })

  test("array of integers infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1, 2, 3])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(i32)
      expect(result.dimensions).toEqual([3])
    } else {
      throw new Error("Expected array type")
    }
  })

  test("array of floats infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1.5, 2.5, 3.5])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(f64)
      expect(result.dimensions).toEqual([3])
    } else {
      throw new Error("Expected array type")
    }
  })

  test("array of bigints infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1n, 2n, 3n])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(i32)
      expect(result.dimensions).toEqual([3])
    } else {
      throw new Error("Expected array type")
    }
  })

  test("array of strings infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, ["a", "b", "c"])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(stringType)
      expect(result.dimensions).toEqual([3])
    } else {
      throw new Error("Expected array type")
    }
  })

  test("nested arrays inferred correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [
      [1, 2],
      [3, 4],
    ])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBeInstanceOf(ArrayType)
      expect(result.dimensions).toEqual([2])
    } else {
      throw new Error("Expected array type")
    }
  })

  test("array of integers infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1, 2, 3])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(i32)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("array of floats infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1.5, 2.5, 3.5])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(f64)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("array of bigints infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1n, 2n, 3n])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(i32)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("array of strings infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, ["a", "b", "c"])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBe(stringType)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("nested arrays inferred correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [
      [1, 2],
      [3, 4],
    ])
    if (result && isArrayType(result)) {
      expect(result.elementType).toBeInstanceOf(ArrayType)
      expect(result.dimensions).toEqual([2])
    }
  })

  test("array of floats infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1.5, 2.5, 3.5])
    expect(isArrayType(result)).toBe(true)
    if (result) {
      expect(result.elementType).toBe(f64)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("array of bigints infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [1n, 2n, 3n])
    expect(isArrayType(result)).toBe(true)
    if (result) {
      expect(result.elementType).toBe(i32)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("array of strings infers element type correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, ["a", "b", "c"])
    expect(isArrayType(result)).toBe(true)
    if (result) {
      expect(result.elementType).toBe(stringType)
      expect(result.dimensions).toEqual([3])
    }
  })

  test("nested arrays inferred correctly", () => {
    const ctx = new TypeInferenceContext()
    const result = inferType(ctx, [
      [1, 2],
      [3, 4],
    ])
    expect(isArrayType(result)).toBe(true)
    if (result) {
      expect(result.elementType).toBeInstanceOf(ArrayType)
      expect(result.dimensions).toEqual([2])
    }
  })

  test("unknown types return null", () => {
    const ctx = new TypeInferenceContext()
    expect(inferType(ctx, true)).toBe(null)
    expect(inferType(ctx, null)).toBe(null)
    expect(inferType(ctx, undefined)).toBe(null)
    expect(inferType(ctx, {})).toBe(null)
  })
})

describe("toString", () => {
  test("primitive types have correct string representation", () => {
    expect(i32.toString()).toBe("i32")
    expect(u64.toString()).toBe("u64")
    expect(f128.toString()).toBe("f128")
  })

  test("array types have correct string representation", () => {
    expect(array(i32, []).toString()).toBe("[i32]")
    expect(array(i32, [10]).toString()).toBe("[i32[10]]")
    expect(array(i32, [3, 4]).toString()).toBe("[i32[3][4]]")
  })

  test("map types have correct string representation", () => {
    expect(map(i32, f64).toString()).toBe("{i32: f64}")
    expect(map(u8, i32).toString()).toBe("{u8: i32}")
  })

  test("void type has correct string representation", () => {
    expect(voidType.toString()).toBe("void")
  })

  test("TypeVar has correct string representation", () => {
    const tv = new TypeVar("T")
    expect(tv.toString()).toBe("T")
  })
})

describe("Edge Cases and Type Compatibility", () => {
  test("maximum bit sizes handle correctly", () => {
    expect(i256.bits).toBe(256)
    expect(u256.bits).toBe(256)
    expect(f256.bits).toBe(256)
    expect(canCoerce(i128, i256)).toBe(false)
    expect(canCoerce(f128, f256)).toBe(false)
  })

  test("minimum bit sizes handle correctly", () => {
    expect(i8.bits).toBe(8)
    expect(u8.bits).toBe(8)
    expect(f8.bits).toBe(8)
    expect(canCoerce(i16, i8)).toBe(false)
    expect(canCoerce(f16, f8)).toBe(false)
  })

  test("coercion chain from smallest to largest (strict mode - no coercion)", () => {
    expect(canCoerce(i8, i16)).toBe(false)
    expect(canCoerce(i16, i32)).toBe(false)
    expect(canCoerce(i32, i64)).toBe(false)
    expect(canCoerce(i64, i128)).toBe(false)
    expect(canCoerce(i128, i256)).toBe(false)
  })

  test("common type across bit size spectrum", () => {
    expect(getCommonType(i8, i256)).toBe(i256)
    expect(getCommonType(u8, u256)).toBe(u256)
    expect(getCommonType(f8, f256)).toBe(f256)
  })

  test("boundary value validations for all integer types", () => {
    expect(checkBounds(i8, -128n)).toBe(true)
    expect(checkBounds(i8, 127n)).toBe(true)
    expect(checkBounds(i16, -32768n)).toBe(true)
    expect(checkBounds(i16, 32767n)).toBe(true)
    expect(checkBounds(u8, 0n)).toBe(true)
    expect(checkBounds(u8, 255n)).toBe(true)
    expect(checkBounds(u16, 65535n)).toBe(true)
  })

  test("boundary value just outside limits fails", () => {
    expect(checkBounds(i8, -129n)).toBe(false)
    expect(checkBounds(i8, 128n)).toBe(false)
    expect(checkBounds(u8, 256n)).toBe(false)
  })

  test("same type coercion success for all types", () => {
    expect(canCoerce(i8, i8)).toBe(true)
    expect(canCoerce(i256, i256)).toBe(true)
    expect(canCoerce(u8, u8)).toBe(true)
    expect(canCoerce(u256, u256)).toBe(true)
    expect(canCoerce(f8, f8)).toBe(true)
    expect(canCoerce(f256, f256)).toBe(true)
  })

  test("mixed numeric type coercion failure", () => {
    expect(canCoerce(i32, u32)).toBe(false)
    expect(canCoerce(u32, i32)).toBe(false)
    expect(canCoerce(f32, i32)).toBe(false)
    expect(canCoerce(i32, u16)).toBe(false)
  })

  test("array dimensions preservation in sameType", () => {
    const arr1 = array(i32, [1, 2, 3])
    const arr2 = array(i32, [1, 2, 3])
    const arr3 = array(i32, [3, 2, 1])
    expect(sameType(arr1, arr2)).toBe(true)
    expect(sameType(arr1, arr3)).toBe(false)
  })

  test("array rank affects type equality", () => {
    const arr1 = array(i32, [])
    const arr2 = array(i32, [10])
    const arr3 = array(i32, [10, 20])
    expect(sameType(arr1, arr2)).toBe(false)
    expect(sameType(arr2, arr3)).toBe(false)
  })
})

describe("StructType", () => {
  test("StructType constructor creates type with fields", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(pointType.name).toBe("Point")
    expect(pointType.fields.size).toBe(2)
    expect(pointType.fields.get("x")).toBe(f64)
    expect(pointType.fields.get("y")).toBe(f64)
  })

  test("StructType with mixed field types", () => {
    const particleType = new types.StructType("Particle", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 },
      { name: "mass", type: f64 },
      { name: "id", type: i32 },
      { name: "active", type: new types.UnsignedType("u8") }
    ]))
    expect(particleType.fields.size).toBe(5)
    expect(particleType.fields.get("id")).toBe(i32)
  })

  test("StructType toString", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(pointType.toString()).toBe("struct Point { x: f64, y: f64 }")
  })

  test("StructType with empty fields", () => {
    const emptyType = new types.StructType("Empty", new Map())
    expect(emptyType.fields.size).toBe(0)
    expect(emptyType.toString()).toBe("struct Empty {  }")
  })
})

describe("AOSType (Array of Structs)", () => {
  test("AOSType constructor creates dynamic array type", () => {
    const particleType = new types.StructType("Particle", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosType = new types.AOSType(particleType)
    expect(aosType.elementType).toBe(particleType)
    expect(aosType.size).toBeNull()
  })

  test("AOSType constructor with fixed capacity", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosType = new types.AOSType(pointType, 100)
    expect(aosType.elementType).toBe(pointType)
    expect(aosType.size).toBe(100)
  })

  test("AOSType toString", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosDynamic = new types.AOSType(pointType)
    const aosFixed = new types.AOSType(pointType, 50)
    expect(aosDynamic.toString()).toBe("[Point]")
    expect(aosFixed.toString()).toBe("[Point; 50]")
  })
})

describe("SOAType (Struct of Arrays) - new design", () => {
  test("SOAType constructor creates SoA type from StructType", () => {
    const structType = new types.StructType("Datum", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const soaType = new types.SOAType(structType, 100)
    expect(soaType.structType).toBe(structType)
    expect(soaType.capacity).toBe(100)
    expect(soaType.fields.size).toBe(2)
    expect(soaType.fields.get("x")).toEqual(f64)
  })

  test("SOAType with null capacity (dynamic)", () => {
    const structType = new types.StructType("Datum", createFieldMap([
      { name: "id", type: i64 },
      { name: "val", type: i64 }
    ]))
    const soaType = new types.SOAType(structType)
    expect(soaType.capacity).toBeNull()
  })

  test("SOAType toString with capacity", () => {
    const soaType = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    expect(soaType.toString()).toBe("soa Datum[100]")
  })

  test("SOAType toString without capacity", () => {
    const soaType = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ])
    expect(soaType.toString()).toBe("soa Datum[]")
  })

  test("SOAType name delegates to structType", () => {
    const soaType = createSOAType("Particle", [
      { name: "x", type: f64 },
      { name: "y", type: f64 },
      { name: "vx", type: f64 },
      { name: "vy", type: f64 },
      { name: "id", type: i32 }
    ], 1000)
    expect(soaType.name).toBe("Particle")
    expect(soaType.fields.size).toBe(5)
    expect(soaType.fields.has("id")).toBe(true)
  })
})

describe("isStructType type guard", () => {
  test("returns true for StructType", () => {
    const structType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(types.isStructType(structType)).toBe(true)
  })

  test("returns false for non-struct types", () => {
    expect(types.isStructType(i32)).toBe(false)
    expect(types.isStructType(f64)).toBe(false)
    expect(types.isStructType(array(i32, []))).toBe(false)
    expect(types.isStructType(map(i32, f64))).toBe(false)
  })
})

describe("isAOSType type guard", () => {
  test("returns true for AOSType", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosType = new types.AOSType(pointType)
    expect(types.isAOSType(aosType)).toBe(true)
  })

  test("returns false for non-AoS types", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(types.isAOSType(i32)).toBe(false)
    expect(types.isAOSType(f64)).toBe(false)
    expect(types.isAOSType(array(i32, []))).toBe(false)
    expect(types.isAOSType(pointType)).toBe(false)
  })
})

describe("isSOAType type guard", () => {
  test("returns true for SOAType", () => {
    const soaType = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    expect(types.isSOAType(soaType)).toBe(true)
  })

  test("returns false for non-SoA types", () => {
    expect(types.isSOAType(i32)).toBe(false)
    expect(types.isSOAType(f64)).toBe(false)
    expect(types.isSOAType(array(i32, []))).toBe(false)
  })
})

describe("Struct type equality", () => {
  test("same struct type is equal to itself", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(sameType(pointType, pointType)).toBe(true)
  })

  test("struct types with same fields are equal", () => {
    const point1 = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const point2 = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(sameType(point1, point2)).toBe(true)
  })

  test("struct types with different names are not equal", () => {
    const point = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const vec2 = new types.StructType("Vec2", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(sameType(point, vec2)).toBe(false)
  })

  test("struct types with different fields are not equal", () => {
    const point2d = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const point3d = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 },
      { name: "z", type: f64 }
    ]))
    expect(sameType(point2d, point3d)).toBe(false)
  })
})

describe("AoS type equality", () => {
  test("same AoS types are equal", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aos1 = new types.AOSType(pointType)
    const aos2 = new types.AOSType(pointType)
    expect(sameType(aos1, aos2)).toBe(true)
  })

  test("AoS types with different capacities are not equal", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aos1 = new types.AOSType(pointType, 100)
    const aos2 = new types.AOSType(pointType, 200)
    expect(sameType(aos1, aos2)).toBe(false)
  })

  test("dynamic and fixed AoS types are not equal", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosDynamic = new types.AOSType(pointType)
    const aosFixed = new types.AOSType(pointType, 100)
    expect(sameType(aosDynamic, aosFixed)).toBe(false)
  })
})

describe("SoA type equality", () => {
  test("same SoA types are equal", () => {
    const soa1 = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    const soa2 = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    expect(sameType(soa1, soa2)).toBe(true)
  })

  test("SoA types with different backing structs are not equal", () => {
    const soa1 = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    const soa2 = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 },
      { name: "z", type: f64 }
    ], 100)
    expect(sameType(soa1, soa2)).toBe(false)
  })

  test("SoA types with different capacities are not equal", () => {
    const soa1 = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    const soa2 = createSOAType("Datum", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 200)
    expect(sameType(soa1, soa2)).toBe(false)
  })
})

describe("Data structure type compatibility", () => {
  test("Map types are not compatible with Struct types", () => {
    const mapType = map(i32, f64)
    const structType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    expect(sameType(mapType, structType)).toBe(false)
    expect(types.compatibleTypes(mapType, structType)).toBe(false)
  })

  test("Struct types are not compatible with AoS types", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosType = new types.AOSType(pointType)
    expect(sameType(pointType, aosType)).toBe(false)
    expect(types.compatibleTypes(pointType, aosType)).toBe(false)
  })

  test("AoS types are not compatible with SoA types", () => {
    const pointType = new types.StructType("Point", createFieldMap([
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ]))
    const aosType = new types.AOSType(pointType)
    const soaType = createSOAType("Point", [
      { name: "x", type: f64 },
      { name: "y", type: f64 }
    ], 100)
    expect(sameType(aosType, soaType)).toBe(false)
    expect(types.compatibleTypes(aosType, soaType)).toBe(false)
  })
})
