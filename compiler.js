// src/posix_constants.ts
var POSIX_CONSTANTS = {
  O_RDONLY: 0,
  O_WRONLY: 1,
  O_RDWR: 2,
  O_CREAT: 64,
  O_EXCL: 128,
  O_NOCTTY: 256,
  O_TRUNC: 512,
  O_APPEND: 1024,
  O_NONBLOCK: 2048,
  O_SYNC: 4096,
  O_ASYNC: 8192,
  O_DIRECTORY: 65536,
  O_NOFOLLOW: 131072,
  O_CLOEXEC: 524288,
  F_OK: 0,
  R_OK: 4,
  W_OK: 2,
  X_OK: 1,
  S_IRUSR: 256,
  S_IWUSR: 128,
  S_IXUSR: 64,
  S_IRGRP: 32,
  S_IWGRP: 16,
  S_IXGRP: 8,
  S_IROTH: 4,
  S_IWOTH: 2,
  S_IXOTH: 1,
  S_IRWXU: 448,
  S_IRWXG: 56,
  S_IRWXO: 7,
  S_IFMT: 61440,
  S_IFIFO: 4096,
  S_IFCHR: 8192,
  S_IFDIR: 16384,
  S_IFBLK: 24576,
  S_IFREG: 32768,
  S_IFLNK: 40960,
  S_IFSOCK: 49152,
  SEEK_SET: 0,
  SEEK_CUR: 1,
  SEEK_END: 2,
  EPERM: 1,
  ENOENT: 2,
  ESRCH: 3,
  EINTR: 4,
  EIO: 5,
  ENXIO: 6,
  E2BIG: 7,
  ENOEXEC: 8,
  EBADF: 9,
  ECHILD: 10,
  EAGAIN: 11,
  EWOULDBLOCK: 11,
  ENOMEM: 12,
  EACCES: 13,
  EFAULT: 14,
  ENOTBLK: 15,
  EBUSY: 16,
  EEXIST: 17,
  EXDEV: 18,
  ENODEV: 19,
  ENOTDIR: 20,
  EISDIR: 21,
  EINVAL: 22,
  ENFILE: 23,
  EMFILE: 24,
  ENOTTY: 25,
  ETXTBSY: 26,
  EFBIG: 27,
  ENOSPC: 28,
  ESPIPE: 29,
  EROFS: 30,
  EMLINK: 31,
  EPIPE: 32,
  EDOM: 33,
  ERANGE: 34,
  FD_CLOEXEC: 1,
  AT_FDCWD: -100,
  AT_SYMLINK_NOFOLLOW: 256,
  AT_REMOVEDIR: 512,
  UTIME_NOW: -2,
  UTIME_OMIT: -1
};
function isPOSIXConstant(name) {
  return name in POSIX_CONSTANTS;
}
function getPOSIXConstant(name) {
  if (name in POSIX_CONSTANTS) {
    return POSIX_CONSTANTS[name];
  }
  throw new Error(`Unknown POSIX constant: ${name}`);
}

// src/lexer.ts
class Lexer {
  input;
  pos = 0;
  line = 1;
  column = 1;
  constructor(input) {
    this.input = input;
  }
  currentPosition() {
    return { line: this.line, column: this.column, index: this.pos };
  }
  peek(offset = 0) {
    return this.input[this.pos + offset] ?? "";
  }
  advance(n = 1) {
    for (let i2 = 0;i2 < n; i2++) {
      if (this.peek() === `
`) {
        this.line++;
        this.column = 1;
      } else if (this.peek() !== "") {
        this.column++;
      }
      this.pos++;
    }
  }
  match(pattern) {
    pattern.lastIndex = this.pos;
    const match = pattern.exec(this.input);
    return match ? match[0] : null;
  }
  tokenize() {
    const tokens = [];
    const whitespaceRegex = /\s+/y;
    const commentRegex = /#.*|\/\/.*/y;
    const fnRegex = /fn\b/y;
    const returnRegex = /return\b/y;
    const ifRegex = /if\b/y;
    const elseRegex = /else\b/y;
    const whileRegex = /while\b/y;
    const forRegex = /for\b/y;
    const toRegex = /to\b/y;
    const castRegex = /cast\b/y;
    const notRegex = /not\b/y;
    const llmRegex = /LLM\b/y;
    const notEqualRegex = /!=/y;
    const equalEqualRegex = /==/y;
    const assignRegex = /:=/y;
    const lessEqualRegex = /<=/y;
    const greaterEqualRegex = />=/y;
    const arrowRegex = /->/y;
    const plusRegex = /\+/y;
    const minusRegex = /-/y;
    const timesRegex = /\*/y;
    const divideRegex = /\//y;
    const moduloRegex = /%/y;
    const bitwiseAndRegex = /&/y;
    const bitwiseOrRegex = /\|/y;
    const lessRegex = /</y;
    const greaterRegex = />/y;
    const equalRegex = /=/y;
    const lbracketRegex = /\[/y;
    const rbracketRegex = /\]/y;
    const lbraceRegex = /\{/y;
    const rbraceRegex = /\}/y;
    const lparenRegex = /\(/y;
    const rparenRegex = /\)/y;
    const commaRegex = /,/y;
    const colonRegex = /:/y;
    const semicolonRegex = /;/y;
    const dotRegex = /\./y;
    const doubleStringRegex = /"(?:[^"\\]|\\.)*"/y;
    const singleStringRegex = /'(?:[^'\\]|\\.)*'/y;
    const numberRegex = /\b\d+(?:\.\d*)?(?:[eE][+-]?\d+)?\b/y;
    const typeRegex = /\b(?:i|u|f)(?:8|16|32|64|128|256)((?:\[\])*)/y;
    const identifierRegex = /\b[a-zA-Z_][a-zA-Z0-9_]*\b/y;
    while (this.pos < this.input.length) {
      const startPos = this.currentPosition();
      let value = null;
      let type;
      value = this.match(whitespaceRegex);
      if (value) {
        type = "WHITESPACE";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(commentRegex);
      if (value) {
        type = "COMMENT";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(fnRegex);
      if (value) {
        type = "fn";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(returnRegex);
      if (value) {
        type = "return";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(ifRegex);
      if (value) {
        type = "if";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(elseRegex);
      if (value) {
        type = "else";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(whileRegex);
      if (value) {
        type = "while";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(forRegex);
      if (value) {
        type = "for";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(toRegex);
      if (value) {
        type = "to";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(castRegex);
      if (value) {
        type = "cast";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(notRegex);
      if (value) {
        type = "not";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(llmRegex);
      if (value) {
        type = "LLM";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(notEqualRegex);
      if (value) {
        type = "NOT_EQUAL";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(equalEqualRegex);
      if (value) {
        type = "EQUAL";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(assignRegex);
      if (value) {
        type = "ASSIGN";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(arrowRegex);
      if (value) {
        type = "ARROW";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(lessEqualRegex);
      if (value) {
        type = "LESS_EQUAL";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(greaterEqualRegex);
      if (value) {
        type = "GREATER_EQUAL";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(plusRegex);
      if (value) {
        type = "PLUS";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(minusRegex);
      if (value) {
        type = "MINUS";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(timesRegex);
      if (value) {
        type = "TIMES";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(divideRegex);
      if (value) {
        type = "DIVIDE";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(moduloRegex);
      if (value) {
        type = "MODULO";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(bitwiseAndRegex);
      if (value) {
        type = "BITWISE_AND";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(bitwiseOrRegex);
      if (value) {
        type = "BITWISE_OR";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(lessRegex);
      if (value) {
        type = "LESS";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(greaterRegex);
      if (value) {
        type = "GREATER";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(equalRegex);
      if (value) {
        type = "EQUAL";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(lbracketRegex);
      if (value) {
        type = "LBRACKET";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(rbracketRegex);
      if (value) {
        type = "RBRACKET";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(lbraceRegex);
      if (value) {
        type = "LBRACE";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(rbraceRegex);
      if (value) {
        type = "RBRACE";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(lparenRegex);
      if (value) {
        type = "LPAREN";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(rparenRegex);
      if (value) {
        type = "RPAREN";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(commaRegex);
      if (value) {
        type = "COMMA";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(colonRegex);
      if (value) {
        type = "COLON";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(semicolonRegex);
      if (value) {
        type = "SEMICOLON";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(dotRegex);
      if (value) {
        type = "DOT";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(doubleStringRegex);
      if (value) {
        type = "STRING";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(singleStringRegex);
      if (value) {
        type = "STRING";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(typeRegex);
      if (value) {
        type = "TYPE";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(numberRegex);
      if (value) {
        type = "NUMBER";
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      value = this.match(identifierRegex);
      if (value) {
        if (isPOSIXConstant(value)) {
          type = "POSIX_CONSTANT";
        } else {
          type = "IDENTIFIER";
        }
        this.advance(value.length);
        tokens.push({
          type,
          value,
          position: { start: startPos, end: this.currentPosition() }
        });
        continue;
      }
      tokens.push({
        type: "UNKNOWN",
        value: this.peek(),
        position: { start: startPos, end: this.currentPosition() }
      });
      this.advance(1);
    }
    return tokens;
  }
}
function tokenize(input) {
  const lexer = new Lexer(input);
  return lexer.tokenize();
}

// src/types.ts
class IntegerType {
  kind;
  constructor(kind) {
    this.kind = kind;
  }
  get bits() {
    return parseInt(this.kind.slice(1));
  }
  isSigned() {
    return true;
  }
  get minValue() {
    return -(1n << BigInt(this.bits) - 1n);
  }
  get maxValue() {
    return (1n << BigInt(this.bits) - 1n) - 1n;
  }
  toString() {
    return this.kind;
  }
}

class UnsignedType {
  kind;
  constructor(kind) {
    this.kind = kind;
  }
  get bits() {
    return parseInt(this.kind.slice(1));
  }
  isSigned() {
    return false;
  }
  get minValue() {
    return 0n;
  }
  get maxValue() {
    return (1n << BigInt(this.bits)) - 1n;
  }
  toString() {
    return this.kind;
  }
}

class FloatType {
  kind;
  constructor(kind) {
    this.kind = kind;
  }
  get bits() {
    return parseInt(this.kind.slice(1));
  }
  toString() {
    return this.kind;
  }
}

class ArrayType {
  elementType;
  dimensions;
  constructor(elementType, dimensions = []) {
    this.elementType = elementType;
    this.dimensions = dimensions;
  }
  get rank() {
    return this.dimensions.length;
  }
  get isString() {
    return this.elementType instanceof UnsignedType && this.elementType.kind === "u8" && this.rank === 1;
  }
  toString() {
    const dims = this.rank > 0 ? this.dimensions.join("][") : "";
    return `[${this.elementType}${dims ? `[${dims}]` : ""}]`;
  }
}

class TableType {
  keyType;
  valueType;
  constructor(keyType, valueType) {
    this.keyType = keyType;
    this.valueType = valueType;
  }
  toString() {
    return `{${this.keyType}: ${this.valueType}}`;
  }
}

class VoidType {
  toString() {
    return "void";
  }
}
var i8 = new IntegerType("i8");
var i16 = new IntegerType("i16");
var i32 = new IntegerType("i32");
var i64 = new IntegerType("i64");
var i128 = new IntegerType("i128");
var i256 = new IntegerType("i256");
var u8 = new UnsignedType("u8");
var u16 = new UnsignedType("u16");
var u32 = new UnsignedType("u32");
var u64 = new UnsignedType("u64");
var u128 = new UnsignedType("u128");
var u256 = new UnsignedType("u256");
var f8 = new FloatType("f8");
var f16 = new FloatType("f16");
var f32 = new FloatType("f32");
var f64 = new FloatType("f64");
var f128 = new FloatType("f128");
var f256 = new FloatType("f256");
var voidType = new VoidType;
function array(elementType, dimensions = []) {
  return new ArrayType(elementType, dimensions);
}
var stringType = array(u8, []);
function isIntegerType(type) {
  return type instanceof IntegerType;
}
function isUnsignedType(type) {
  return type instanceof UnsignedType;
}
function isFloatType(type) {
  return type instanceof FloatType;
}
function isArrayType(type) {
  return type instanceof ArrayType;
}
function isTableType(type) {
  return type instanceof TableType;
}
function isVoidType(type) {
  return type instanceof VoidType;
}
function isNumericType(type) {
  return isIntegerType(type) || isUnsignedType(type) || isFloatType(type);
}
function sameType(a, b) {
  if (a instanceof IntegerType && b instanceof IntegerType)
    return a.kind === b.kind;
  if (a instanceof UnsignedType && b instanceof UnsignedType)
    return a.kind === b.kind;
  if (a instanceof FloatType && b instanceof FloatType)
    return a.kind === b.kind;
  if (a instanceof ArrayType && b instanceof ArrayType) {
    if (a.rank !== b.rank)
      return false;
    if (!sameType(a.elementType, b.elementType))
      return false;
    for (let i2 = 0;i2 < a.rank; i2++) {
      if (a.dimensions[i2] !== b.dimensions[i2])
        return false;
    }
    return true;
  }
  if (a instanceof TableType && b instanceof TableType) {
    return sameType(a.keyType, b.keyType) && sameType(a.valueType, b.valueType);
  }
  if (a instanceof VoidType && b instanceof VoidType)
    return true;
  return false;
}
function getCommonType(a, b) {
  if (sameType(a, b))
    return a;
  if (isVoidType(a))
    return b;
  if (isVoidType(b))
    return a;
  if (!isNumericType(a) || !isNumericType(b))
    return null;
  const aNum = a;
  const bNum = b;
  if (isFloatType(aNum) && isFloatType(bNum)) {
    return aNum.bits >= bNum.bits ? aNum : bNum;
  }
  if (isIntegerType(aNum) && isIntegerType(bNum)) {
    return aNum.bits >= bNum.bits ? aNum : bNum;
  }
  if (isUnsignedType(aNum) && isUnsignedType(bNum)) {
    return aNum.bits >= bNum.bits ? aNum : bNum;
  }
  if ((isIntegerType(aNum) || isUnsignedType(aNum)) && isFloatType(bNum)) {
    return bNum;
  }
  if (isFloatType(aNum) && (isIntegerType(bNum) || isUnsignedType(bNum))) {
    return aNum;
  }
  if (isIntegerType(aNum) && isUnsignedType(bNum)) {
    const biggerKind = aNum.bits >= bNum.bits ? aNum.kind : bNum.kind.replace("u", "i");
    return new IntegerType(biggerKind);
  }
  if (isUnsignedType(aNum) && isIntegerType(bNum)) {
    const biggerKind = bNum.bits >= aNum.bits ? bNum.kind : aNum.kind.replace("u", "i");
    return new IntegerType(biggerKind);
  }
  return null;
}

// src/parser.ts
class Parser {
  tokens;
  current = 0;
  constructor(tokens) {
    this.tokens = tokens;
  }
  parse() {
    const statements = [];
    while (!this.isAtEnd()) {
      const stmt = this.statement();
      if (stmt) {
        statements.push(stmt);
      }
    }
    const unwrappedStatements = this.getUnwrappedStatements(statements);
    return {
      type: "Program",
      statements: unwrappedStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  getUnwrappedStatements(statements) {
    if (statements.length === 1 && statements[0].type === "Block") {
      const block = statements[0];
      if (block.statements.length === 1) {
        return block.statements;
      }
    }
    return statements;
  }
  statement() {
    while (this.matchType("SEMICOLON")) {}
    if (this.checkType("RBRACE") || this.isAtEnd()) {
      return null;
    }
    if (this.matchType("LBRACE")) {
      return this.blockStatement();
    }
    if (this.matchType("fn")) {
      return this.functionDeclaration();
    }
    if (this.matchType("return")) {
      return this.returnStatement();
    }
    if (this.matchType("if")) {
      return this.ifStatement();
    }
    if (this.matchType("while")) {
      return this.whileStatement();
    }
    if (this.matchType("for")) {
      return this.forStatement();
    }
    if (this.checkType("VARIABLE") || this.checkType("TYPE") || this.checkType("IDENTIFIER") && this.peekNext()?.type === "COLON") {
      return this.variableDeclaration();
    }
    const expr = this.expression();
    this.matchType("SEMICOLON");
    return {
      type: "ExpressionStatement",
      expression: expr,
      position: expr.position
    };
  }
  functionDeclaration() {
    const name = this.consume("IDENTIFIER", "Expected function name").value;
    this.consume("LPAREN", "Expected ( after function name");
    const params = [];
    if (!this.checkType("RPAREN")) {
      do {
        const paramName = this.consume("IDENTIFIER", "Expected parameter name").value;
        this.consume("COLON", "Expected : after parameter name");
        let paramToken = this.consume("TYPE", "Expected parameter type");
        let typeName = paramToken.value;
        while (this.matchType("LBRACKET")) {
          typeName += "[";
          this.consume("RBRACKET", "Expected ] after array bracket");
          typeName += "]";
        }
        const paramType = this.parseType(typeName);
        params.push({ name: paramName, paramType });
      } while (this.matchType("COMMA"));
    }
    this.consume("RPAREN", "Expected ) after parameters");
    this.consume("ARROW", "Expected -> after parameter list");
    let returnToken = this.consume("TYPE", "Expected return type");
    let returnTypeName = returnToken.value;
    while (this.matchType("LBRACKET")) {
      returnTypeName += "[";
      this.consume("RBRACKET", "Expected ] after array bracket");
      returnTypeName += "]";
    }
    const returnType = this.parseType(returnTypeName);
    this.consume("LBRACE", "Expected { after function signature");
    const bodyStatements = [];
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement();
      if (stmt) {
        bodyStatements.push(stmt);
      }
    }
    this.consume("RBRACE", "Expected } after function body");
    const body = {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
    return {
      type: "FunctionDeclaration",
      name,
      params,
      returnType,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  returnStatement() {
    let value = null;
    if (!this.checkType("SEMICOLON")) {
      value = this.expression();
    }
    this.consume("SEMICOLON", "Expected ; after return");
    return {
      type: "Return",
      value,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  ifStatement() {
    this.consume("LPAREN", "Expected ( after if");
    const condition = this.expression();
    this.consume("RPAREN", "Expected ) after condition");
    this.consume("LBRACE", "Expected { after if condition");
    const trueBranchStatements = [];
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement();
      if (stmt) {
        trueBranchStatements.push(stmt);
      }
    }
    this.consume("RBRACE", "Expected } after if body");
    const trueBranch = {
      type: "Block",
      statements: trueBranchStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
    let falseBranch = null;
    if (this.matchType("else")) {
      this.consume("LBRACE", "Expected { after else");
      const falseBranchStatements = [];
      while (!this.checkType("RBRACE") && !this.isAtEnd()) {
        const stmt = this.statement();
        if (stmt) {
          falseBranchStatements.push(stmt);
        }
      }
      this.consume("RBRACE", "Expected } after else body");
      falseBranch = {
        type: "Block",
        statements: falseBranchStatements,
        position: {
          start: { line: 1, column: 1, index: 0 },
          end: this.lastPosition()
        }
      };
    }
    return {
      type: "Conditional",
      condition,
      trueBranch,
      falseBranch,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  whileStatement() {
    this.consume("LPAREN", "Expected ( after while");
    const test = this.expression();
    this.consume("RPAREN", "Expected ) after while condition");
    this.consume("LBRACE", "Expected { after while condition");
    const bodyStatements = [];
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement();
      if (stmt) {
        bodyStatements.push(stmt);
      }
    }
    this.consume("RBRACE", "Expected } after while loop");
    const body = {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
    return {
      type: "WhileLoop",
      test,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  forStatement() {
    const variable = this.consume("IDENTIFIER", "Expected variable name after for").value;
    this.consume("ASSIGN", "Expected := after variable name");
    const start = this.expression();
    this.consume("to", "Expected to after start value");
    const end = this.expression();
    this.consume("LBRACE", "Expected { after for header");
    const bodyStatements = [];
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement();
      if (stmt) {
        bodyStatements.push(stmt);
      }
    }
    this.consume("RBRACE", "Expected } after for loop");
    const body = {
      type: "Block",
      statements: bodyStatements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
    return {
      type: "ForLoop",
      variable,
      start,
      end,
      body,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  blockStatement() {
    const statements = [];
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const stmt = this.statement();
      if (stmt) {
        statements.push(stmt);
      }
    }
    this.consume("RBRACE", "Expected } after block");
    return {
      type: "Block",
      statements,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  variableDeclaration() {
    const name = this.consume("IDENTIFIER", "Expected variable name").value;
    this.consume("COLON", "Expected : after variable name");
    let typeToken = this.consume("TYPE", "Expected type annotation");
    let typeName = typeToken.value;
    while (this.matchType("LBRACKET")) {
      typeName += "[";
      this.consume("RBRACKET", "Expected ] after array bracket");
      typeName += "]";
    }
    const varType = this.parseType(typeName);
    this.consume("EQUAL", "Expected = after type");
    this.matchType("ASSIGN");
    const value = this.expression();
    this.consume("SEMICOLON", "Expected ; after variable declaration");
    return {
      type: "VariableDeclaration",
      name,
      varType,
      value,
      position: {
        start: { line: 1, column: 1, index: 0 },
        end: this.lastPosition()
      }
    };
  }
  expression() {
    return this.assignment();
  }
  assignment() {
    const expr = this.conditional();
    if (this.matchType("ASSIGN")) {
      const value = this.assignment();
      if (expr.type === "Identifier") {
        return {
          type: "AssignmentExpression",
          name: expr.name,
          value,
          position: this.combinePositions(expr, value)
        };
      }
      if (expr.type === "IndexExpression") {
        return {
          type: "AssignmentExpression",
          target: expr,
          value,
          position: this.combinePositions(expr, value)
        };
      }
      throw new Error("Invalid assignment target");
    }
    return expr;
  }
  conditional() {
    const expr = this.logicalOr();
    if (this.matchType("QUESTION")) {
      const thenExpr = this.expression();
      this.consume("COLON", "Expected : in conditional expression");
      const elseExpr = this.expression();
      return {
        type: "Conditional",
        condition: expr,
        trueBranch: thenExpr,
        falseBranch: elseExpr,
        position: this.combinePositions(expr, elseExpr)
      };
    }
    return expr;
  }
  logicalOr() {
    let expr = this.logicalAnd();
    while (this.matchType("BITWISE_OR")) {
      const operator = this.previous();
      const right = this.logicalAnd();
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right)
      };
    }
    return expr;
  }
  logicalAnd() {
    let expr = this.equality();
    while (this.matchType("BITWISE_AND")) {
      const operator = this.previous();
      const right = this.equality();
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right)
      };
    }
    return expr;
  }
  equality() {
    let expr = this.comparison();
    while (this.matchType("EQUAL") || this.matchType("NOT_EQUAL")) {
      const operator = this.previous();
      const right = this.comparison();
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right)
      };
    }
    return expr;
  }
  comparison() {
    let expr = this.additive();
    while (this.matchType("LESS") || this.matchType("LESS_EQUAL") || this.matchType("GREATER") || this.matchType("GREATER_EQUAL")) {
      const operator = this.previous();
      const right = this.additive();
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right)
      };
    }
    return expr;
  }
  additive() {
    let expr = this.multiplicative();
    while (this.matchType("PLUS") || this.matchType("MINUS")) {
      const operator = this.previous();
      const right = this.multiplicative();
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right)
      };
    }
    return expr;
  }
  multiplicative() {
    let expr = this.unary();
    while (this.matchType("TIMES") || this.matchType("DIVIDE") || this.matchType("MODULO")) {
      const operator = this.previous();
      const right = this.unary();
      expr = {
        type: "BinaryExpression",
        left: expr,
        operator: operator.value,
        right,
        position: this.combinePositions(expr, right)
      };
    }
    return expr;
  }
  unary() {
    if (this.matchType("MINUS") || this.matchType("BANG") || this.matchType("not")) {
      const operator = this.previous();
      const argument = this.unary();
      return {
        type: "UnaryExpression",
        operator: operator.value,
        argument,
        position: this.combinePositions({ position: operator.position }, argument)
      };
    }
    return this.primary();
  }
  primary() {
    if (this.matchType("FALSE")) {
      const token2 = this.previous();
      return {
        type: "BooleanLiteral",
        value: false,
        position: token2.position
      };
    }
    if (this.matchType("TRUE")) {
      const token2 = this.previous();
      return {
        type: "BooleanLiteral",
        value: true,
        position: token2.position
      };
    }
    if (this.matchType("NUMBER")) {
      const token2 = this.previous();
      return {
        type: "NumberLiteral",
        value: parseFloat(token2.value),
        position: token2.position
      };
    }
    if (this.matchType("STRING")) {
      const token2 = this.previous();
      return {
        type: "StringLiteral",
        value: token2.value.slice(1, -1),
        position: token2.position
      };
    }
    if (this.matchType("POSIX_CONSTANT")) {
      const token2 = this.previous();
      return {
        type: "POSIXConstant",
        value: token2.value,
        position: token2.position
      };
    }
    if (this.matchType("IDENTIFIER")) {
      const token2 = this.previous();
      let object = {
        type: "Identifier",
        name: token2.value,
        position: token2.position
      };
      while (this.matchType("LBRACKET")) {
        const index = this.expression();
        this.consume("RBRACKET", "Expected ] after index");
        object = {
          type: "IndexExpression",
          object,
          index,
          position: this.combinePositions(object, index)
        };
      }
      if (this.matchType("LPAREN")) {
        const callArgs = [];
        if (!this.checkType("RPAREN")) {
          do {
            callArgs.push(this.expression());
          } while (this.matchType("COMMA"));
        }
        this.consume("RPAREN", "Expected ) after arguments");
        return {
          type: "CallExpression",
          callee: object.type === "Identifier" ? object : { type: "Identifier", name: object.name, position: token2.position },
          args: callArgs,
          position: this.combinePositions(object, this.previous())
        };
      }
      if (this.matchType("DOT")) {
        const property = this.consume("IDENTIFIER", "Expected property name").value;
        return {
          type: "MemberExpression",
          object: object.type === "Identifier" ? object : { type: "Identifier", name: object.name, position: token2.position },
          property,
          position: this.combinePositions(object, this.previous())
        };
      }
      return object;
    }
    if (this.matchType("LBRACKET")) {
      return this.arrayLiteral();
    }
    if (this.matchType("LBRACE")) {
      return this.tableLiteral();
    }
    if (this.matchType("LPAREN")) {
      const expr = this.expression();
      this.consume("RPAREN", "Expected ) after expression");
      return expr;
    }
    const token = this.peek();
    throw new Error(`Unexpected token: ${token.type} at line ${token.position.start.line}`);
  }
  arrayLiteral() {
    const start = this.previous().position.start;
    const elements = [];
    if (!this.checkType("RBRACKET")) {
      do {
        elements.push(this.expression());
      } while (this.matchType("COMMA"));
    }
    this.consume("RBRACKET", "Expected ] after array elements");
    return {
      type: "ArrayLiteral",
      elements,
      position: {
        start,
        end: this.lastPosition()
      }
    };
  }
  parseArrayType() {
    let dimensions = 0;
    while (this.matchType("LBRACKET")) {
      dimensions++;
      this.consume("RBRACKET", "Expected ] after array dimension");
    }
    return dimensions;
  }
  tableLiteral() {
    const start = this.previous().position.start;
    const columns = [];
    while (!this.checkType("RBRACE") && !this.isAtEnd()) {
      const key = this.consume("IDENTIFIER", "Expected key in table literal");
      this.consume("COLON", "Expected : after key");
      const values = [];
      values.push(this.expression());
      columns.push({ name: key.value, values, columnType: null });
      this.matchType("COMMA");
    }
    this.consume("RBRACE", "Expected } after table literal");
    return {
      type: "TableLiteral",
      columns,
      position: {
        start,
        end: this.lastPosition()
      }
    };
  }
  parseType(typeName) {
    const bracketMatch = typeName.match(/^(.*?)(\[\]*)$/);
    if (bracketMatch) {
      const baseName = bracketMatch[1];
      const arraySuffix = bracketMatch[2];
      let elementType;
      if (baseName.startsWith("i")) {
        elementType = new IntegerType(baseName);
      } else if (baseName.startsWith("u")) {
        elementType = new UnsignedType(baseName);
      } else if (baseName.startsWith("f")) {
        elementType = new FloatType(baseName);
      } else {
        return new VoidType;
      }
      const dimensions = [];
      let currentType = elementType;
      const bracketCount = (arraySuffix.match(/\[/g) || []).length;
      for (let i2 = 0;i2 < bracketCount; i2++) {
        currentType = new ArrayType(currentType, []);
      }
      return currentType;
    }
    if (typeName.startsWith("i")) {
      return new IntegerType(typeName);
    }
    if (typeName.startsWith("u")) {
      return new UnsignedType(typeName);
    }
    if (typeName.startsWith("f")) {
      return new FloatType(typeName);
    }
    return new VoidType;
  }
  consume(type, message) {
    if (this.checkType(type)) {
      return this.advance();
    }
    const token = this.peek();
    throw new Error(`${message} at line ${token.position.start.line}`);
  }
  checkType(type) {
    if (this.isAtEnd())
      return false;
    return this.peek().type === type;
  }
  skipWhitespace() {}
  matchType(type) {
    if (this.checkType(type)) {
      this.advance();
      return true;
    }
    return false;
  }
  advance(steps = 1) {
    this.current += steps;
    return this.tokens[this.current - steps];
  }
  previous() {
    return this.tokens[this.current - 1];
  }
  peek() {
    return this.tokens[this.current];
  }
  peekNext() {
    if (this.current + 1 >= this.tokens.length)
      return null;
    return this.tokens[this.current + 1];
  }
  isAtEnd() {
    return this.current >= this.tokens.length;
  }
  lastPosition() {
    if (this.tokens.length > 0) {
      return this.tokens[this.tokens.length - 1].position.end;
    }
    return { line: 1, column: 1, index: 0 };
  }
  combinePositions(a, b) {
    return {
      start: a.position.start,
      end: b.position.end
    };
  }
}
function parseTokens(tokens) {
  const parser = new Parser(tokens);
  return parser.parse();
}

// src/analyzer.ts
class SymbolTable {
  stack = [];
  depth = 0;
  constructor() {
    this.pushScope();
  }
  pushScope() {
    this.stack.push(new Map);
    this.depth++;
  }
  popScope() {
    if (this.stack.length > 1) {
      this.stack.pop();
      this.depth--;
    }
  }
  declare(name, symbolType, declaredType = null) {
    const currentScope = this.stack[this.stack.length - 1];
    if (currentScope.has(name)) {
      return;
    }
    currentScope.set(name, {
      name,
      symbolType,
      declaredType,
      inferredType: null,
      depth: this.depth
    });
  }
  lookup(name) {
    for (let i2 = this.stack.length - 1;i2 >= 0; i2--) {
      const symbol = this.stack[i2].get(name);
      if (symbol) {
        return symbol;
      }
    }
    return null;
  }
  setCurrentType(name, type) {
    for (let i2 = this.stack.length - 1;i2 >= 0; i2--) {
      const symbol = this.stack[i2].get(name);
      if (symbol) {
        symbol.inferredType = type;
        return;
      }
    }
  }
  getCurrentDepth() {
    return this.depth;
  }
}

class SemanticAnalyzer {
  symbolTable;
  errors = [];
  currentFunction = null;
  constructor() {
    this.symbolTable = new SymbolTable;
  }
  analyze(program) {
    this.symbolTable = new SymbolTable;
    this.errors = [];
    this.declarePOSIXBuiltins();
    this.visitProgram(program);
    return this.errors;
  }
  declarePOSIXBuiltins() {
    const i64Type = new IntegerType("i64");
    const voidType2 = new VoidType;
    const posixFunctions = {
      open: { params: [{ name: "path", type: "i64" }, { name: "flags", type: "i64" }], returnType: i64Type },
      read: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }], returnType: i64Type },
      write: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }], returnType: i64Type },
      pread: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }, { name: "offset", type: "i64" }], returnType: i64Type },
      pwrite: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }, { name: "count", type: "i64" }, { name: "offset", type: "i64" }], returnType: i64Type },
      lseek: { params: [{ name: "fd", type: "i64" }, { name: "offset", type: "i64" }, { name: "whence", type: "i64" }], returnType: i64Type },
      close: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      fsync: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      fdatasync: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      stat: { params: [{ name: "path", type: "i64" }, { name: "buf", type: "i64" }], returnType: i64Type },
      lstat: { params: [{ name: "path", type: "i64" }, { name: "buf", type: "i64" }], returnType: i64Type },
      fstat: { params: [{ name: "fd", type: "i64" }, { name: "buf", type: "i64" }], returnType: i64Type },
      access: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      faccessat: { params: [{ name: "dirfd", type: "i64" }, { name: "path", type: "i64" }, { name: "mode", type: "i64" }, { name: "flags", type: "i64" }], returnType: i64Type },
      utimes: { params: [{ name: "path", type: "i64" }, { name: "times", type: "i64" }], returnType: i64Type },
      futimes: { params: [{ name: "fd", type: "i64" }, { name: "times", type: "i64" }], returnType: i64Type },
      utimensat: { params: [{ name: "dirfd", type: "i64" }, { name: "path", type: "i64" }, { name: "times", type: "i64" }, { name: "flags", type: "i64" }], returnType: i64Type },
      chmod: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      fchmod: { params: [{ name: "fd", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      chown: { params: [{ name: "path", type: "i64" }, { name: "owner", type: "i64" }, { name: "group", type: "i64" }], returnType: i64Type },
      fchown: { params: [{ name: "fd", type: "i64" }, { name: "owner", type: "i64" }, { name: "group", type: "i64" }], returnType: i64Type },
      umask: { params: [{ name: "mask", type: "i64" }], returnType: i64Type },
      truncate: { params: [{ name: "path", type: "i64" }, { name: "length", type: "i64" }], returnType: i64Type },
      ftruncate: { params: [{ name: "fd", type: "i64" }, { name: "length", type: "i64" }], returnType: i64Type },
      link: { params: [{ name: "oldpath", type: "i64" }, { name: "newpath", type: "i64" }], returnType: i64Type },
      symlink: { params: [{ name: "target", type: "i64" }, { name: "linkpath", type: "i64" }], returnType: i64Type },
      readlink: { params: [{ name: "path", type: "i64" }, { name: "buf", type: "i64" }, { name: "bufsiz", type: "i64" }], returnType: i64Type },
      rename: { params: [{ name: "oldpath", type: "i64" }, { name: "newpath", type: "i64" }], returnType: i64Type },
      unlink: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      mkdir: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      rmdir: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      fcntl: { params: [{ name: "fd", type: "i64" }, { name: "cmd", type: "i64" }], returnType: i64Type },
      pathconf: { params: [{ name: "path", type: "i64" }, { name: "name", type: "i64" }], returnType: i64Type },
      fpathconf: { params: [{ name: "fd", type: "i64" }, { name: "name", type: "i64" }], returnType: i64Type },
      dup: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      dup2: { params: [{ name: "fd", type: "i64" }, { name: "fd2", type: "i64" }], returnType: i64Type },
      creat: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      mkfifo: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }], returnType: i64Type },
      mknod: { params: [{ name: "path", type: "i64" }, { name: "mode", type: "i64" }, { name: "dev", type: "i64" }], returnType: i64Type },
      opendir: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      readdir: { params: [{ name: "dirp", type: "i64" }], returnType: i64Type },
      closedir: { params: [{ name: "dirp", type: "i64" }], returnType: i64Type },
      rewinddir: { params: [{ name: "dirp", type: "i64" }], returnType: voidType2 },
      chdir: { params: [{ name: "path", type: "i64" }], returnType: i64Type },
      fchdir: { params: [{ name: "fd", type: "i64" }], returnType: i64Type },
      getcwd: { params: [{ name: "buf", type: "i64" }, { name: "size", type: "i64" }], returnType: i64Type }
    };
    for (const [name, func] of Object.entries(posixFunctions)) {
      this.symbolTable.declare(name, "function", func.returnType);
    }
  }
  emitError(message, position) {
    this.errors.push({ message, position });
  }
  visitProgram(node) {
    const program = node;
    for (const stmt of program.statements) {
      this.visitStatement(stmt);
    }
  }
  visitStatement(node) {
    switch (node.type) {
      case "VariableDeclaration":
        this.visitVariableDeclaration(node);
        break;
      case "Assignment":
        this.visitAssignment(node);
        break;
      case "AssignmentExpression":
        this.visitAssignment(node);
        break;
      case "ExpressionStatement":
        this.visitExpressionStatement(node);
        break;
      case "Block":
        this.visitBlock(node);
        break;
      case "Return":
        this.visitReturn(node);
        break;
      case "Conditional":
        this.visitConditional(node);
        break;
      case "WhileLoop":
        this.visitWhileLoop(node);
        break;
      case "ForLoop":
        this.visitForLoop(node);
        break;
      case "FunctionDeclaration":
        this.visitFunctionDeclaration(node);
        break;
    }
  }
  visitVariableDeclaration(node) {
    const declaredType = node.varType;
    const valueType = node.value ? this.visitExpression(node.value) : null;
    if (valueType) {
      if (declaredType) {
        if (!sameType(valueType, declaredType)) {
          this.emitError(`Type mismatch: cannot assign ${valueType.toString()} to ${declaredType.toString()} (requires explicit cast)`, node.position);
        }
        this.symbolTable.declare(node.name, "variable", declaredType);
        this.symbolTable.setCurrentType(node.name, declaredType);
      } else {
        this.symbolTable.declare(node.name, "variable", valueType);
        this.symbolTable.setCurrentType(node.name, valueType);
      }
    } else {
      if (declaredType) {
        this.symbolTable.declare(node.name, "variable", declaredType);
      } else {
        this.emitError(`Cannot infer type for variable '${node.name}'`, node.position);
      }
    }
  }
  visitAssignment(node) {
    let symbol = this.symbolTable.lookup(node.name);
    if (!symbol) {
      this.emitError(`Undefined variable '${node.name}'`, node.position);
      this.visitExpression(node.value);
      return;
    }
    if (symbol.symbolType !== "variable") {
      this.emitError(`Cannot assign to ${symbol.symbolType} '${node.name}'`, node.position);
    }
    const valueType = this.visitExpression(node.value);
    if (valueType && symbol.declaredType) {
      if (!sameType(valueType, symbol.declaredType)) {
        this.emitError(`Type mismatch: cannot assign ${valueType.toString()} to ${symbol.declaredType.toString()} (requires explicit cast)`, node.position);
      }
    } else if (valueType && symbol.inferredType) {
      if (!sameType(valueType, symbol.inferredType)) {
        this.emitError(`Type mismatch: cannot assign ${valueType.toString()} to ${symbol.inferredType.toString()} (requires explicit cast)`, node.position);
      }
    }
  }
  visitExpressionStatement(node) {
    this.visitExpression(node.expression);
  }
  visitAssignmentExpression(node) {
    const valueType = this.visitExpression(node.value);
    if (node.name !== undefined) {
      let symbol = this.symbolTable.lookup(node.name);
      if (!symbol) {
        this.emitError(`Undefined variable '${node.name}'`, node.position);
        return null;
      }
      if (symbol.symbolType !== "variable") {
        this.emitError(`Cannot assign to ${symbol.symbolType} '${node.name}'`, node.position);
        return null;
      }
      if (valueType && symbol.declaredType) {
        if (!sameType(valueType, symbol.declaredType)) {
          this.emitError(`Type mismatch: cannot assign ${valueType.toString()} to ${symbol.declaredType.toString()} (requires explicit cast)`, node.position);
        }
      } else if (valueType && symbol.inferredType) {
        if (!sameType(valueType, symbol.inferredType)) {
          this.emitError(`Type mismatch: cannot assign ${valueType.toString()} to ${symbol.inferredType.toString()} (requires explicit cast)`, node.position);
        }
      }
    } else if (node.target !== undefined) {
      const objectValue = this.visitExpression(node.target.object);
      const indexValue = this.visitExpression(node.target.index);
      let targetType = null;
      if (objectValue && isArrayType(objectValue)) {
        targetType = objectValue.elementType;
      }
      if (targetType && valueType && !sameType(targetType, valueType)) {
        this.emitError(`Type mismatch: cannot assign ${valueType.toString()} to array element of type ${targetType.toString()} (requires explicit cast)`, node.position);
      }
      return targetType;
    }
    return valueType;
  }
  visitBlock(node) {
    this.symbolTable.pushScope();
    for (const stmt of node.statements) {
      this.visitStatement(stmt);
    }
    this.symbolTable.popScope();
  }
  visitReturn(node) {
    if (!this.currentFunction) {
      this.emitError(`Return statement outside function`, node.position);
    }
    if (node.value) {
      this.visitExpression(node.value);
    }
  }
  visitConditional(node) {
    const conditionType = this.visitExpression(node.condition);
    if (node.condition.type === "AssignmentExpression") {
      this.emitError("Assignment (:=) cannot be used as a condition. Use == for comparison.", node.condition.position);
    }
    if (conditionType) {
      if (!isIntegerType(conditionType) && !isUnsignedType(conditionType)) {
        this.emitError(`Condition must be integer or unsigned type, got ${conditionType.toString()}`, node.condition.position);
      }
    }
    this.visitBlock(node.trueBranch);
    if (node.falseBranch) {
      this.visitBlock(node.falseBranch);
    }
  }
  visitWhileLoop(node) {
    const conditionType = this.visitExpression(node.test);
    if (node.test.type === "AssignmentExpression") {
      this.emitError("Assignment (:=) cannot be used as a condition. Use == for comparison.", node.test.position);
    }
    if (conditionType) {
      if (!isIntegerType(conditionType) && !isUnsignedType(conditionType)) {
        this.emitError(`While loop condition must be integer or unsigned type, got ${conditionType.toString()}`, node.test.position);
      }
    }
    this.visitBlock(node.body);
  }
  visitForLoop(node) {
    const startType = this.visitExpression(node.start);
    const endType = this.visitExpression(node.end);
    if (startType && endType) {
      if (!isIntegerType(startType) || !isIntegerType(endType)) {
        this.emitError(`For loop bounds must be integer types`, node.position);
      }
    }
    this.symbolTable.pushScope();
    this.symbolTable.declare(node.variable, "variable", startType || new IntegerType("i64"));
    this.visitBlock(node.body);
    this.symbolTable.popScope();
  }
  visitFunctionDeclaration(node) {
    this.symbolTable.declare(node.name, "function", node.returnType);
    const prevFunction = this.currentFunction;
    this.currentFunction = node.name;
    this.symbolTable.pushScope();
    for (const param of node.params) {
      this.symbolTable.declare(param.name, "parameter", param.paramType);
      this.symbolTable.setCurrentType(param.name, param.paramType);
    }
    this.visitBlock(node.body);
    this.symbolTable.popScope();
    this.currentFunction = prevFunction;
  }
  visitExpression(node) {
    switch (node.type) {
      case "Identifier":
        return this.visitIdentifier(node);
      case "NumberLiteral":
        return this.visitNumberLiteral(node);
      case "StringLiteral":
        return this.visitStringLiteral(node);
      case "ArrayLiteral":
        return this.visitArrayLiteral(node);
      case "TableLiteral":
        return this.visitTableLiteral(node);
      case "BinaryExpression":
        return this.visitBinaryExpression(node);
      case "UnaryExpression":
        return this.visitUnaryExpression(node);
      case "CallExpression":
        return this.visitCallExpression(node);
      case "MemberExpression":
        return this.visitMemberExpression(node);
      case "IndexExpression":
        return this.visitIndexExpression(node);
      case "LLMExpression":
        return this.visitLLMExpression(node);
      case "Lambda":
        return this.visitLambda(node);
      case "BlockExpression":
        return this.visitBlockExpression(node);
      case "CastExpression":
        return this.visitCastExpression(node);
      case "AssignmentExpression":
        return this.visitAssignmentExpression(node);
      default:
        const unknown = node;
        this.emitError(`Unknown expression type: ${unknown.type}`, unknown.position);
        return null;
    }
  }
  visitIdentifier(node) {
    const symbol = this.symbolTable.lookup(node.name);
    if (!symbol) {
      this.emitError(`Undefined variable '${node.name}'`, node.position);
      return null;
    }
    return symbol.declaredType ?? symbol.inferredType;
  }
  visitNumberLiteral(node) {
    if (node.literalType) {
      return node.literalType;
    }
    const value = typeof node.value === "string" ? node.value.toLowerCase() : String(node.value);
    const isFloat = value.includes(".") || value.includes("e") || value.includes("E") || Number.isFinite(node.value) && !Number.isInteger(node.value);
    if (isFloat) {
      return new FloatType("f32");
    }
    return new IntegerType("i64");
  }
  visitStringLiteral(node) {
    return new ArrayType(new UnsignedType("u8"), []);
  }
  visitArrayLiteral(node) {
    if (node.elements.length === 0) {
      this.emitError(`Cannot infer type for empty array literal`, node.position);
      return null;
    }
    const elementTypes = [];
    for (const elem of node.elements) {
      const elemType = this.visitExpression(elem);
      if (elemType) {
        elementTypes.push(elemType);
      }
    }
    if (elementTypes.length === 0) {
      return null;
    }
    let commonType = elementTypes[0];
    for (let i2 = 1;i2 < elementTypes.length; i2++) {
      if (!sameType(commonType, elementTypes[i2])) {
        if (sameType(elementTypes[i2], commonType)) {
          commonType = commonType;
        } else if (sameType(commonType, elementTypes[i2])) {
          commonType = elementTypes[i2];
        } else {
          this.emitError(`Array elements have incompatible types: ${commonType.toString()} and ${elementTypes[i2].toString()}`, node.position);
          return commonType;
        }
      }
    }
    return new ArrayType(commonType, [node.elements.length]);
  }
  visitTableLiteral(node) {
    const keyTypes = [];
    const valueTypes = [];
    for (const col of node.columns) {
      if (col.columnType) {
        keyTypes.push(col.columnType);
      }
      if (col.values.length === 0) {
        continue;
      }
      const firstValueType = this.visitExpression(col.values[0]);
      if (!firstValueType) {
        continue;
      }
      const colTypes = [firstValueType];
      for (let i2 = 1;i2 < col.values.length; i2++) {
        const valType = this.visitExpression(col.values[i2]);
        if (valType) {
          colTypes.push(valType);
        }
      }
      if (colTypes.length > 0) {
        let commonType = colTypes[0];
        for (let i2 = 1;i2 < colTypes.length; i2++) {
          if (!sameType(commonType, colTypes[i2])) {
            this.emitError(`Table column '${col.name}' has incompatible types`, node.position);
          }
        }
        valueTypes.push(commonType);
      }
    }
    if (valueTypes.length === 0) {
      return null;
    }
    return new TableType(new IntegerType("i32"), valueTypes[0]);
  }
  visitBinaryExpression(node) {
    const leftType = this.visitExpression(node.left);
    const rightType = this.visitExpression(node.right);
    if (!leftType || !rightType) {
      return null;
    }
    const operator = node.operator;
    const arithmeticOperators = ["+", "-", "*", "/", "%", "&", "|", "TIMES", "DIVIDE"];
    const comparisonOperators = ["==", "!=", "<", ">", "<=", ">="];
    const logicalOperators = ["&&", "||", "and", "or", "AND", "OR"];
    if (arithmeticOperators.includes(operator)) {
      if (!isNumericType(leftType) || !isNumericType(rightType)) {
        this.emitError(`Operator '${operator}' requires numeric types`, node.position);
        return leftType;
      }
      if (!sameType(leftType, rightType)) {
        this.emitError(`Operator '${operator}' requires same types, got ${leftType.toString()} and ${rightType.toString()}`, node.position);
        return leftType;
      }
      if (isArrayType(leftType) || isArrayType(rightType)) {
        if (!sameType(leftType, rightType)) {
          this.emitError(`Array operations require same array types`, node.position);
        }
      }
      return leftType;
    }
    if (comparisonOperators.includes(operator)) {
      const commonType = getCommonType(leftType, rightType);
      if (!commonType) {
        this.emitError(`Cannot compare ${leftType.toString()} with ${rightType.toString()}`, node.position);
      }
      return new IntegerType("i32");
    }
    if (logicalOperators.includes(operator)) {
      if (!isNumericType(leftType) || !isNumericType(rightType)) {
        this.emitError(`Logical operator '${operator}' requires numeric types`, node.position);
      }
      return new IntegerType("i32");
    }
    this.emitError(`Unknown binary operator '${operator}'`, node.position);
    return leftType;
  }
  visitUnaryExpression(node) {
    const argumentType = this.visitExpression(node.argument);
    if (!argumentType) {
      return null;
    }
    const operator = node.operator;
    if (operator === "!") {
      if (!isNumericType(argumentType)) {
        this.emitError(`Operator '!' requires numeric type`, node.position);
      }
      return argumentType;
    }
    if (operator === "-" || operator === "+") {
      if (!isNumericType(argumentType)) {
        this.emitError(`Operator '${operator}' requires numeric type`, node.position);
      }
      return argumentType;
    }
    this.emitError(`Unknown unary operator '${operator}'`, node.position);
    return argumentType;
  }
  visitCastExpression(node) {
    const valueType = this.visitExpression(node.value);
    if (!valueType) {
      return null;
    }
    const targetType = node.targetType;
    if (!(isIntegerType(targetType) || isUnsignedType(targetType) || isFloatType(targetType))) {
      this.emitError(`Cast target must be a numeric type, got ${targetType.toString()}`, node.position);
      return null;
    }
    if (!(isIntegerType(valueType) || isUnsignedType(valueType) || isFloatType(valueType))) {
      this.emitError(`Cast source must be a numeric type, got ${valueType.toString()}`, node.position);
      return null;
    }
    const sourceUnsigned = isUnsignedType(valueType);
    const sourceSigned = isIntegerType(valueType);
    const targetUnsigned = isUnsignedType(targetType);
    const targetSigned = isIntegerType(targetType);
    const sourceFloat = isFloatType(valueType);
    const targetFloat = isFloatType(targetType);
    const sourceBits = valueType.bits;
    const targetBits = targetType.bits;
    if (sourceUnsigned && targetUnsigned) {
      if (sourceBits > targetBits) {
        this.emitError(`Lossy cast from ${valueType.toString()} to ${targetType.toString()}: would lose precision`, node.position);
      }
    } else if (sourceSigned && targetSigned) {
      if (sourceBits > targetBits) {
        this.emitError(`Lossy cast from ${valueType.toString()} to ${targetType.toString()}: would lose precision`, node.position);
      }
    } else if ((sourceSigned || sourceUnsigned) && targetFloat) {
      return targetType;
    } else if (sourceFloat && (targetSigned || targetUnsigned)) {
      this.emitError(`Precision loss warning: casting from ${valueType.toString()} to ${targetType.toString()} may lose precision`, node.position);
    } else if (sourceUnsigned && targetSigned) {
      if (sourceBits > targetBits) {
        this.emitError(`Lossy cast from ${valueType.toString()} to ${targetType.toString()}: would lose precision`, node.position);
      }
    } else if (sourceSigned && targetUnsigned) {
      this.emitError(`Signed to unsigned cast from ${valueType.toString()} to ${targetType.toString()} may change sign semantics`, node.position);
    }
    return targetType;
  }
  visitCallExpression(node) {
    if (node.callee.type === "Identifier") {
      const identifier = node.callee;
      const symbol = this.symbolTable.lookup(identifier.name);
      if (!symbol) {
        this.emitError(`Undefined function '${identifier.name}'`, node.position);
        return null;
      }
    }
    const calleeType = this.visitExpression(node.callee);
    if (!calleeType) {
      return null;
    }
    if (node.callee.type === "Identifier") {
      const identifier = node.callee;
      const symbol = this.symbolTable.lookup(identifier.name);
      if (symbol && symbol.symbolType === "variable") {
        if (symbol.inferredType && isArrayType(symbol.inferredType)) {
          const arrayType = symbol.inferredType;
          const args = node.args ?? node.arguments;
          if (args.length === 1) {
            const indexType = this.visitExpression(args[0]);
            if (indexType && !(isIntegerType(indexType) || isUnsignedType(indexType))) {
              this.emitError(`Array index must be integer type`, args[0].position);
            }
            return arrayType.elementType;
          }
        }
      }
    }
    if (node.callee.type === "MemberExpression") {
      const memberExpr = node.callee;
      const objectType = this.visitExpression(memberExpr.object);
      if (objectType && isArrayType(objectType)) {
        const arrayType = objectType;
        const args = node.args ?? node.arguments;
        if (args.length === 1) {
          const indexType = this.visitExpression(args[0]);
          if (indexType && !(isIntegerType(indexType) || isUnsignedType(indexType))) {
            this.emitError(`Array index must be integer type`, args[0].position);
          }
          return arrayType.elementType;
        }
      }
    }
    return null;
  }
  visitMemberExpression(node) {
    const objectType = this.visitExpression(node.object);
    if (!objectType) {
      return null;
    }
    if (node.object.type === "Identifier") {
      const identifier = node.object;
      const symbol = this.symbolTable.lookup(identifier.name);
      if (symbol && symbol.declaredType && isTableType(symbol.declaredType)) {
        const tableType = symbol.declaredType;
        return tableType.valueType;
      }
    }
    if (isArrayType(objectType)) {
      const arrayType = objectType;
      if (arrayType.rank > 0) {
        const newDimensions = arrayType.dimensions.slice(0, -1);
        if (newDimensions.length === 0) {
          return arrayType.elementType;
        }
        return new ArrayType(arrayType.elementType, newDimensions);
      }
      return arrayType.elementType;
    }
    this.emitError(`Cannot access property '${node.property}' on type ${objectType.toString()}`, node.position);
    return null;
  }
  visitIndexExpression(node) {
    const objectType = this.visitExpression(node.object);
    if (!objectType) {
      return null;
    }
    const indexType = this.visitExpression(node.index);
    if (!indexType) {
      return null;
    }
    if (!(isIntegerType(indexType) || isUnsignedType(indexType))) {
      this.emitError(`Array index must be integer type, got ${indexType.toString()}`, node.index.position);
    }
    if (isArrayType(objectType)) {
      const arrayType = objectType;
      if (arrayType.rank > 0) {
        const newDimensions = arrayType.dimensions.slice(0, -1);
        if (newDimensions.length === 0) {
          return arrayType.elementType;
        }
        return new ArrayType(arrayType.elementType, newDimensions);
      }
      return arrayType.elementType;
    }
    this.emitError(`Cannot index into non-array type ${objectType.toString()}`, node.position);
    return null;
  }
  visitLLMExpression(node) {
    const promptType = this.visitExpression(node.prompt);
    if (promptType) {
      const expectedStringType = new ArrayType(new UnsignedType("u8"), []);
      if (!sameType(promptType, expectedStringType) && !sameType(promptType, expectedStringType)) {
        this.emitError(`LLM prompt must be string type ([u8])`, node.prompt.position);
      }
    }
    const modelSizeType = this.visitExpression(node.modelSize);
    if (modelSizeType) {
      const expectedStringType = new ArrayType(new UnsignedType("u8"), []);
      if (!sameType(modelSizeType, expectedStringType) && !sameType(modelSizeType, expectedStringType)) {
        this.emitError(`LLM model_size must be string type`, node.modelSize.position);
      }
    }
    const reasoningEffortType = this.visitExpression(node.reasoningEffort);
    if (reasoningEffortType) {
      if (!isFloatType(reasoningEffortType) && !(isIntegerType(reasoningEffortType) || isUnsignedType(reasoningEffortType))) {
        this.emitError(`LLM reasoning_effort must be numeric type`, node.reasoningEffort.position);
      }
    }
    const contextType = this.visitExpression(node.context);
    if (contextType && !isArrayType(contextType)) {
      this.emitError(`LLM context must be array type`, node.context.position);
    }
    return node.returnType;
  }
  visitLambda(node) {
    return node.returnType;
  }
  visitBlockExpression(node) {
    this.visitBlock(node.block);
    if (node.block.statements.length > 0) {
      const lastStmt = node.block.statements[node.block.statements.length - 1];
      if (lastStmt.type === "ExpressionStatement") {
        return this.visitExpression(lastStmt.expression);
      }
    }
    return new VoidType;
  }
}

// src/llvm_codegen.ts
class LLVMIRGenerator {
  functions = new Map;
  globals = [];
  currentFunction = null;
  blockCounter = 0;
  valueCounter = 0;
  labelCounter = 0;
  variableTypes = new Map;
  stringConstants = [];
  generate(ast) {
    const ir = [];
    this.stringConstants = [];
    this.valueCounter = 0;
    this.variableTypes.clear();
    ir.push("; AlgolScript LLVM IR");
    ir.push("; Generated by AlgolScript compiler");
    ir.push("");
    const platform = process.platform === "darwin" ? "aarch64-apple-darwin" : process.platform === "win32" ? "x86_64-pc-windows-msvc" : "x86_64-unknown-linux-gnu";
    ir.push(`target triple = "${platform}"`);
    ir.push("");
    this.setupDeclarations(ir);
    const functionDeclarations = this.collectFunctionDeclarations(ast);
    ir.push("; String constants");
    for (const str of this.stringConstants) {
      ir.push(str);
    }
    ir.push("");
    const mainFunc = functionDeclarations.find((f) => f.name === "main");
    const hasMain = !!mainFunc;
    ir.push("; Function declarations");
    for (const funcDecl of functionDeclarations) {
      if (funcDecl.name === "main") {
        funcDecl.name = "program_user";
      }
      this.generateFunctionDeclaration(ir, funcDecl);
    }
    ir.push("");
    if (hasMain) {
      ir.push("; Main entry point (calls user's main)");
      const hasParams = mainFunc && mainFunc.params.length >= 2;
      if (hasParams) {
        ir.push("define i32 @main(i32 %argc, ptr %argv) {");
        ir.push("entry:");
        ir.push("  call void @gc_init()");
        ir.push("  %args_array = call ptr @array_alloc(i64 8, i64 1, i64 %argc)");
        ir.push("  %args_array_data = getelementptr ptr, ptr %args_array, i64 0");
        for (let i2 = 0;i2 < 10; i2++) {
          ir.push(`  ; Process argv[${i2}]`);
          ir.push(`  %argv_gep_${i2} = getelementptr ptr, ptr %argv, i64 ${i2}`);
          ir.push(`  %argv_ptr_${i2} = load ptr, ptr %argv_gep_${i2}`);
          ir.push(`  %in_bounds_${i2} = icmp ult i64 ${i2}, %argc`);
          ir.push(`  br i1 %in_bounds_${i2}, label %store_argv_${i2}, label %argv_done_${i2}`);
          ir.push(`store_argv_${i2}:`);
          ir.push(`  %array_gep_${i2} = getelementptr i64, ptr %args_array_data, i64 ${i2}`);
          ir.push(`  %argv_int_${i2} = ptrtoint ptr %argv_ptr_${i2} to i64`);
          ir.push(`  store i64 %argv_int_${i2}, ptr %array_gep_${i2}`);
          ir.push(`  br label %argv_done_${i2}`);
          ir.push(`argv_done_${i2}:`);
        }
        ir.push(`  ; Skip remaining argv elements (${i}..)`);
        ir.push("  %cli_table = call ptr @table_new(i64 2)");
        ir.push("  %argc_key = alloca [5 x i8]");
        ir.push('  store [5 x i8] c"argc\\00", ptr %argc_key');
        ir.push("  call void @table_set(ptr %cli_table, ptr %argc_key, i64 4, i64 %argc)");
        ir.push("  %args_key = alloca [5 x i8]");
        ir.push('  store [5 x i8] c"args\\00", ptr %args_key');
        ir.push("  %args_int = ptrtoint ptr %args_array to i64");
        ir.push("  call void @table_set(ptr %cli_table, ptr %args_key, i64 4, i64 %args_int)");
        ir.push("  %cli_table_int = ptrtoint ptr %cli_table to i64");
        ir.push("  %result = call i64 @program_user(i64 %argc, i64 %cli_table_int)");
      } else {
        ir.push("define i32 @main(i32 %argc, ptr %argv) {");
        ir.push("entry:");
        ir.push("  call void @gc_init()");
        ir.push("  %result = call i64 @program_user()");
      }
      ir.push("  %truncated = trunc i64 %result to i32");
      ir.push("  ret i32 %truncated");
      ir.push("}");
    } else {
      ir.push("define void @program() {");
      ir.push("entry:");
      for (const statement of ast.statements) {
        this.generateStatement(ir, statement);
      }
      ir.push("  ret void");
      ir.push("}");
      this.setupMainFunction(ir);
    }
    this.generateRuntimeFunctions(ir);
    return ir.join(`
`);
  }
  collectFunctionDeclarations(ast) {
    const functions = [];
    const declarations = this.findFunctionDeclarationsRecursive(ast.statements);
    functions.push(...declarations);
    return functions;
  }
  findFunctionDeclarationsRecursive(statements) {
    const functions = [];
    for (const stmt of statements) {
      if (stmt.type === "FunctionDeclaration") {
        functions.push(stmt);
      } else if (stmt.type === "Block") {
        functions.push(...this.findFunctionDeclarationsRecursive(stmt.statements));
      }
    }
    return functions;
  }
  setupDeclarations(ir) {
    ir.push("; Declare external LLM function");
    ir.push("declare ptr @llm_call(ptr %prompt, ptr %options, ptr %return_type)");
    ir.push("");
    ir.push("; Declare GC functions");
    ir.push("declare void @gc_init()");
    ir.push("declare ptr @gc_alloc(i64 %size)");
    ir.push("declare void @gc_collect()");
    ir.push("");
    ir.push("; Declare array functions");
    ir.push("declare ptr @array_alloc(i64 %element_size, i64 %dimension_count, i64 %dimensions)");
    ir.push("declare i64 @array_get(ptr %array, i64 %index)");
    ir.push("declare void @array_set(ptr %array, i64 %index, i64 %value)");
    ir.push("");
    ir.push("; Declare table functions");
    ir.push("declare ptr @table_new(i64 %initial_capacity)");
    ir.push("declare i64 @table_get(ptr %table, ptr %key, i64 %key_len)");
    ir.push("declare void @table_set(ptr %table, ptr %key, i64 %key_len, i64 %value)");
    ir.push("");
    ir.push("; Declare CLI argument helper functions");
    ir.push("declare i64 @get_argc_value(ptr %cli_table)");
    ir.push("declare i64 @get_argv_value(ptr %cli_table, i64 %index)");
    ir.push("");
    ir.push("; Declare POSIX filesystem operations (from Cosmopolitan Libc)");
    ir.push("declare i32 @open(ptr, i32, ...)");
    ir.push("declare i64 @read(i32, ptr, i64)");
    ir.push("declare i64 @write(i32, ptr, i64)");
    ir.push("declare i64 @pread(i32, ptr, i64, i64)");
    ir.push("declare i64 @pwrite(i32, ptr, i64, i64)");
    ir.push("declare i64 @lseek(i32, i64, i32)");
    ir.push("declare i32 @close(i32)");
    ir.push("declare i32 @fsync(i32)");
    ir.push("declare i32 @fdatasync(i32)");
    ir.push("declare i32 @stat(ptr, ptr)");
    ir.push("declare i32 @lstat(ptr, ptr)");
    ir.push("declare i32 @fstat(i32, ptr)");
    ir.push("declare i32 @access(ptr, i32)");
    ir.push("declare i32 @faccessat(i32, ptr, i32, i32)");
    ir.push("declare i32 @utimes(ptr, ptr)");
    ir.push("declare i32 @futimes(i32, ptr)");
    ir.push("declare i32 @utimensat(i32, ptr, ptr, i32)");
    ir.push("declare i32 @chmod(ptr, i32)");
    ir.push("declare i32 @fchmod(i32, i32)");
    ir.push("declare i32 @chown(ptr, i32, i32)");
    ir.push("declare i32 @fchown(i32, i32, i32)");
    ir.push("declare i32 @umask(i32)");
    ir.push("declare i32 @truncate(ptr, i64)");
    ir.push("declare i32 @ftruncate(i32, i64)");
    ir.push("declare i32 @link(ptr, ptr)");
    ir.push("declare i32 @symlink(ptr, ptr)");
    ir.push("declare i64 @readlink(ptr, ptr, i64)");
    ir.push("declare i32 @rename(ptr, ptr)");
    ir.push("declare i32 @unlink(ptr)");
    ir.push("declare i32 @mkdir(ptr, i32)");
    ir.push("declare i32 @rmdir(ptr)");
    ir.push("declare ptr @opendir(ptr)");
    ir.push("declare ptr @readdir(ptr)");
    ir.push("declare void @rewinddir(ptr)");
    ir.push("declare i32 @closedir(ptr)");
    ir.push("declare i32 @chdir(ptr)");
    ir.push("declare i32 @fchdir(i32)");
    ir.push("declare ptr @getcwd(ptr, i64)");
    ir.push("declare i32 @fcntl(i32, i32, ...)");
    ir.push("declare i64 @pathconf(ptr, i32)");
    ir.push("declare i64 @fpathconf(i32, i32)");
    ir.push("declare i32 @dup(i32)");
    ir.push("declare i32 @dup2(i32, i32)");
    ir.push("declare i32 @creat(ptr, i32)");
    ir.push("declare i32 @mkfifo(ptr, i32)");
    ir.push("declare i32 @mknod(ptr, i32, i64)");
    ir.push("declare ptr @__errno_location()");
    ir.push("");
  }
  generateStatement(ir, statement) {
    switch (statement.type) {
      case "VariableDeclaration": {
        const llvmType = this.toLLVMType(statement.varType);
        const reg = `%${statement.name}`;
        ir.push(`  ${reg} = alloca ${llvmType}`);
        this.variableTypes.set(statement.name, llvmType);
        if (statement.value) {
          const value2 = this.generateExpression(ir, statement.value);
          ir.push(`  store ${llvmType} ${value2}, ptr ${reg}`);
        }
        break;
      }
      case "Assignment":
        const value = this.generateExpression(ir, statement.value);
        ir.push(`  store i64 ${value}, ptr %${statement.name}`);
        break;
      case "ExpressionStatement":
        this.generateExpression(ir, statement.expression);
        break;
      case "Block":
        for (const stmt of statement.statements) {
          this.generateStatement(ir, stmt);
        }
        break;
      case "Return":
        if (statement.value) {
          const value2 = this.generateExpression(ir, statement.value);
          ir.push(`  ret ${this.currentFunction?.returnType || "i64"} ${value2}`);
        } else {
          ir.push(`  ret ${this.currentFunction?.returnType || "void"}`);
        }
        break;
      case "Conditional":
        this.generateConditional(ir, statement);
        break;
      case "FunctionDeclaration":
        break;
      case "WhileLoop":
        this.generateWhileLoop(ir, statement);
        break;
      case "ForLoop":
        this.generateForLoop(ir, statement);
        break;
      default:
        break;
    }
  }
  generateExpression(ir, expr) {
    switch (expr.type) {
      case "AssignmentExpression":
        return this.generateAssignmentExpression(ir, expr);
      case "NumberLiteral":
        return `${expr.value}`;
      case "POSIXConstant":
        return `${getPOSIXConstant(expr.value)}`;
      case "StringLiteral":
        return this.generateStringLiteral(ir, expr.value);
      case "Identifier": {
        const name = expr.name;
        const varType = this.variableTypes.get(name) || "i64";
        const ptrReg = `%${name}_local`;
        const valReg = `%${this.valueCounter++}`;
        if (this.currentFunction && this.currentFunction.params.find((p) => p.name === name)) {
          ir.push(`  ${valReg} = load ${varType}, ptr ${ptrReg}`);
          return valReg;
        }
        ir.push(`  ${valReg} = load ${varType}, ptr %${name}`);
        return valReg;
      }
      case "BinaryExpression":
        return this.generateBinaryExpression(ir, expr);
      case "UnaryExpression":
        return this.generateUnaryExpression(ir, expr);
      case "CallExpression":
        return this.generateCallExpression(ir, expr);
      case "ArrayLiteral":
        return this.generateArrayLiteral(ir, expr);
      case "TableLiteral":
        return this.generateTableLiteral(ir, expr);
      case "MemberExpression":
        return this.generateMemberExpression(ir, expr);
      case "IndexExpression":
        return this.generateIndexExpression(ir, expr);
      case "LLMCall":
        return this.generateLLMCall(ir, expr);
      case "MapExpression":
        return this.generateMapExpression(ir, expr);
      case "CastExpression":
        return this.generateCastExpression(ir, expr);
      default:
        return "";
    }
  }
  generateStringLiteral(ir, value) {
    const name = `@str${this.valueCounter++}`;
    const escaped = value.replace(/\\/g, "\\\\").replace(/"/g, "\\\"");
    const strDef = `${name} = private unnamed_addr constant [${value.length + 1} x i8] c"${escaped}\\00"`;
    this.stringConstants.push(strDef);
    return name;
  }
  generateBinaryExpression(ir, expr) {
    const left = this.generateExpression(ir, expr.left);
    const right = this.generateExpression(ir, expr.right);
    const reg = `%${this.valueCounter++}`;
    const opMap = {
      "+": ["add i64", "add"],
      "-": ["sub i64", "sub"],
      "*": ["mul i64", "mul"],
      "/": ["sdiv i64", "fdiv"],
      "%": ["srem i64", "frem"],
      "<": ["icmp slt i64", "fcmp olt"],
      ">": ["icmp sgt i64", "fcmp ogt"],
      "<=": ["icmp sle i64", "fcmp ole"],
      ">=": ["icmp sge i64", "fcmp oge"],
      "=": ["icmp eq i64", "fcmp oeq"],
      "==": ["icmp eq i64", "fcmp oeq"],
      "!=": ["icmp ne i64", "fcmp one"],
      "&&": ["and i64", "and"],
      "||": ["or i64", "or"],
      "&": ["and i64", "and"],
      "|": ["or i64", "or"]
    };
    const [intOp, floatOp] = opMap[expr.operator];
    const isFloat = expr.left.type === "FloatLiteral";
    const op = isFloat ? `${floatOp} ${left}, ${right}` : `${intOp} ${left}, ${right}`;
    ir.push(`  ${reg} = ${op}`);
    return reg;
  }
  generateUnaryExpression(ir, expr) {
    const argument = expr.argument || expr.operand;
    const value = this.generateExpression(ir, argument);
    const reg = `%${this.valueCounter++}`;
    if (expr.operator === "-") {
      ir.push(`  ${reg} = sub i64 0, ${value}`);
    } else if (expr.operator === "!") {
      ir.push(`  ${reg} = xor i64 ${value}, 1`);
    }
    return reg;
  }
  generateCallExpression(ir, expr) {
    const args = (expr.args || expr.arguments || []).map((arg) => this.generateExpression(ir, arg)).filter(Boolean);
    if (expr.callee.type === "Identifier") {
      const funcName = expr.callee.name;
      const funcInfo = this.functions.get(funcName);
      let typedArgs;
      let returnType = "i64";
      if (funcInfo && funcInfo.params) {
        typedArgs = args.map((arg, i2) => {
          const paramType = funcInfo.params[i2]?.type || "i64";
          return `${paramType} ${arg}`;
        });
        returnType = funcInfo.returnType || "i64";
      } else {
        const posixPointerVars = [
          "open",
          "read",
          "write",
          "pread",
          "pwrite",
          "stat",
          "lstat",
          "fstat",
          "chmod",
          "fchmod",
          "chown",
          "fchown",
          "truncate",
          "ftruncate",
          "access",
          "faccessat",
          "link",
          "symlink",
          "readlink",
          "rename",
          "unlink",
          "mkdir",
          "rmdir",
          "opendir",
          "closedir",
          "chdir",
          "fchdir",
          "getcwd",
          "utimes",
          "futimes",
          "utimensat"
        ];
        typedArgs = args.map((arg, i2) => {
          if (posixPointerVars.includes(funcName) && arg.startsWith("@str")) {
            return `ptr ${arg}`;
          }
          return `i64 ${arg}`;
        });
      }
      const reg = `%${this.valueCounter++}`;
      ir.push(`  ${reg} = call ${returnType} @${funcName}(${typedArgs.join(", ")})`);
      return reg;
    }
    return "";
  }
  generateAssignmentExpression(ir, expr) {
    const { name, value, target } = expr;
    const valueReg = this.generateExpression(ir, value);
    if (name !== undefined) {
      ir.push(`  store i64 ${valueReg}, ptr %${name}`);
    } else if (target !== undefined && target.type === "IndexExpression") {
      const arrayReg = this.generateExpression(ir, target.object);
      const indexReg = this.generateExpression(ir, target.index);
      ir.push(`  call void @array_set(ptr ${arrayReg}, i64 ${indexReg}, i64 ${valueReg})`);
    }
    return valueReg;
  }
  generateArrayLiteral(ir, expr) {
    const elementReg = `%${this.valueCounter++}`;
    const size = expr.elements.length;
    const dimensions = size;
    ir.push(`  %size${this.valueCounter} = add i64 ${size}, 0`);
    ir.push(`  %dim_count${this.valueCounter} = add i64 1, 0`);
    ir.push(`  %dimensions${this.valueCounter} = add i64 ${dimensions}, 0`);
    ir.push(`  ${elementReg} = call ptr @array_alloc(i64 %size${this.valueCounter}, i64 %dim_count${this.valueCounter}, i64 %dimensions${this.valueCounter})`);
    this.valueCounter++;
    for (let i2 = 0;i2 < expr.elements.length; i2++) {
      const value = this.generateExpression(ir, expr.elements[i2]);
      if (value) {
        ir.push(`  call void @array_set(ptr ${elementReg}, i64 ${i2}, i64 ${value})`);
      }
    }
    return elementReg;
  }
  generateTableLiteral(ir, expr) {
    const tableReg = `%${this.valueCounter++}`;
    ir.push(`  ${tableReg} = call ptr @table_new(i64 %capacity)`);
    for (let i2 = 0;i2 < expr.entries.length; i2++) {
      const { key, value } = expr.entries[i2];
      const keyStr = this.generateStringLiteral(ir, key);
      const valueReg = this.generateExpression(ir, value);
      if (valueReg) {
        ir.push(`  call void @table_set(ptr ${tableReg}, ptr ${keyStr}, i64 ${key.length}, i64 ${valueReg})`);
      }
    }
    return tableReg;
  }
  generateMemberExpression(ir, expr) {
    const obj = this.generateExpression(ir, expr.object);
    const reg = `%${this.valueCounter++}`;
    ir.push(`  ${reg} = call i64 @table_get(ptr ${obj}, ptr @key_${expr.property}, i64 ${expr.property.length})`);
    return reg;
  }
  generateIndexExpression(ir, expr) {
    const array2 = this.generateExpression(ir, expr.object);
    const index = this.generateExpression(ir, expr.index);
    const reg = `%${this.valueCounter++}`;
    ir.push(`  ${reg} = call i64 @array_get(ptr ${array2}, i64 ${index})`);
    return reg;
  }
  generateLLMCall(ir, expr) {
    const resultReg = `%${this.valueCounter++}`;
    const prompt = this.generateStringLiteral(ir, expr.arguments.prompt);
    const options = this.generateStringLiteral(ir, JSON.stringify(expr.arguments.options || {}));
    const returnType = this.generateStringLiteral(ir, expr.returnType || "string");
    ir.push(`  ${resultReg} = call ptr @llm_call(ptr ${prompt}, ptr ${options}, ptr ${returnType})`);
    return resultReg;
  }
  generateMapExpression(ir, expr) {
    const collection = this.generateExpression(ir, expr.collection);
    const func = expr.function;
    return collection;
  }
  generateCastExpression(ir, expr) {
    const value = this.generateExpression(ir, expr.value);
    const sourceType = this.toLLVMType(expr.sourceType);
    const targetType = this.toLLVMType(expr.targetType);
    const isSourceSigned = expr.sourceType?.type === "IntegerType";
    const isTargetSigned = expr.targetType?.type === "IntegerType";
    const reg = `%${this.valueCounter++}`;
    const sourceIsInt = sourceType.startsWith("i");
    const targetIsInt = targetType.startsWith("i");
    const sourceIsFloat = sourceType.startsWith("f");
    const targetIsFloat = targetType.startsWith("f");
    if (sourceIsInt && targetIsInt) {
      const sourceBits = this.getIntBits(sourceType);
      const targetBits = this.getIntBits(targetType);
      if (targetBits > sourceBits) {
        const op = isSourceSigned ? "sext" : "zext";
        ir.push(`  ${reg} = ${op} ${sourceType} ${value} to ${targetType}`);
      } else if (targetBits < sourceBits) {
        ir.push(`  ${reg} = trunc ${sourceType} ${value} to ${targetType}`);
      } else {
        return value;
      }
    } else if (sourceIsInt && targetIsFloat) {
      const op = isSourceSigned ? "sitofp" : "uitofp";
      ir.push(`  ${reg} = ${op} ${sourceType} ${value} to ${targetType}`);
    } else if (sourceIsFloat && targetIsInt) {
      const op = isTargetSigned ? "fptosi" : "fptoui";
      ir.push(`  ${reg} = ${op} ${sourceType} ${value} to ${targetType}`);
    } else if (sourceIsFloat && targetIsFloat) {
      const sourceBits = this.getFloatBits(sourceType);
      const targetBits = this.getFloatBits(targetType);
      if (targetBits > sourceBits) {
        ir.push(`  ${reg} = fpext ${sourceType} ${value} to ${targetType}`);
      } else if (targetBits < sourceBits) {
        ir.push(`  ${reg} = fptrunc ${sourceType} ${value} to ${targetType}`);
      } else {
        return value;
      }
    }
    return reg;
  }
  generateBlock(ir, statement) {
    for (const stmt of statement.statements) {
      this.generateStatement(ir, stmt);
    }
  }
  generateConditional(ir, statement) {
    const condition = this.generateExpression(ir, statement.test || statement.condition);
    const trueLabel = this.nextLabel();
    const falseLabel = this.nextLabel();
    const endLabel = this.nextLabel();
    ir.push(`  br i1 ${condition}, label %${trueLabel}, label %${falseLabel}`);
    ir.push("");
    ir.push(`${trueLabel}:`);
    const trueBranch = statement.trueBranch || statement.consequent;
    const trueHadTerminator = this.generateBlockWithTerminator(ir, trueBranch);
    if (!trueHadTerminator) {
      ir.push(`  br label %${endLabel}`);
      ir.push("");
    }
    ir.push(`${falseLabel}:`);
    const falseBranch = statement.falseBranch || statement.alternate;
    let falseHadTerminator = false;
    if (falseBranch) {
      falseHadTerminator = this.generateBlockWithTerminator(ir, falseBranch);
      if (!falseHadTerminator) {
        ir.push(`  br label %${endLabel}`);
        ir.push("");
      }
    } else {
      ir.push(`  br label %${endLabel}`);
      ir.push("");
    }
    const needsEndLabel = !falseBranch || falseBranch && !trueHadTerminator && !falseHadTerminator;
    if (needsEndLabel) {
      ir.push(`${endLabel}:`);
    }
  }
  generateBlockWithTerminator(ir, statement) {
    if (statement.type === "Block") {
      const lastStmt = statement.statements[statement.statements.length - 1];
      if (lastStmt && lastStmt.type === "Return") {
        for (const stmt of statement.statements) {
          this.generateStatement(ir, stmt);
        }
        return true;
      }
      for (const stmt of statement.statements) {
        this.generateStatement(ir, stmt);
      }
      return false;
    } else {
      if (statement.type === "Return") {
        this.generateStatement(ir, statement);
        return true;
      }
      this.generateStatement(ir, statement);
      return false;
    }
  }
  generateWhileLoop(ir, statement) {
    const startLabel = this.nextLabel();
    const bodyLabel = this.nextLabel();
    const endLabel = this.nextLabel();
    ir.push(`  br label %${startLabel}`);
    ir.push("");
    ir.push(`${startLabel}:`);
    const condition = this.generateExpression(ir, statement.test || statement.condition);
    ir.push(`  br i1 ${condition}, label %${bodyLabel}, label %${endLabel}`);
    ir.push("");
    ir.push(`${bodyLabel}:`);
    this.generateStatement(ir, statement.body);
    ir.push(`  br label %${startLabel}`);
    ir.push("");
    ir.push(`${endLabel}:`);
  }
  generateForLoop(ir, statement) {
    const headerLabel = this.nextLabel();
    const bodyLabel = this.nextLabel();
    const incLabel = this.nextLabel();
    const endLabel = this.nextLabel();
    const { variable, start, end, body } = statement;
    const reg = `%${variable}`;
    ir.push(`  ${reg} = alloca i64`);
    const startValue = this.generateExpression(ir, start);
    ir.push(`  store i64 ${startValue}, ptr ${reg}`);
    ir.push(`  br label %${headerLabel}`);
    ir.push("");
    ir.push(`${headerLabel}:`);
    const varValue = this.generateExpression(ir, { type: "Identifier", name: variable });
    const endValue = this.generateExpression(ir, end);
    const cond = `%${this.valueCounter++}`;
    ir.push(`  ${cond} = icmp sle i64 ${varValue}, ${endValue}`);
    ir.push(`  br i1 ${cond}, label %${bodyLabel}, label %${endLabel}`);
    ir.push("");
    ir.push(`${bodyLabel}:`);
    this.generateStatement(ir, body);
    ir.push(`  br label %${incLabel}`);
    ir.push("");
    ir.push(`${incLabel}:`);
    const currentValue = this.generateExpression(ir, { type: "Identifier", name: variable });
    const incValue = `%${this.valueCounter++}`;
    ir.push(`  ${incValue} = add i64 ${currentValue}, 1`);
    ir.push(`  store i64 ${incValue}, ptr %${variable}`);
    ir.push(`  br label %${headerLabel}`);
    ir.push("");
    ir.push(`${endLabel}:`);
  }
  generateFunctionDeclaration(ir, statement) {
    const { name, params, returnType, body } = statement;
    const llvmParams = params.map((p) => {
      const llvmType = this.toLLVMType(p.paramType);
      return { name: p.name, type: llvmType };
    });
    const llvmReturnType = this.toLLVMType(returnType);
    const func = {
      name,
      returnType: llvmReturnType,
      params: llvmParams,
      body: []
    };
    this.functions.set(name, func);
    this.currentFunction = func;
    const paramStr = llvmParams.map((p) => `${p.type} %${p.name}`).join(", ");
    ir.push(`define ${llvmReturnType} @${name}(${paramStr}) {`);
    ir.push("entry:");
    for (const param of llvmParams) {
      const paramReg = `%${param.name}`;
      const localReg = `%${param.name}_local`;
      ir.push(`  ${localReg} = alloca ${param.type}`);
      ir.push(`  store ${param.type} ${paramReg}, ptr ${localReg}`);
    }
    ir.push("");
    for (const stmt of body.statements) {
      this.generateStatement(ir, stmt);
    }
    let hasReturn = false;
    for (const stmt of body.statements) {
      if (stmt.type === "Return") {
        hasReturn = true;
        break;
      }
    }
    if (!hasReturn) {
      if (llvmReturnType !== "void") {
        ir.push("  ret i64 0");
      } else {
        ir.push("  ret void");
      }
    }
    ir.push("}");
    ir.push("");
    this.currentFunction = null;
  }
  generateReturn(ir, statement) {
    const returnType = this.currentFunction?.returnType || "i64";
    if (statement.value) {
      const reg = this.generateExpression(ir, statement.value);
      ir.push(`  ret ${returnType} ${reg}`);
    } else {
      ir.push(`  ret ${returnType === "void" ? "void" : "i64 0"}`);
    }
  }
  setupMainFunction(ir) {
    ir.push("");
    ir.push("; Main entry point");
    ir.push("define i32 @main() {");
    ir.push("entry:");
    ir.push("  call void @gc_init()");
    ir.push("  call void @program()");
    ir.push("  ret i32 0");
    ir.push("}");
  }
  generateRuntimeFunctions(ir) {
    ir.push("");
    ir.push("; Runtime function definitions (linked from runtime library)");
  }
  nextLabel() {
    return `label${this.labelCounter++}`;
  }
  toLLVMType(type) {
    if (!type)
      return "i64";
    if (type instanceof IntegerType || type instanceof UnsignedType) {
      return type.kind;
    }
    if (type instanceof FloatType) {
      return type.kind === "f32" ? "f32" : "f64";
    }
    if (type instanceof ArrayType) {
      return "ptr";
    }
    if (type instanceof TableType) {
      return "ptr";
    }
    if (type instanceof VoidType) {
      return "void";
    }
    return "i64";
  }
  getIntBits(type) {
    const match = type.match(/i(\d+)/);
    return match ? parseInt(match[1]) : 64;
  }
  getFloatBits(type) {
    return type === "f64" || type === "double" ? 64 : 32;
  }
}
function generateLLVMIR(ast) {
  const generator = new LLVMIRGenerator;
  return generator.generate(ast);
}

// src/compiler.ts
async function compile(source) {
  const errors = [];
  try {
    const tokens = tokenize(source);
    const filteredTokens = tokens.filter((t) => t.type !== "WHITESPACE" && t.type !== "COMMENT");
    const ast = parseTokens(filteredTokens);
    const analyzer = new SemanticAnalyzer;
    const semanticErrors = analyzer.analyze(ast);
    if (semanticErrors.length > 0) {
      errors.push(...semanticErrors.map((e) => ({
        message: e.message,
        line: e.position.start.line,
        column: e.position.start.column
      })));
      return { llvmIR: "", errors };
    }
    const llvmIR = generateLLVMIR(ast);
    return { llvmIR, errors: [] };
  } catch (error) {
    if (error instanceof Error) {
      let line = 0;
      let column = 0;
      const lineMatch = error.message.match(/at line (\d+)/);
      if (lineMatch) {
        line = parseInt(lineMatch[1], 10);
      }
      errors.push({
        message: error.message,
        line,
        column
      });
    }
    return { llvmIR: "", errors };
  }
}

class AlgolScriptCompiler {
  async compile(source) {
    return compile(source);
  }
}
async function compileToFile(source, outputPath) {
  const result = await compile(source);
  if (result.errors.length > 0) {
    console.error("Compilation errors:");
    for (const error of result.errors) {
      console.error(`  Line ${error.line}, Col ${error.column}: ${error.message}`);
    }
    throw new Error("Compilation failed");
  }
  await Bun.write(outputPath, result.llvmIR);
  console.log(`LLVM IR written to ${outputPath}`);
}
async function compileToExecutable(source, outputPath) {
  const llFile = outputPath.replace(/\.exe$/, "") + ".ll";
  await compileToFile(source, llFile);
  const llc = Bun.spawn(["llc", "-filetype=obj", llFile], { cwd: process.cwd() });
  await llc.exited;
  const objFile = llFile.replace(".ll", ".o");
  const clang = Bun.spawn(["clang", objFile, "-o", outputPath, "-L./runtime", "-lalgol_runtime"], {
    cwd: process.cwd()
  });
  await clang.exited;
  console.log(`Executable written to ${outputPath}`);
}
export {
  compileToFile,
  compileToExecutable,
  compile,
  AlgolScriptCompiler
};
