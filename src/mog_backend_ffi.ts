import { dlopen, FFIType, ptr, CString } from "bun:ffi";
import { resolve } from "path";

const libPath = resolve(import.meta.dir, "../build/libmog_backend.dylib");

const { symbols } = dlopen(libPath, {
  mog_qbe_compile: {
    args: [FFIType.ptr, FFIType.i32],
    returns: FFIType.ptr,
  },
  mog_assemble: {
    args: [FFIType.ptr, FFIType.i32, FFIType.ptr],
    returns: FFIType.i32,
  },
  mog_compile_and_link: {
    args: [FFIType.ptr, FFIType.i32, FFIType.ptr, FFIType.ptr, FFIType.i32],
    returns: FFIType.i32,
  },
});

/**
 * Compile QBE IL to assembly text in-process.
 * Returns the assembly as a string.
 */
export function qbeCompile(qbeIL: string): string {
  const buf = Buffer.from(qbeIL, "utf-8");
  const result = symbols.mog_qbe_compile(ptr(buf), buf.length);
  if (result === null || result === 0) {
    throw new Error("mog_qbe_compile failed (returned NULL)");
  }
  // result is a char* allocated by open_memstream — read it as a string
  // CString clones the data so it's safe even if we don't free immediately
  return new CString(result) as unknown as string;
}

/**
 * Compile QBE IL to an object file on disk (QBE in-process + spawns `as`).
 * Throws on failure.
 */
export function qbeAssemble(qbeIL: string, outputObjPath: string): void {
  const ilBuf = Buffer.from(qbeIL, "utf-8");
  const outBuf = Buffer.from(outputObjPath + "\0", "utf-8");
  const rc = symbols.mog_assemble(ptr(ilBuf), ilBuf.length, ptr(outBuf));
  if (rc !== 0) {
    throw new Error(`mog_assemble failed with code ${rc}`);
  }
}

/**
 * Compile QBE IL and link into a binary on disk (QBE in-process + as + ld).
 * extraObjects is an array of additional .o or .a file paths to link.
 * Throws on failure.
 */
export function qbeCompileAndLink(
  qbeIL: string,
  outputPath: string,
  extraObjects: string[] = [],
): void {
  const ilBuf = Buffer.from(qbeIL, "utf-8");
  const outBuf = Buffer.from(outputPath + "\0", "utf-8");

  // Build the const char** array for extra_objects.
  // Each entry is a pointer to a null-terminated string buffer.
  // We must keep references to the buffers so they aren't GC'd.
  const strBufs: Buffer[] = extraObjects.map((s) => Buffer.from(s + "\0", "utf-8"));

  let ptrArrayPtr: ReturnType<typeof ptr> | 0 = 0;
  let ptrArrayBuf: Buffer | null = null;

  if (strBufs.length > 0) {
    // Each pointer is 8 bytes (64-bit).
    ptrArrayBuf = Buffer.alloc(strBufs.length * 8);
    for (let i = 0; i < strBufs.length; i++) {
      ptrArrayBuf.writeBigUInt64LE(BigInt(ptr(strBufs[i])), i * 8);
    }
    ptrArrayPtr = ptr(ptrArrayBuf);
  }

  const rc = symbols.mog_compile_and_link(
    ptr(ilBuf),
    ilBuf.length,
    ptr(outBuf),
    ptrArrayPtr,
    strBufs.length,
  );

  if (rc !== 0) {
    throw new Error(`mog_compile_and_link failed with code ${rc}`);
  }
}
