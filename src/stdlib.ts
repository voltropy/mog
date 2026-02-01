import { array, NDArray } from "./arrays.ts"
import {
  t,
  keys,
  values,
  merge,
  has,
  type Table,
  type TableKey,
  type TableValue,
  isTable,
  isTableKey,
} from "./tables.ts"

type PrimitiveValue = bigint | number | boolean
type Value = PrimitiveValue | NDArray | Table | null

const MATH_PI = Math.PI
const MATH_E = Math.E

const MathOps = {
  sin(x: number): number {
    if (typeof x !== "number") throw new Error("sin expects a number")
    return Math.sin(x)
  },

  cos(x: number): number {
    if (typeof x !== "number") throw new Error("cos expects a number")
    return Math.cos(x)
  },

  tan(x: number): number {
    if (typeof x !== "number") throw new Error("tan expects a number")
    return Math.tan(x)
  },

  exp(x: number): number {
    if (typeof x !== "number") throw new Error("exp expects a number")
    return Math.exp(x)
  },

  log(x: number): number {
    if (typeof x !== "number") throw new Error("log expects a number")
    if (x <= 0) throw new Error("log expects a positive number")
    return Math.log(x)
  },

  log10(x: number): number {
    if (typeof x !== "number") throw new Error("log10 expects a number")
    if (x <= 0) throw new Error("log10 expects a positive number")
    return Math.log10(x)
  },

  sqrt(x: number): number {
    if (typeof x !== "number") throw new Error("sqrt expects a number")
    if (x < 0) throw new Error("sqrt expects a non-negative number")
    return Math.sqrt(x)
  },

  abs(x: number): number {
    if (typeof x !== "number") throw new Error("abs expects a number")
    return Math.abs(x)
  },

  min(...args: number[]): number {
    if (args.length === 0) throw new Error("min requires at least one argument")
    if (args.some((v) => typeof v !== "number")) throw new Error("min expects all numbers")
    return Math.min(...args)
  },

  max(...args: number[]): number {
    if (args.length === 0) throw new Error("max requires at least one argument")
    if (args.some((v) => typeof v !== "number")) throw new Error("max expects all numbers")
    return Math.max(...args)
  },

  floor(x: number): number {
    if (typeof x !== "number") throw new Error("floor expects a number")
    return Math.floor(x)
  },

  ceil(x: number): number {
    if (typeof x !== "number") throw new Error("ceil expects a number")
    return Math.ceil(x)
  },

  round(x: number): number {
    if (typeof x !== "number") throw new Error("round expects a number")
    return Math.round(x)
  },

  pow(base: number, exp: number): number {
    if (typeof base !== "number" || typeof exp !== "number") throw new Error("pow expects two numbers")
    return Math.pow(base, exp)
  },

  random(): number {
    return Math.random()
  },

  acos(x: number): number {
    if (typeof x !== "number") throw new Error("acos expects a number")
    if (x < -1 || x > 1) throw new Error("acos expects a number between -1 and 1")
    return Math.acos(x)
  },

  asin(x: number): number {
    if (typeof x !== "number") throw new Error("asin expects a number")
    if (x < -1 || x > 1) throw new Error("asin expects a number between -1 and 1")
    return Math.asin(x)
  },

  atan(x: number): number {
    if (typeof x !== "number") throw new Error("atan expects a number")
    return Math.atan(x)
  },

  atan2(y: number, x: number): number {
    if (typeof y !== "number" || typeof x !== "number") throw new Error("atan2 expects two numbers")
    return Math.atan2(y, x)
  },
}

const StringOps = {
  length(str: string): number {
    if (typeof str !== "string") throw new Error("length expects a string")
    return str.length
  },

  substring(str: string, start: number, end?: number): string {
    if (typeof str !== "string") throw new Error("substring expects a string as first argument")
    if (typeof start !== "number") throw new Error("substring expects a number as second argument")
    if (end !== undefined && typeof end !== "number") throw new Error("substring expects end to be a number")
    return str.substring(start, end)
  },

  concat(...strings: string[]): string {
    if (strings.some((s) => typeof s !== "string")) throw new Error("concat expects all strings")
    return strings.join("")
  },

  split(str: string, separator: string): string[] {
    if (typeof str !== "string") throw new Error("split expects a string as first argument")
    if (typeof separator !== "string") throw new Error("split expects a string separator")
    return str.split(separator)
  },

  toLowerCase(str: string): string {
    if (typeof str !== "string") throw new Error("toLowerCase expects a string")
    return str.toLowerCase()
  },

  toUpperCase(str: string): string {
    if (typeof str !== "string") throw new Error("toUpperCase expects a string")
    return str.toUpperCase()
  },

  trim(str: string): string {
    if (typeof str !== "string") throw new Error("trim expects a string")
    return str.trim()
  },

  charAt(str: string, index: number): string {
    if (typeof str !== "string") throw new Error("charAt expects a string as first argument")
    if (typeof index !== "number") throw new Error("charAt expects a number as second argument")
    return str.charAt(index)
  },

  indexOf(str: string, search: string, from?: number): number {
    if (typeof str !== "string") throw new Error("indexOf expects a string as first argument")
    if (typeof search !== "string") throw new Error("indexOf expects a string as second argument")
    if (from !== undefined && typeof from !== "number") throw new Error("indexOf expects from to be a number")
    return str.indexOf(search, from)
  },

  replace(str: string, search: string, replacement: string): string {
    if (typeof str !== "string") throw new Error("replace expects a string as first argument")
    if (typeof search !== "string") throw new Error("replace expects a string as second argument")
    if (typeof replacement !== "string") throw new Error("replace expects a string as third argument")
    return str.replace(search, replacement)
  },

  startsWith(str: string, prefix: string): boolean {
    if (typeof str !== "string") throw new Error("startsWith expects a string as first argument")
    if (typeof prefix !== "string") throw new Error("startsWith expects a string as second argument")
    return str.startsWith(prefix)
  },

  endsWith(str: string, suffix: string): boolean {
    if (typeof str !== "string") throw new Error("endsWith expects a string as first argument")
    if (typeof suffix !== "string") throw new Error("endsWith expects a string as second argument")
    return str.endsWith(suffix)
  },

  includes(str: string, search: string): boolean {
    if (typeof str !== "string") throw new Error("includes expects a string as first argument")
    if (typeof search !== "string") throw new Error("includes expects a string as second argument")
    return str.includes(search)
  },
}

function toArrayValues(arr: NDArray): number[] {
  const values: number[] = []
  for (let i = 0; i < arr.size; i++) values.push(arr.get([i]))
  return values
}

function fromArrayValues(values: number[]): NDArray {
  return array(...values)
}

const ArrayOps = {
  length(arr: NDArray | unknown[]): number {
    if (arr instanceof NDArray) {
      return arr.shape[0] ?? arr.size
    }
    if (Array.isArray(arr)) {
      return arr.length
    }
    throw new Error("length expects an array")
  },

  push(arr: NDArray | unknown[], ...items: unknown[]): NDArray | unknown[] {
    if (Array.isArray(arr) && items.every((x) => typeof x === "number")) {
      for (const item of items) arr.push(item)
      return arr
    }
    if (arr instanceof NDArray) {
      const currentValues = toArrayValues(arr)
      const newValues = [...currentValues, ...(items as number[])]
      return fromArrayValues(newValues)
    }
    throw new Error("push expects an array and number values")
  },

  pop(arr: NDArray | unknown[]): unknown {
    if (Array.isArray(arr)) {
      return arr.pop()
    }
    if (arr instanceof NDArray) {
      if (arr.size === 0) throw new Error("Cannot pop from empty array")
      const values = toArrayValues(arr)
      const popped = values[values.length - 1]
      return popped
    }
    throw new Error("pop expects an array")
  },

  reverse(arr: NDArray | unknown[]): NDArray | unknown[] {
    if (Array.isArray(arr)) {
      return arr.reverse()
    }
    if (arr instanceof NDArray) {
      const values = toArrayValues(arr).reverse()
      return fromArrayValues(values)
    }
    throw new Error("reverse expects an array")
  },

  sort(arr: NDArray | unknown[]): NDArray | unknown[] {
    if (Array.isArray(arr) && arr.every((x) => typeof x === "number")) {
      return arr.sort((a, b) => (a as number) - (b as number))
    }
    if (arr instanceof NDArray) {
      const values = toArrayValues(arr).sort((a, b) => a - b)
      return fromArrayValues(values)
    }
    throw new Error("sort expects an array of numbers")
  },

  first(arr: NDArray | unknown[]): unknown {
    if (Array.isArray(arr)) {
      if (arr.length === 0) throw new Error("Cannot get first element from empty array")
      return arr[0]
    }
    if (arr instanceof NDArray) {
      if (arr.size === 0) throw new Error("Cannot get first element from empty array")
      return arr.get([0])
    }
    throw new Error("first expects an array")
  },

  last(arr: NDArray | unknown[]): unknown {
    if (Array.isArray(arr)) {
      if (arr.length === 0) throw new Error("Cannot get last element from empty array")
      return arr[arr.length - 1]
    }
    if (arr instanceof NDArray) {
      if (arr.size === 0) throw new Error("Cannot get last element from empty array")
      return arr.get([arr.size - 1])
    }
    throw new Error("last expects an array")
  },

  slice(arr: NDArray | unknown[], start: number, end?: number): unknown[] {
    if (Array.isArray(arr)) {
      return arr.slice(start, end)
    }
    if (arr instanceof NDArray) {
      const values = toArrayValues(arr)
      return values.slice(start, end)
    }
    throw new Error("slice expects an array")
  },

  indexOf(arr: NDArray | unknown[], value: unknown): number {
    if (Array.isArray(arr)) {
      return arr.indexOf(value)
    }
    if (arr instanceof NDArray && typeof value === "number") {
      const values = toArrayValues(arr)
      return values.indexOf(value)
    }
    throw new Error("indexOf expects an array")
  },
}

const TableOps = {
  hasKey(table: Table, key: TableKey): boolean {
    if (!isTable(table)) throw new Error("hasKey expects a table as first argument")
    if (!isTableKey(key)) throw new Error("hasKey expects a string or number key")
    return has(table, key)
  },

  getKeys(table: Table): TableKey[] {
    if (!isTable(table)) throw new Error("keys expects a table")
    return keys(table)
  },

  getValues(table: Table): TableValue[] {
    if (!isTable(table)) throw new Error("values expects a table")
    return values(table)
  },

  mergeTables(a: Table, b: Table): Table {
    if (!isTable(a) || !isTable(b)) throw new Error("merge expects two tables")
    return merge(a, b)
  },

  clear(table: Table): void {
    if (!isTable(table)) throw new Error("clear expects a table")
    table.clear()
  },

  size(table: Table): number {
    if (!isTable(table)) throw new Error("size expects a table")
    return table.size
  },

  clone(table: Table): Table {
    if (!isTable(table)) throw new Error("clone expects a table")
    return new Map(table)
  },

  setEntry(table: Table, key: TableKey, value: TableValue): void {
    if (!isTable(table)) throw new Error("set expects a table as first argument")
    if (!isTableKey(key)) throw new Error("set expects a string or number key")
    table.set(key, value)
  },

  getEntry(table: Table, key: TableKey): TableValue | undefined {
    if (!isTable(table)) throw new Error("get expects a table as first argument")
    if (!isTableKey(key)) throw new Error("get expects a string or number key")
    return table.get(key)
  },

  deleteEntry(table: Table, key: TableKey): boolean {
    if (!isTable(table)) throw new Error("delete expects a table as first argument")
    if (!isTableKey(key)) throw new Error("delete expects a string or number key")
    return table.delete(key)
  },
}

const TypeOps = {
  typeof(value: unknown): string {
    if (value === null) return "null"
    if (value instanceof NDArray) return "array"
    if (isTable(value)) return "table"
    if (typeof value === "bigint") return "integer"
    if (typeof value === "number") {
      return Number.isInteger(value) ? "integer" : "float"
    }
    if (typeof value === "boolean") return "boolean"
    if (typeof value === "string") return "string"
    return typeof value
  },

  isNull(value: unknown): boolean {
    return value === null
  },

  isNumber(value: unknown): boolean {
    return typeof value === "number"
  },

  isInteger(value: unknown): boolean {
    return typeof value === "bigint" || (typeof value === "number" && Number.isInteger(value))
  },

  isFloat(value: unknown): boolean {
    return typeof value === "number" && !Number.isInteger(value)
  },

  isBoolean(value: unknown): boolean {
    return typeof value === "boolean"
  },

  isString(value: unknown): boolean {
    return typeof value === "string"
  },

  isArray(value: unknown): boolean {
    return value instanceof NDArray || Array.isArray(value)
  },

  isTable(value: unknown): boolean {
    return isTable(value)
  },

  typecast<T>(value: unknown, type: "number" | "string" | "boolean" | "array" | "table"): T {
    switch (type) {
      case "number":
        if (typeof value === "number") return value as T
        if (typeof value === "bigint") return Number(value) as T
        if (typeof value === "string") {
          const num = Number(value)
          if (isNaN(num)) throw new Error(`Cannot cast string "${value}" to number`)
          return num as T
        }
        if (typeof value === "boolean") return (value ? 1 : 0) as T
        throw new Error(`Cannot cast ${TypeOps.typeof(value)} to number`)

      case "string":
        if (typeof value === "string") return value as T
        if (typeof value === "number" || typeof value === "bigint") return String(value) as T
        if (typeof value === "boolean") return String(value) as T
        throw new Error(`Cannot cast ${TypeOps.typeof(value)} to string`)

      case "boolean":
        if (typeof value === "boolean") return value as T
        if (typeof value === "number") return (value !== 0) as T
        if (typeof value === "bigint") return (value !== 0n) as T
        if (typeof value === "string") {
          if (value === "true" || value === "1") return true as T
          if (value === "false" || value === "0") return false as T
          throw new Error(`Cannot cast string "${value}" to boolean`)
        }
        throw new Error(`Cannot cast ${TypeOps.typeof(value)} to boolean`)

      case "array":
        if (value instanceof NDArray) return value as T
        if (Array.isArray(value)) return value as T
        throw new Error(`Cannot cast ${TypeOps.typeof(value)} to array`)

      case "table":
        if (isTable(value)) return value as T
        throw new Error(`Cannot cast ${TypeOps.typeof(value)} to table`)

      default:
        throw new Error(`Invalid type: ${type}`)
    }
  },
}

const ConversionOps = {
  toString(value: unknown): string {
    if (value === null) return "null"
    if (value instanceof NDArray) return `[${toArrayValues(value).join(", ")}]`
    if (isTable(value)) {
      const entries = Array.from(value.entries())
        .map(([k, v]) => `${JSON.stringify(k)}: ${JSON.stringify(v)}`)
        .join(", ")
      return `{${entries}}`
    }
    return String(value)
  },

  toNumber(value: unknown): number {
    if (typeof value === "number") return value
    if (typeof value === "bigint") return Number(value)
    if (typeof value === "string") {
      const num = Number(value)
      if (isNaN(num)) throw new Error(`Cannot convert string "${value}" to number`)
      return num
    }
    if (typeof value === "boolean") return value ? 1 : 0
    throw new Error(`Cannot convert ${TypeOps.typeof(value)} to number`)
  },

  toBoolean(value: unknown): boolean {
    if (typeof value === "boolean") return value
    if (typeof value === "number" || typeof value === "bigint") return value !== 0
    if (typeof value === "string") {
      return value !== "" && value !== "0" && value !== "false"
    }
    return value !== null
  },
}

export const stdlib = {
  math: MathOps,
  string: StringOps,
  array: ArrayOps,
  table: TableOps,
  type: TypeOps,
  convert: ConversionOps,
  constants: { PI: MATH_PI, E: MATH_E },
}

export const {
  sin,
  cos,
  tan,
  exp,
  log,
  log10,
  sqrt,
  abs,
  min,
  max,
  floor,
  ceil,
  round,
  pow,
  random,
  acos,
  asin,
  atan,
  atan2,
} = MathOps

export const {
  length: stringLength,
  substring,
  concat: stringConcat,
  split,
  toLowerCase,
  toUpperCase,
  trim,
  charAt,
  indexOf: stringIndexOf,
  replace,
  startsWith,
  endsWith,
  includes,
} = StringOps

export const { length: arrayLength, push, pop, reverse, sort, first, last, slice, indexOf: arrayIndexOf } = ArrayOps

export const {
  hasKey,
  getKeys,
  getValues,
  mergeTables,
  clear,
  size: tableSize,
  clone: tableClone,
  setEntry,
  getEntry,
  deleteEntry,
} = TableOps

export const {
  typeof: typeofOp,
  isNull,
  isNumber,
  isInteger,
  isFloat,
  isBoolean,
  isString,
  isArray,
  isTable: isTableOp,
  typecast,
} = TypeOps

export const { toString, toNumber, toBoolean } = ConversionOps

export const { PI, E } = { PI: MATH_PI, E: MATH_E }
