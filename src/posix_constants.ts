/**
 * POSIX Filesystem Constants
 * These constants map to standard POSIX/C values used with Cosmopolitan Libc
 */

export const POSIX_CONSTANTS: Record<string, number> = {
  // Open flags
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

  // Access modes
  F_OK: 0,
  R_OK: 4,
  W_OK: 2,
  X_OK: 1,

  // File mode bits (permissions)
  S_IRUSR: 256,      // 0400
  S_IWUSR: 128,      // 0200
  S_IXUSR: 64,       // 0100
  S_IRGRP: 32,       // 0040
  S_IWGRP: 16,       // 0020
  S_IXGRP: 8,        // 0010
  S_IROTH: 4,        // 0004
  S_IWOTH: 2,        // 0002
  S_IXOTH: 1,        // 0001
  S_IRWXU: 448,      // 0700
  S_IRWXG: 56,       // 0070
  S_IRWXO: 7,        // 0007

  // File type bits
  S_IFMT: 61440,     // 0170000
  S_IFIFO: 4096,     // 0010000
  S_IFCHR: 8192,     // 0020000
  S_IFDIR: 16384,    // 0040000
  S_IFBLK: 24576,    // 0060000
  S_IFREG: 32768,    // 0100000
  S_IFLNK: 40960,    // 0120000
  S_IFSOCK: 49152,   // 0140000

  // Lseek constants
  SEEK_SET: 0,
  SEEK_CUR: 1,
  SEEK_END: 2,

  // errno values
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

  // Miscellaneous
  FD_CLOEXEC: 1,
  AT_FDCWD: -100,
  AT_SYMLINK_NOFOLLOW: 256,
  AT_REMOVEDIR: 512,

  // Stat constants
  UTIME_NOW: -2,
  UTIME_OMIT: -1,
} as const;

export function isPOSIXConstant(name: string): boolean {
  return name in POSIX_CONSTANTS;
}

export function getPOSIXConstant(name: string): number {
  if (name in POSIX_CONSTANTS) {
    return POSIX_CONSTANTS[name as keyof typeof POSIX_CONSTANTS];
  }
  throw new Error(`Unknown POSIX constant: ${name}`);
}