### POSIX Filesystem Operations

POSIX (Portable Operating System Interface) defines a set of standard system calls for interacting with files and directories in Unix-like operating systems. These operations are primarily found in headers like `<unistd.h>`, `<fcntl.h>`, `<sys/stat.h>`, and `<dirent.h>`. They handle tasks such as creating, opening, reading, writing, and managing files and directories.

Below is a comprehensive list of key POSIX filesystem operations, grouped by category for clarity. This is based on the POSIX.1 standard (IEEE Std 1003.1). Note that some operations may have variants (e.g., for file descriptors vs. paths), and error handling is typically done via `errno`.

#### File Creation and Opening
- `creat(path, mode)`: Creates a new file or truncates an existing one, returning a file descriptor.
- `open(path, flags, [mode])`: Opens a file and returns a file descriptor; supports flags like O_RDONLY, O_WRONLY, O_RDWR, O_CREAT, O_TRUNC, etc.
- `mkfifo(path, mode)`: Creates a FIFO special file (named pipe).
- `mknod(path, mode, dev)`: Creates a special file (e.g., character or block device).

#### File I/O
- `read(fd, buf, count)`: Reads data from a file descriptor into a buffer.
- `write(fd, buf, count)`: Writes data from a buffer to a file descriptor.
- `pread(fd, buf, count, offset)`: Reads from a specific offset (thread-safe variant).
- `pwrite(fd, buf, count, offset)`: Writes to a specific offset (thread-safe variant).
- `lseek(fd, offset, whence)`: Repositions the file offset for the next read/write.

#### File Closing and Synchronization
- `close(fd)`: Closes a file descriptor.
- `fsync(fd)`: Synchronizes file data and metadata to storage.
- `fdatasync(fd)`: Synchronizes only file data to storage (faster than fsync).

#### File Status and Metadata
- `stat(path, buf)`: Retrieves file status information (e.g., size, permissions, timestamps).
- `lstat(path, buf)`: Like stat, but doesn't follow symbolic links.
- `fstat(fd, buf)`: Retrieves status for an open file descriptor.
- `access(path, mode)`: Checks file accessibility (e.g., R_OK for read permission).
- `faccessat(dirfd, path, mode, flags)`: Relative access check from a directory file descriptor.
- `utimes(path, times)`: Changes access and modification times.
- `futimes(fd, times)`: Changes times for an open file descriptor.
- `utimensat(dirfd, path, times, flags)`: High-precision time changes.

#### File Permissions and Ownership
- `chmod(path, mode)`: Changes file permissions.
- `fchmod(fd, mode)`: Changes permissions for an open file descriptor.
- `chown(path, owner, group)`: Changes file owner and group.
- `fchown(fd, owner, group)`: Changes owner/group for an open file descriptor.
- `umask(mask)`: Sets the file creation mask for new files.

#### File Truncation and Resizing
- `truncate(path, length)`: Truncates a file to a specified length.
- `ftruncate(fd, length)`: Truncates an open file to a specified length.

#### Linking and Renaming
- `link(oldpath, newpath)`: Creates a hard link to a file.
- `symlink(oldpath, newpath)`: Creates a symbolic link.
- `readlink(path, buf, bufsiz)`: Reads the target of a symbolic link.
- `rename(oldpath, newpath)`: Renames or moves a file/directory.
- `unlink(path)`: Removes a file (deletes a directory entry).

#### Directory Operations
- `mkdir(path, mode)`: Creates a new directory.
- `rmdir(path)`: Removes an empty directory.
- `opendir(path)`: Opens a directory stream.
- `readdir(dirp)`: Reads the next directory entry.
- `rewinddir(dirp)`: Resets the directory stream to the beginning.
- `closedir(dirp)`: Closes a directory stream.
- `chdir(path)`: Changes the current working directory.
- `fchdir(fd)`: Changes directory using a file descriptor.
- `getcwd(buf, size)`: Gets the current working directory path.

#### Advanced/Configuration
- `fcntl(fd, cmd, [arg])`: Performs control operations (e.g., locking, duplicating descriptors).
- `pathconf(path, name)`: Gets configuration values for a file/path (e.g., maximum link count).
- `fpathconf(fd, name)`: Gets configuration for an open file descriptor.
- `dup(fd)`: Duplicates a file descriptor.
- `dup2(fd, newfd)`: Duplicates to a specific new descriptor.

These operations form the core of POSIX filesystem interaction. For detailed specifications, including error codes and behaviors, refer to the official POSIX documentation from The Open Group. Implementations may vary slightly across systems (e.g., Linux, macOS, FreeBSD), but they adhere to this standard for portability. If you need examples or details on a specific operation, let me know!