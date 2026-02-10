# POSIX Filesystem Operations Implementation Review

## Summary

This document summarizes the current state of POSIX filesystem operations in the Mog compiler.

## Changes Made

### 1. Fixed Octal Literal Parsing (parser.ts:685-696)
**Problem**: Octal literals like `0644` were being parsed as decimal `644` instead of `420`.

**Solution**: Added detection for octal notation (numbers starting with 0) and use `parseInt(token.value, 8)` for octal, `parseFloat()` for others.

```typescript
if (token.value.startsWith("0") && token.value.length > 1 && !token.value.includes(".")) {
  value = parseInt(token.value, 8)  // Parse as octal
} else {
  value = parseFloat(token.value)
}
```

### 2. Fixed String Constant Generation (llvm_codegen.ts)
**Problem**: String constants were using the same counter as LLVM registers, causing naming conflicts (e.g., `@str10`, `@str18` vs `@str0`, `@str1`).

**Solution**: 
- Added separate `stringCounter` independent of `valueCounter`
- Reset counter between collection and generation phases
- Emit strings before function declarations

### 3. Added POSIX Function Signature Support (llvm_codegen.ts:432-487)
**Problem**: POSIX function calls weren't properly typed, causing LLVM errors like `expected value token`.

**Solution**: Added a `posixSignatures` lookup table that maps function names to their argument types, ensuring proper LLVM IR generation:

```typescript
const posixSignatures: Record<string, string[]> = {
  open: ["ptr", "i64"],
  read: ["i64", "ptr", "i64"],
  write: ["i64", "ptr", "i64"],
  // ... 40+ functions
}
```

### 4. Added Missing POSIX Declarations (llvm_codegen.ts:937-941)
Added LLVM declarations for:
- `pread`, `pwrite` - positioned read/write
- `utimensat` - nanosecond precision timestamps
- `truncate` - truncate by path

## Current Test Status

### Working Tests (Exit Code 0)
| Test | Functions Tested |
|------|-----------------|
| test_posix_cwd.mog | mkdir, chdir, rmdir |
| test_posix_dup.mog | open, dup, dup2, close, unlink |
| test_posix_mkdir.mog | mkdir, rmdir |
| test_posix_permissions.mog | open, chmod, fchmod, chown, fchown, close, unlink |
| test_posix_rename.mog | open, write, close, rename, access, unlink |
| test_posix_special.mog | creat, mkfifo, mknod, unlink |

### Known Issues

#### 1. Variadic Functions on macOS (ARM64)
**Affected**: `open()` with O_CREAT flag, `fcntl()`, `mknod()`

**Issue**: On macOS ARM64, variadic functions require special calling convention handling. The `open()` call works without the mode argument but fails when the variadic mode argument is passed.

**Workaround**: Use `creat()` instead of `open()` when creating files.

**Example**:
```mog
# This fails on macOS:
fd: i64 = open("file.txt", O_CREAT | O_WRONLY, 0644);

# Use this instead:
fd: i64 = creat("file.txt", 0644);
```

#### 2. Test File Issues
Some test files have incorrect usage patterns:

**test_posix_read.mog**, **test_posix_comprehensive.mog**: Use `cast<i64>(buffer)` which produces `i64` but functions expect `ptr`.

**Fix**: Pass arrays directly without cast:
```mog
buf: i64[] = [0, 0, 0, 0, 0];
bytes: i64 = read(fd, buf, 5);  # Pass buf directly, not cast<i64>(buf)
```

**test_posix_directory_read.mog**: Uses `rewinddir(dir)` where `dir` is `i64` but function expects `ptr`.

## Supported POSIX Functions (48 total)

### File Operations
- `open`, `creat`, `close`, `unlink`, `rename`
- `read`, `write`, `pread`, `pwrite`
- `lseek`, `fsync`, `fdatasync`
- `truncate`, `ftruncate`

### Directory Operations
- `mkdir`, `rmdir`, `chdir`, `fchdir`, `getcwd`
- `opendir`, `readdir`, `closedir`, `rewinddir`

### File Metadata
- `stat`, `lstat`, `fstat`
- `chmod`, `fchmod`, `chown`, `fchown`
- `access`, `faccessat`
- `utimes`, `futimes`, `utimensat`

### Links
- `link`, `symlink`, `readlink`

### File Descriptors
- `dup`, `dup2`, `fcntl`

### Special Files
- `mkfifo`, `mknod`

### Path Configuration
- `pathconf`, `fpathconf`

## POSIX Constants (Defined in posix_constants.ts)

### Open Flags
O_RDONLY, O_WRONLY, O_RDWR, O_CREAT, O_EXCL, O_NOCTTY, O_TRUNC, O_APPEND, O_NONBLOCK, O_SYNC, O_ASYNC, O_DIRECTORY, O_NOFOLLOW, O_CLOEXEC

### Access Modes
F_OK, R_OK, W_OK, X_OK

### File Permissions
S_IRUSR, S_IWUSR, S_IXUSR, S_IRGRP, S_IWGRP, S_IXGRP, S_IROTH, S_IWOTH, S_IXOTH, S_IRWXU, S_IRWXG, S_IRWXO

### File Types
S_IFMT, S_IFIFO, S_IFCHR, S_IFDIR, S_IFBLK, S_IFREG, S_IFLNK, S_IFSOCK

### Seek Constants
SEEK_SET, SEEK_CUR, SEEK_END

### Error Codes
EPERM through ERANGE (34 standard errno values)

## Recommendations

1. **For macOS users**: Use `creat()` instead of `open()` with O_CREAT
2. **For buffer passing**: Pass arrays directly without casting
3. **For stat buffers**: Use array syntax: `statbuf: i64[] = [0, 0, ...]` and pass directly to `stat()`

## Files Modified

- `src/parser.ts` - Octal literal parsing
- `src/llvm_codegen.ts` - String constant handling, POSIX signatures, function declarations

## Tests Verified Working

```bash
# Working tests (after fixes)
test_posix_cwd.mog        # mkdir, chdir, rmdir
test_posix_dup.mog        # dup, dup2
test_posix_mkdir.mog      # mkdir, rmdir
test_posix_permissions.mog # chmod, fchmod, chown, fchown
test_posix_rename.mog     # rename, access
test_posix_special.mog    # creat, mkfifo, mknod
```