// Mog POSIX host — registers 'fs' and 'process' capabilities.
//
// Port of runtime/posix_host.c.  Provides file I/O, process control, and
// environment access through the capability system.

use std::ptr;

use crate::gc::gc_external_alloc;

// ---------------------------------------------------------------------------
// MogValue — 24-byte tagged union matching the C layout.
//
//  offset 0: tag (i32)   — 0=INT, 1=FLOAT, 2=BOOL, 3=STRING, 4=NONE,
//                           5=HANDLE, 6=ERROR
//  offset 4: (padding)
//  offset 8: data (16 bytes, largest member is handle = 2 pointers)
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MogValue {
    pub tag: i32,
    _pad: i32,
    pub data: [u8; 16], // interpreted per tag
}

const MOG_TAG_INT: i32 = 0;
const _MOG_TAG_FLOAT: i32 = 1;
const MOG_TAG_BOOL: i32 = 2;
const MOG_TAG_STRING: i32 = 3;
const _MOG_TAG_NONE: i32 = 4;
#[allow(dead_code)]
const MOG_TAG_HANDLE: i32 = 5;
const MOG_TAG_ERROR: i32 = 6;

impl MogValue {
    fn int(v: i64) -> Self {
        let mut val = MogValue {
            tag: MOG_TAG_INT,
            _pad: 0,
            data: [0u8; 16],
        };
        unsafe {
            ptr::copy_nonoverlapping(&v as *const i64 as *const u8, val.data.as_mut_ptr(), 8);
        }
        val
    }

    fn string(s: *const u8) -> Self {
        let mut val = MogValue {
            tag: MOG_TAG_STRING,
            _pad: 0,
            data: [0u8; 16],
        };
        let p = s as usize;
        unsafe {
            ptr::copy_nonoverlapping(
                &p as *const usize as *const u8,
                val.data.as_mut_ptr(),
                std::mem::size_of::<usize>(),
            );
        }
        val
    }

    fn error(msg: *const u8) -> Self {
        let mut val = MogValue {
            tag: MOG_TAG_ERROR,
            _pad: 0,
            data: [0u8; 16],
        };
        let p = msg as usize;
        unsafe {
            ptr::copy_nonoverlapping(
                &p as *const usize as *const u8,
                val.data.as_mut_ptr(),
                std::mem::size_of::<usize>(),
            );
        }
        val
    }

    fn bool_val(b: bool) -> Self {
        let mut val = MogValue {
            tag: MOG_TAG_BOOL,
            _pad: 0,
            data: [0u8; 16],
        };
        let v: i64 = if b { 1 } else { 0 };
        unsafe {
            ptr::copy_nonoverlapping(&v as *const i64 as *const u8, val.data.as_mut_ptr(), 8);
        }
        val
    }

    #[allow(dead_code)]
    fn none() -> Self {
        MogValue {
            tag: _MOG_TAG_NONE,
            _pad: 0,
            data: [0u8; 16],
        }
    }

    /// Extract i64 from data field (offset 8 of the value).
    fn as_int(&self) -> i64 {
        let mut v: i64 = 0;
        unsafe {
            ptr::copy_nonoverlapping(self.data.as_ptr(), &mut v as *mut i64 as *mut u8, 8);
        }
        v
    }

    /// Extract *const u8 (string pointer) from data field.
    fn as_string_ptr(&self) -> *const u8 {
        let mut p: usize = 0;
        unsafe {
            ptr::copy_nonoverlapping(
                self.data.as_ptr(),
                &mut p as *mut usize as *mut u8,
                std::mem::size_of::<usize>(),
            );
        }
        p as *const u8
    }
}

// ---------------------------------------------------------------------------
// Static error message strings (must live for 'static)
// ---------------------------------------------------------------------------

macro_rules! static_cstr {
    ($name:ident, $s:expr) => {
        static $name: &[u8] = concat!($s, "\0").as_bytes();
    };
}

static_cstr!(ERR_FS_READ_MISSING, "fs.read_file: missing path");
static_cstr!(ERR_FS_READ_BAD_PATH, "fs.read_file: path must be a string");
static_cstr!(ERR_FS_READ_OPEN, "fs.read_file: cannot open file");
static_cstr!(ERR_FS_READ_OOM, "fs.read_file: out of memory");
static_cstr!(ERR_FS_WRITE_MISSING, "fs.write_file: missing args");
static_cstr!(ERR_FS_WRITE_INVALID, "fs.write_file: invalid args");
static_cstr!(ERR_FS_APPEND_MISSING, "fs.append_file: missing args");
static_cstr!(ERR_FS_APPEND_INVALID, "fs.append_file: invalid args");
static_cstr!(ERR_FS_EXISTS_MISSING, "fs.exists: missing path");
static_cstr!(ERR_FS_EXISTS_BAD, "fs.exists: path must be a string");
static_cstr!(ERR_FS_REMOVE_MISSING, "fs.remove: missing path");
static_cstr!(ERR_FS_REMOVE_BAD, "fs.remove: path must be a string");
static_cstr!(ERR_FS_SIZE_MISSING, "fs.file_size: missing path");
static_cstr!(ERR_FS_SIZE_BAD, "fs.file_size: path must be a string");
static_cstr!(ERR_PROC_SLEEP_MISSING, "process.sleep: missing ms");
static_cstr!(ERR_PROC_GETENV_MISSING, "process.getenv: missing name");
static_cstr!(ERR_PROC_GETENV_BAD, "process.getenv: name must be a string");
static_cstr!(ERR_PROC_CWD_FAIL, "process.cwd: getcwd failed");
static_cstr!(ERR_PROC_ARG_MISSING, "process.arg: missing index");
static_cstr!(ERR_PROC_ARG_OOB, "process.arg: index out of bounds");
static_cstr!(ERR_PROC_ARG_OOM, "process.arg: out of memory");

// ---------------------------------------------------------------------------
// Argv storage — set once from main(), read by process.argc / process.arg
//
// SAFETY: These are written exactly once by mog_set_argv() (called from the
// generated $main before any user code runs) and only read afterwards.
// The Mog runtime is single-threaded, so no data races are possible.
// ---------------------------------------------------------------------------

static mut MOG_ARGC: i32 = 0;
static mut MOG_ARGV: *const *const u8 = ptr::null();

#[unsafe(no_mangle)]
pub extern "C" fn mog_set_argv(argc: i32, argv: *const *const u8) {
    if argv.is_null() || argc < 0 {
        return;
    }
    unsafe {
        MOG_ARGC = argc;
        MOG_ARGV = argv;
    }
}

// ---------------------------------------------------------------------------
// Capability entry struct — matches the C MogCapEntry layout.
//
// For registration we need:
//   struct MogCapEntry { name: *const u8, func: fn ptr, is_async: i32 }
//
// However the C code uses a simpler 2-field struct terminated by {NULL,NULL}.
// We match the actual C struct from posix_host.c which has { name, func }.
// ---------------------------------------------------------------------------

/// Capability function signature: (vm, args, nargs) -> MogValue
type CapFn = extern "C" fn(*mut u8, *const MogValue, i32) -> MogValue;

#[repr(C)]
struct MogCapEntry {
    name: *const u8,
    func: Option<CapFn>,
}

// SAFETY: MogCapEntry contains only static string pointers and function pointers.
// These are inherently thread-safe (read-only, immutable, 'static lifetime).
unsafe impl Sync for MogCapEntry {}

// ---------------------------------------------------------------------------
// External VM functions we call
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn mog_register_capability(vm: *mut u8, name: *const u8, entries: *const MogCapEntry) -> i32;
    fn mog_has_capability(vm: *mut u8, name: *const u8) -> i32;
    fn mog_loop_get_global() -> *mut u8;
    fn mog_future_new() -> *mut u8;
    fn mog_loop_add_timer(loop_ptr: *mut u8, ms: i64, future: *mut u8);
}

// ---------------------------------------------------------------------------
// Helper: extract string arg from the MogValue args array.
// ---------------------------------------------------------------------------

unsafe fn arg_string(args: *const MogValue, index: i32, nargs: i32) -> *const libc::c_char {
    unsafe {
        if index >= nargs {
            return ptr::null();
        }
        let val = &*args.add(index as usize);
        if val.tag != MOG_TAG_STRING {
            return ptr::null();
        }
        val.as_string_ptr() as *const libc::c_char
    }
}

unsafe fn arg_int(args: *const MogValue, index: i32, nargs: i32) -> i64 {
    unsafe {
        if index >= nargs {
            return 0;
        }
        let val = &*args.add(index as usize);
        val.as_int()
    }
}

// ---------------------------------------------------------------------------
// fs capability functions
// ---------------------------------------------------------------------------

extern "C" fn fs_read_file(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_FS_READ_MISSING.as_ptr());
    }
    unsafe {
        let path = arg_string(args, 0, nargs);
        if path.is_null() {
            return MogValue::error(ERR_FS_READ_BAD_PATH.as_ptr());
        }

        let mode = b"rb\0".as_ptr() as *const libc::c_char;
        let f = libc::fopen(path, mode);
        if f.is_null() {
            return MogValue::error(ERR_FS_READ_OPEN.as_ptr());
        }

        libc::fseek(f, 0, libc::SEEK_END);
        let len = libc::ftell(f) as usize;
        libc::fseek(f, 0, libc::SEEK_SET);

        let buf = gc_external_alloc(len + 1);
        if buf.is_null() {
            libc::fclose(f);
            return MogValue::error(ERR_FS_READ_OOM.as_ptr());
        }

        let nread = libc::fread(buf as *mut libc::c_void, 1, len, f);
        *buf.add(nread) = 0;
        libc::fclose(f);

        MogValue::string(buf)
    }
}

extern "C" fn fs_write_file(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 2 {
        return MogValue::error(ERR_FS_WRITE_MISSING.as_ptr());
    }
    unsafe {
        let path = arg_string(args, 0, nargs);
        let contents = arg_string(args, 1, nargs);
        if path.is_null() || contents.is_null() {
            return MogValue::error(ERR_FS_WRITE_INVALID.as_ptr());
        }

        let mode = b"wb\0".as_ptr() as *const libc::c_char;
        let f = libc::fopen(path, mode);
        if f.is_null() {
            return MogValue::int(-1);
        }

        let len = libc::strlen(contents);
        let written = libc::fwrite(contents as *const libc::c_void, 1, len, f);
        libc::fclose(f);

        MogValue::int(written as i64)
    }
}

extern "C" fn fs_append_file(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 2 {
        return MogValue::error(ERR_FS_APPEND_MISSING.as_ptr());
    }
    unsafe {
        let path = arg_string(args, 0, nargs);
        let contents = arg_string(args, 1, nargs);
        if path.is_null() || contents.is_null() {
            return MogValue::error(ERR_FS_APPEND_INVALID.as_ptr());
        }

        let mode = b"ab\0".as_ptr() as *const libc::c_char;
        let f = libc::fopen(path, mode);
        if f.is_null() {
            return MogValue::int(-1);
        }

        let len = libc::strlen(contents);
        let written = libc::fwrite(contents as *const libc::c_void, 1, len, f);
        libc::fclose(f);

        MogValue::int(written as i64)
    }
}

extern "C" fn fs_exists(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_FS_EXISTS_MISSING.as_ptr());
    }
    unsafe {
        let path = arg_string(args, 0, nargs);
        if path.is_null() {
            return MogValue::error(ERR_FS_EXISTS_BAD.as_ptr());
        }
        MogValue::bool_val(libc::access(path, libc::F_OK) == 0)
    }
}

extern "C" fn fs_remove(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_FS_REMOVE_MISSING.as_ptr());
    }
    unsafe {
        let path = arg_string(args, 0, nargs);
        if path.is_null() {
            return MogValue::error(ERR_FS_REMOVE_BAD.as_ptr());
        }
        let result = libc::remove(path);
        MogValue::int(if result == 0 { 0 } else { -1 })
    }
}

extern "C" fn fs_file_size(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_FS_SIZE_MISSING.as_ptr());
    }
    unsafe {
        let path = arg_string(args, 0, nargs);
        if path.is_null() {
            return MogValue::error(ERR_FS_SIZE_BAD.as_ptr());
        }
        let mut st: libc::stat = std::mem::zeroed();
        if libc::stat(path, &mut st) != 0 {
            return MogValue::int(-1);
        }
        MogValue::int(st.st_size as i64)
    }
}

// ---------------------------------------------------------------------------
// process capability functions
// ---------------------------------------------------------------------------

extern "C" fn process_sleep(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_PROC_SLEEP_MISSING.as_ptr());
    }
    unsafe {
        let ms = arg_int(args, 0, nargs);
        let loop_ptr = mog_loop_get_global();
        if !loop_ptr.is_null() {
            // Truly async: create future, add timer
            let future = mog_future_new();
            mog_loop_add_timer(loop_ptr, ms, future);
            // Return the future pointer as an int
            MogValue::int(future as i64)
        } else {
            // No event loop: synchronous sleep
            let ts = libc::timespec {
                tv_sec: (ms / 1000) as libc::time_t,
                tv_nsec: ((ms % 1000) * 1_000_000) as libc::c_long,
            };
            libc::nanosleep(&ts, ptr::null_mut());
            MogValue::int(0)
        }
    }
}

extern "C" fn process_timestamp(_vm: *mut u8, _args: *const MogValue, _nargs: i32) -> MogValue {
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    unsafe {
        libc::clock_gettime(libc::CLOCK_REALTIME, &mut ts);
    }
    let ms = ts.tv_sec as i64 * 1000 + ts.tv_nsec as i64 / 1_000_000;
    MogValue::int(ms)
}

extern "C" fn process_getenv(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_PROC_GETENV_MISSING.as_ptr());
    }
    unsafe {
        let name = arg_string(args, 0, nargs);
        if name.is_null() {
            return MogValue::error(ERR_PROC_GETENV_BAD.as_ptr());
        }
        let val = libc::getenv(name);
        if val.is_null() {
            return MogValue::string(b"\0".as_ptr());
        }
        MogValue::string(val as *const u8)
    }
}

extern "C" fn process_cwd(_vm: *mut u8, _args: *const MogValue, _nargs: i32) -> MogValue {
    let mut buf = [0u8; 4096];
    unsafe {
        let result = libc::getcwd(buf.as_mut_ptr() as *mut libc::c_char, buf.len());
        if !result.is_null() {
            // We need to return a pointer that outlives this stack frame.
            // Copy into a host-managed buffer.
            let len = libc::strlen(buf.as_ptr() as *const libc::c_char);
            let dest = gc_external_alloc(len + 1);
            if !dest.is_null() {
                ptr::copy_nonoverlapping(buf.as_ptr(), dest, len + 1);
                return MogValue::string(dest);
            }
        }
    }
    MogValue::error(ERR_PROC_CWD_FAIL.as_ptr())
}

extern "C" fn process_exit(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    unsafe {
        let code = if nargs > 0 {
            arg_int(args, 0, nargs)
        } else {
            0
        };
        libc::exit(code as i32);
    }
}

extern "C" fn process_argc(_vm: *mut u8, _args: *const MogValue, _nargs: i32) -> MogValue {
    unsafe { MogValue::int(MOG_ARGC as i64) }
}

extern "C" fn process_arg(_vm: *mut u8, args: *const MogValue, nargs: i32) -> MogValue {
    if nargs < 1 {
        return MogValue::error(ERR_PROC_ARG_MISSING.as_ptr());
    }
    unsafe {
        let index = arg_int(args, 0, nargs);
        if index < 0 || index >= MOG_ARGC as i64 {
            return MogValue::error(ERR_PROC_ARG_OOB.as_ptr());
        }
        let c_str = *MOG_ARGV.add(index as usize) as *const libc::c_char;
        if c_str.is_null() {
            return MogValue::error(ERR_PROC_ARG_OOB.as_ptr());
        }
        let len = libc::strlen(c_str);
        let dest = gc_external_alloc(len + 1);
        if dest.is_null() {
            return MogValue::error(ERR_PROC_ARG_OOM.as_ptr());
        }
        ptr::copy_nonoverlapping(c_str as *const u8, dest, len + 1);
        MogValue::string(dest)
    }
}

// ---------------------------------------------------------------------------
// Capability entry tables — null-terminated, matching C convention.
// ---------------------------------------------------------------------------

static FS_ENTRIES: &[MogCapEntry] = &[
    MogCapEntry {
        name: b"read_file\0".as_ptr(),
        func: Some(fs_read_file),
    },
    MogCapEntry {
        name: b"write_file\0".as_ptr(),
        func: Some(fs_write_file),
    },
    MogCapEntry {
        name: b"append_file\0".as_ptr(),
        func: Some(fs_append_file),
    },
    MogCapEntry {
        name: b"exists\0".as_ptr(),
        func: Some(fs_exists),
    },
    MogCapEntry {
        name: b"remove\0".as_ptr(),
        func: Some(fs_remove),
    },
    MogCapEntry {
        name: b"file_size\0".as_ptr(),
        func: Some(fs_file_size),
    },
    MogCapEntry {
        name: ptr::null(),
        func: None,
    },
];

static PROCESS_ENTRIES: &[MogCapEntry] = &[
    MogCapEntry {
        name: b"sleep\0".as_ptr(),
        func: Some(process_sleep),
    },
    MogCapEntry {
        name: b"timestamp\0".as_ptr(),
        func: Some(process_timestamp),
    },
    MogCapEntry {
        name: b"getenv\0".as_ptr(),
        func: Some(process_getenv),
    },
    MogCapEntry {
        name: b"cwd\0".as_ptr(),
        func: Some(process_cwd),
    },
    MogCapEntry {
        name: b"exit\0".as_ptr(),
        func: Some(process_exit),
    },
    MogCapEntry {
        name: b"argc\0".as_ptr(),
        func: Some(process_argc),
    },
    MogCapEntry {
        name: b"arg\0".as_ptr(),
        func: Some(process_arg),
    },
    MogCapEntry {
        name: ptr::null(),
        func: None,
    },
];

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_register_posix_host(vm: *mut u8) {
    unsafe {
        // Only register if not already present (idempotent)
        if mog_has_capability(vm, b"fs\0".as_ptr()) == 0 {
            mog_register_capability(vm, b"fs\0".as_ptr(), FS_ENTRIES.as_ptr());
        }
        if mog_has_capability(vm, b"process\0".as_ptr()) == 0 {
            mog_register_capability(vm, b"process\0".as_ptr(), PROCESS_ENTRIES.as_ptr());
        }
    }
}
