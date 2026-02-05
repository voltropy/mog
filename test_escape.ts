import { Parser } from "./src/parser.ts"

const code = `request := "GET /get HTTP/1.1\\r\\nHost: httpbin.org\\r\\nConnection: close\\r\\n\\r\\n"`
const parser = new Parser([] as any)
parser["tokens"] = [] as any
parser["position"] = 0
const ast = (parser as any).parseStringLiteral()

console.log("Raw value:", ast.value)
console.log("Value length:", ast.value.length)
console.log("Value chars:", [...ast.value].map(c => `${c}(${c.charCodeAt(0)})`).join(", "))
