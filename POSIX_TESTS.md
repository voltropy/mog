# AlgolScript POSIX Test Suite Summary

## Overview

This document provides a comprehensive summary of the 17 POSIX test files in the AlgolScript test suite. These tests validate the POSIX filesystem and I/O bindings available in AlgolScript.

**Total Tests:** 17 test files  
**Test Categories:** File I/O, Directory Operations, File Metadata, Links, Permissions, Process Control

---

## Test File Details

### 1. test_posix_basic.algol
**Purpose:** Basic POSIX file operations

**Functions Tested:**
- `open()` - Open/create files with flags (O_CREAT, O_WRONLY)
- `write()` - Write data to file descriptors
- `close()` - Close file descriptors

**Status:** ✅ Compiles and passes

**Notes:** Simple smoke test for basic POSIX functionality.

---

### 2. test_posix_read.algol
**Purpose:** POSIX read operations

**Functions Tested:**
- `open()` - Open files for reading (O_RDONLY)
- `read()` - Read data from file descriptors into buffers
- `close()` - Close file descriptors

**Status:** ✅ Compiles and passes

**Notes:** Tests reading data using i64 arrays as byte buffers.

---

### 3. test_posix_seek_sync.algol
**Purpose:** File positioning and synchronization

**Functions Tested:**
- `lseek()` - Seek to positions in files (SEEK_SET=0, SEEK_CUR=1, SEEK_END=2)
- `fsync()` - Synchronize file data to disk
- `open()`, `write()`, `close()`, `unlink()`

**Status:** ✅ Compiles and passes

**Notes:** Tests all three seek modes: SEEK_SET (absolute), SEEK_CUR (relative), SEEK_END (from end).

---

### 4. test_posix_dup.algol
**Purpose:** File descriptor duplication

**Functions Tested:**
- `dup()` - Duplicate file descriptor to lowest available fd
- `dup2()` - Duplicate file descriptor to specific fd number

**Status:** ✅ Compiles and passes

**Notes:** Validates that writes through duplicated fds affect the same underlying file.

---

### 5. test_posix_truncate.algol
**Purpose:** File truncation operations

**Functions Tested:**
- `truncate()` - Truncate file by path
- `ftruncate()` - Truncate file by file descriptor

**Status:** ✅ Compiles and passes

**Notes:** Tests both path-based and fd-based truncation.

---

### 6. test_posix_time.algol
**Purpose:** File timestamp operations

**Functions Tested:**
- `utimes()` - Set access/modification times by path
- `futimes()` - Set access/modification times by file descriptor

**Status:** ✅ Compiles and passes

**Notes:** Uses timeval arrays [sec1, usec1, sec2, usec2] for atime/mtime.

---

### 7. test_posix_stat.algol
**Purpose:** File metadata retrieval

**Functions Tested:**
- `stat()` - Get file stats by path
- `lstat()` - Get file stats (don't follow symlinks)
- `fstat()` - Get file stats by file descriptor
- `symlink()` - Create symbolic links

**Status:** ✅ Compiles and passes

**Notes:** Uses large i64 arrays (51 elements) for stat buffer structure. Tests difference between stat and lstat on symlinks.

---

### 8. test_posix_access.algol
**Purpose:** File accessibility checks

**Functions Tested:**
- `access()` - Check file accessibility (F_OK=0, R_OK=4, W_OK=2, X_OK=1)
- `faccessat()` - Check accessibility relative to directory fd

**Status:** ✅ Compiles and passes

**Notes:** Tests both existence checks (F_OK) and permission checks. Uses AT_FDCWD=-100 for faccessat.

---

### 9. test_posix_permissions.algol
**Purpose:** File permission operations

**Functions Tested:**
- `chmod()` - Change mode by path
- `fchmod()` - Change mode by file descriptor
- `chown()` - Change owner by path (with -1 for no change)
- `fchown()` - Change owner by file descriptor

**Status:** ✅ Compiles and passes

**Notes:** Uses -1 for uid/gid to test without changing ownership. Tests both path-based and fd-based operations.

---

### 10. test_posix_links.algol
**Purpose:** File linking operations

**Functions Tested:**
- `link()` - Create hard links
- `symlink()` - Create symbolic links

**Status:** ✅ Compiles and passes

**Notes:** Tests both hard links (same inode) and symbolic links (path reference).

---

### 11. test_posix_rename.algol
**Purpose:** File renaming

**Functions Tested:**
- `rename()` - Rename/move files
- `access()` - Verify rename success

**Status:** ✅ Compiles and passes

**Notes:** Validates that source no longer exists and target exists after rename.

---

### 12. test_posix_mkdir.algol
**Purpose:** Directory creation and removal

**Functions Tested:**
- `mkdir()` - Create directories with permissions
- `rmdir()` - Remove empty directories

**Status:** ✅ Compiles and passes

**Notes:** Simple test for directory lifecycle.

---

### 13. test_posix_directory_read.algol
**Purpose:** Directory traversal

**Functions Tested:**
- `opendir()` - Open directory for reading
- `readdir()` - Read directory entries (structure access incomplete)
- `rewinddir()` - Reset directory stream position
- `closedir()` - Close directory stream

**Status:** ⚠️ Partial - compiles but entry iteration is incomplete

**Notes:** Currently has placeholder loops for directory entry iteration. The dirent structure access pattern needs implementation.

---

### 14. test_posix_cwd.algol
**Purpose:** Current working directory operations

**Functions Tested:**
- `chdir()` - Change current directory

**Status:** ✅ Compiles and passes

**Notes:** Tests relative path navigation. Note: `getcwd()` and `fchdir()` bindings incomplete.

---

### 15. test_posix_control.algol
**Purpose:** File and path configuration

**Functions Tested:**
- `fcntl()` - File control operations (F_GETFD=1 tested)
- `pathconf()` - Get path configuration values
- `fpathconf()` - Get file descriptor configuration values

**Status:** ✅ Compiles and passes

**Notes:** Uses command 1 (F_GETFD) for fcntl test.

---

### 16. test_posix_special.algol
**Purpose:** Special file creation

**Functions Tested:**
- `creat()` - Create file with specific permissions (equivalent to open with O_CREAT|O_WRONLY|O_TRUNC)
- `mkfifo()` - Create named pipes
- `mknod()` - Create special files

**Status:** ✅ Compiles and passes

**Notes:** Tests special file types. mkfifo and mknod may fail on some systems; test handles gracefully.

---

### 17. test_posix_comprehensive.algol
**Purpose:** Integration test combining multiple operations

**Functions Tested:**
- `open()`, `write()`, `read()`, `close()`
- `mkdir()`, `rmdir()`
- `unlink()`

**Status:** ✅ Compiles and passes

**Notes:** End-to-end test simulating real-world file operations workflow.

---

## Common Patterns for POSIX Functions

### File Operations Pattern
```algol
fd: i64 = open("filename", O_CREAT | O_WRONLY, 0644);
if (fd == -1) {
  return error_code;
}
write(fd, "data", 4);
close(fd);
```

### Buffer Declaration Pattern
```algol
# For stat structures (51 i64s for struct stat)
statbuf: i64[] = [0, 0, 0, ...];  # 51 zeros

# For read buffers
buf: i64[] = [0, 0, 0, ...];  # size as needed
```

### Error Handling Pattern
```algol
result: i64 = posix_function(args);
if (result == -1) {
  cleanup_resources();
  return error_code;
}
```

### Permission Constants
- File modes: `0644` (rw-r--r--), `0755` (rwxr-xr-x), `0666` (rw-rw-rw-)
- Access modes: `0` (F_OK/existence), `4` (R_OK/read), `2` (W_OK/write), `1` (X_OK/execute)

### Open Flags (bitwise OR)
- `O_RDONLY` (0) - Read only
- `O_WRONLY` (1) - Write only  
- `O_RDWR` (2) - Read/write
- `O_CREAT` - Create if doesn't exist
- `O_TRUNC` - Truncate existing file
- `O_APPEND` - Append mode

### Seek Whence Values
- `0` (SEEK_SET) - From beginning
- `1` (SEEK_CUR) - From current position
- `2` (SEEK_END) - From end of file

---

## Platform-Specific Notes

### macOS Variadic Function Limitations

**Important:** AlgolScript's POSIX bindings have a known limitation on macOS (ARM64/x86_64) regarding variadic C functions.

**Affected Functions:**
- `open()` - Works (special-cased in binding)
- `fcntl()` - Limited support
- `ioctl()` - Limited support

**Workaround:** Use non-variadic alternatives when available:
- `open()` → `creat()` for simple file creation
- `fcntl(fd, F_GETFD)` → Simplified to single command form

### File Mode Constants
On macOS/Linux, use octal notation for permissions:
- `0644` = Owner read/write, group read, others read
- `0755` = Owner read/write/execute, group read/execute, others read/execute

### Special Files
- `mkfifo()` may require elevated permissions on some systems
- `mknod()` typically requires root privileges for device files

### Directory Operations
- `rmdir()` only works on empty directories
- Directory iteration (`readdir`) requires struct layout knowledge

---

## Summary Statistics

| Category | Count | Status |
|----------|-------|--------|
| Basic I/O | 4 | ✅ All passing |
| File Metadata | 3 | ✅ All passing |
| Links | 2 | ✅ All passing |
| Directories | 3 | 2 passing, 1 partial |
| Permissions | 2 | ✅ All passing |
| Control/Special | 3 | ✅ All passing |

**Overall:** 16/17 tests fully functional, 1 partial (directory entry reading)
