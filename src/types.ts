type IntegerKind = "i8" | "i16" | "i32" | "i64" | "i128" | "i256"

type UnsignedKind = "u8" | "u16" | "u32" | "u64" | "u128" | "u256"

type FloatKind = "f8" | "f16" | "f32" | "f64" | "f128" | "f256"

type PrimitiveKind = IntegerKind | UnsignedKind | FloatKind

class IntegerType {
  kind: IntegerKind
  type = "IntegerType"

  constructor(kind: IntegerKind) {
    this.kind = kind
  }

  get bits(): number {
    return parseInt(this.kind.slice(1))
  }

  isSigned(): boolean {
    return true
  }

  get minValue(): bigint {
    return -((1n as bigint) << (BigInt(this.bits) - 1n))
  }

  get maxValue(): bigint {
    return ((1n as bigint) << (BigInt(this.bits) - 1n)) - 1n
  }

  toString(): string {
    return this.kind
  }
}

class UnsignedType {
  kind: UnsignedKind
  type = "UnsignedType"

  constructor(kind: UnsignedKind) {
    this.kind = kind
  }

  get bits(): number {
    return parseInt(this.kind.slice(1))
  }

  isSigned(): boolean {
    return false
  }

  get minValue(): bigint {
    return 0n
  }

  get maxValue(): bigint {
    return ((1n as bigint) << BigInt(this.bits)) - 1n
  }

  toString(): string {
    return this.kind
  }
}

class FloatType {
  kind: FloatKind
  type = "FloatType"

  constructor(kind: FloatKind) {
    this.kind = kind
  }

  get bits(): number {
    return parseInt(this.kind.slice(1))
  }

  toString(): string {
    return this.kind
  }
}

class ArrayType {
  elementType: Type
  dimensions: number[]
  type = "ArrayType"

  constructor(elementType: Type, dimensions: number[] = []) {
    this.elementType = elementType
    this.dimensions = dimensions
  }

  get rank(): number {
    return this.dimensions.length
  }

  get isString(): boolean {
    return this.elementType instanceof UnsignedType && this.elementType.kind === "u8" && this.rank === 1
  }

  toString(): string {
    const dims = this.rank > 0 ? this.dimensions.join("][") : ""
    return `[${this.elementType}${dims ? `[${dims}]` : ""}]`
  }
}

class TableType {
  keyType: Type
  valueType: Type
  type = "TableType"

  constructor(keyType: Type, valueType: Type) {
    this.keyType = keyType
    this.valueType = valueType
  }

  toString(): string {
    return `{${this.keyType}: ${this.valueType}}`
  }
}

class PointerType {
  type = "PointerType"
  elementType: Type | null = null

  constructor(elementType: Type | null = null) {
    this.elementType = elementType
  }

  toString(): string {
    return this.elementType ? `ptr<${this.elementType}>` : "ptr"
  }
}

class VoidType {
  type = "VoidType"

  toString(): string {
    return "void"
  }
}

type Type = IntegerType | UnsignedType | FloatType | ArrayType | TableType | PointerType | VoidType

const i8 = new IntegerType("i8")
const i16 = new IntegerType("i16")
const i32 = new IntegerType("i32")
const i64 = new IntegerType("i64")
const i128 = new IntegerType("i128")
const i256 = new IntegerType("i256")

const u8 = new UnsignedType("u8")
const u16 = new UnsignedType("u16")
const u32 = new UnsignedType("u32")
const u64 = new UnsignedType("u64")
const u128 = new UnsignedType("u128")
const u256 = new UnsignedType("u256")

const f8 = new FloatType("f8")
const f16 = new FloatType("f16")
const f32 = new FloatType("f32")
const f64 = new FloatType("f64")
const f128 = new FloatType("f128")
const f256 = new FloatType("f256")

const voidType = new VoidType()

function array(elementType: Type, dimensions: number[] = []): ArrayType {
  return new ArrayType(elementType, dimensions)
}

function table(keyType: Type, valueType: Type): TableType {
  return new TableType(keyType, valueType)
}

const stringType = array(u8, [])

function isIntegerType(type: Type): type is IntegerType {
  return type instanceof IntegerType
}

function isUnsignedType(type: Type): type is UnsignedType {
  return type instanceof UnsignedType
}

function isFloatType(type: Type): type is FloatType {
  return type instanceof FloatType
}

function isArrayType(type: Type): type is ArrayType {
  return type instanceof ArrayType
}

function isTableType(type: Type): type is TableType {
  return type instanceof TableType
}

function isVoidType(type: Type): type is VoidType {
  return type instanceof VoidType
}

function isPointerType(type: Type): type is PointerType {
  return type instanceof PointerType
}

function isNumericType(type: Type): boolean {
  return isIntegerType(type) || isUnsignedType(type) || isFloatType(type)
}

function isSigned(type: IntegerType | UnsignedType): boolean {
  return type.isSigned()
}

function sameType(a: Type, b: Type): boolean {
  if (a instanceof IntegerType && b instanceof IntegerType) return a.kind === b.kind
  if (a instanceof UnsignedType && b instanceof UnsignedType) return a.kind === b.kind
  if (a instanceof FloatType && b instanceof FloatType) return a.kind === b.kind
  if (a instanceof ArrayType && b instanceof ArrayType) {
    if (a.rank !== b.rank) return false
    if (!sameType(a.elementType, b.elementType)) return false
    for (let i = 0; i < a.rank; i++) {
      if (a.dimensions[i] !== b.dimensions[i]) return false
    }
    return true
  }
  if (a instanceof TableType && b instanceof TableType) {
    return sameType(a.keyType, b.keyType) && sameType(a.valueType, b.valueType)
  }
  if (a instanceof VoidType && b instanceof VoidType) return true
  if (a instanceof PointerType && b instanceof PointerType) return true
  return false
}

function compatibleTypes(from: Type, to: Type): boolean {
  if (sameType(from, to)) return true

  if (from instanceof ArrayType && to instanceof ArrayType) {
    // For array compatibility, check if element types are compatible
    // and ranks match (allowing target to be unbounded)
    if (from.rank !== to.rank && to.rank !== 0) {
      return false
    }
    
    // For nested arrays, we need to compare element types recursively
    // but allow dimensions to differ (e.g., [i64[3]] vs [i64[]])
    return compatibleTypes(from.elementType, to.elementType)
  }

  return false
}

function canCoerce(from: Type, to: Type): boolean {
  return compatibleTypes(from, to)
}

// For literal assignment: allows float widening (f32 literal -> f64 variable)
function canCoerceWithWidening(from: Type, to: Type): boolean {
  if (compatibleTypes(from, to)) return true

  // Allow float widening (e.g., f32 -> f64)
  if (isFloatType(from) && isFloatType(to)) {
    return from.bits <= to.bits
  }

  return false
}

function getCommonType(a: Type, b: Type): Type | null {
  if (sameType(a, b)) return a

  if (isVoidType(a)) return b
  if (isVoidType(b)) return a

  if (!isNumericType(a) || !isNumericType(b)) return null

  const aNum = a as IntegerType | UnsignedType | FloatType
  const bNum = b as IntegerType | UnsignedType | FloatType

  if (isFloatType(aNum) && isFloatType(bNum)) {
    return aNum.bits >= bNum.bits ? aNum : bNum
  }
  if (isIntegerType(aNum) && isIntegerType(bNum)) {
    return aNum.bits >= bNum.bits ? aNum : bNum
  }
  if (isUnsignedType(aNum) && isUnsignedType(bNum)) {
    return aNum.bits >= bNum.bits ? aNum : bNum
  }
  if ((isIntegerType(aNum) || isUnsignedType(aNum)) && isFloatType(bNum)) {
    return bNum
  }
  if (isFloatType(aNum) && (isIntegerType(bNum) || isUnsignedType(bNum))) {
    return aNum
  }
  if (isIntegerType(aNum) && isUnsignedType(bNum)) {
    const biggerKind =
      aNum.bits >= bNum.bits ? (aNum.kind as IntegerKind) : (bNum.kind.replace("u", "i") as IntegerKind)
    return new IntegerType(biggerKind)
  }
  if (isUnsignedType(aNum) && isIntegerType(bNum)) {
    const biggerKind =
      bNum.bits >= aNum.bits ? (bNum.kind as IntegerKind) : (aNum.kind.replace("u", "i") as IntegerKind)
    return new IntegerType(biggerKind)
  }

  return null
}

function checkBounds(type: IntegerType | UnsignedType, value: bigint): boolean {
  return value >= type.minValue && value <= type.maxValue
}

type TypeConstraint = {
  type: Type
  nullable?: boolean
}

class TypeVar {
  name: string
  constraints: TypeConstraint[]

  constructor(name: string, constraints: TypeConstraint[] = []) {
    this.name = name
    this.constraints = constraints
  }

  satisfies(type: Type): boolean {
    if (this.constraints.length === 0) return true
    return this.constraints.some((c) => canCoerce(type, c.type))
  }

  toString(): string {
    return this.name
  }
}

class TypeInferenceContext {
  private bindings: Map<string, Type>

  constructor() {
    this.bindings = new Map()
  }

  bind(varName: string, type: Type): void {
    this.bindings.set(varName, type)
  }

  lookup(varName: string): Type | null {
    return this.bindings.get(varName) ?? null
  }

  infer(expr: { type?: Type | null }): Type | null {
    return expr.type ?? null
  }

  unify(a: Type | TypeVar, b: Type | TypeVar): boolean {
    const typeA = a instanceof TypeVar ? this.lookup(a.name) : a
    const typeB = b instanceof TypeVar ? this.lookup(b.name) : b

    if (!typeA || !typeB) return false
    if (sameType(typeA, typeB)) return true
    return canCoerce(typeA, typeB) || canCoerce(typeB, typeA)
  }

  clear(): void {
    this.bindings.clear()
  }
}

function inferType(context: TypeInferenceContext, expr: unknown): Type | null {
  if (typeof expr === "bigint") {
    return i32
  }
  if (typeof expr === "number") {
    if (Number.isInteger(expr)) return i32
    return f64
  }
  if (typeof expr === "string") {
    return stringType
  }
  if (Array.isArray(expr)) {
    if (expr.length === 0) return array(i32, [])

    const elementType = inferType(context, expr[0])
    if (!elementType) return null

    return array(elementType, [expr.length])
  }
  return null
}

export type { Type, PrimitiveKind, IntegerKind, UnsignedKind, FloatKind, TypeConstraint }
export {
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
  TableType,
  PointerType,
  VoidType,
  TypeVar,
  TypeInferenceContext,
  array,
  table,
  isIntegerType,
  isUnsignedType,
  isFloatType,
  isArrayType,
  isTableType,
  isVoidType,
  isPointerType,
  isNumericType,
  isSigned,
  sameType,
  compatibleTypes,
  canCoerce,
  canCoerceWithWidening,
  getCommonType,
  checkBounds,
  inferType,
}
