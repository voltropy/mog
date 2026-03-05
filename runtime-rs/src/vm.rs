// Mog VM runtime — lifecycle, capabilities, value constructors/extractors, interrupt.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_CAPABILITIES: usize = 32;
const MAX_FUNCTIONS_PER_CAP: usize = 64;

// ---------------------------------------------------------------------------
// MogValue — 24-byte tagged union (matches C layout exactly)
// ---------------------------------------------------------------------------

pub const MOG_INT: i32 = 0;
pub const MOG_FLOAT: i32 = 1;
pub const MOG_BOOL: i32 = 2;
pub const MOG_STRING: i32 = 3;
pub const MOG_NONE: i32 = 4;
pub const MOG_HANDLE: i32 = 5;
pub const MOG_ERROR: i32 = 6;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MogHandle {
    pub ptr: *mut u8,
    pub type_name: *const u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union MogValueData {
    pub i: i64,
    pub f: f64,
    pub b: i64, // bool stored as i64 for ABI stability
    pub s: *const u8,
    pub handle: MogHandle,
    pub error: *const u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MogValue {
    pub tag: i32,
    pub _pad: i32,
    pub data: MogValueData,
}

impl MogValue {
    fn _zeroed() -> Self {
        MogValue {
            tag: MOG_NONE,
            _pad: 0,
            data: MogValueData { i: 0 },
        }
    }
}

// ---------------------------------------------------------------------------
// MogCapEntry — function pointer table entry (matches C layout)
// ---------------------------------------------------------------------------

/// Host function signature: fn(vm, args, nargs) -> MogValue
pub type MogHostFn = extern "C" fn(*mut u8, *const MogValue, i32) -> MogValue;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MogCapEntry {
    pub name: *const u8,
    pub func: MogHostFn,
}

// ---------------------------------------------------------------------------
// MogLimits
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MogLimits {
    pub max_memory: usize,
    pub max_cpu_ms: i32,
    pub max_stack_depth: i32,
}

// ---------------------------------------------------------------------------
// Internal capability / VM structs
// ---------------------------------------------------------------------------

struct MogCapability {
    name: [u8; 128],
    functions: [MogCapEntryInternal; MAX_FUNCTIONS_PER_CAP],
    func_count: i32,
}

#[derive(Copy, Clone)]
struct MogCapEntryInternal {
    name: *const u8,
    func: Option<MogHostFn>,
}

impl Default for MogCapEntryInternal {
    fn default() -> Self {
        MogCapEntryInternal {
            name: std::ptr::null(),
            func: None,
        }
    }
}

impl Default for MogCapability {
    fn default() -> Self {
        MogCapability {
            name: [0u8; 128],
            functions: [MogCapEntryInternal::default(); MAX_FUNCTIONS_PER_CAP],
            func_count: 0,
        }
    }
}

pub struct MogVM {
    capabilities: [MogCapability; MAX_CAPABILITIES],
    cap_count: i32,
    limits: MogLimits,
}

// ---------------------------------------------------------------------------
// Global VM pointer
// ---------------------------------------------------------------------------

static mut G_MOG_VM: *mut MogVM = std::ptr::null_mut();

// ---------------------------------------------------------------------------
// VM lifecycle
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_vm_new() -> *mut u8 {
    let vm = Box::new(MogVM {
        capabilities: std::array::from_fn(|_| MogCapability::default()),
        cap_count: 0,
        limits: MogLimits {
            max_memory: 0,
            max_cpu_ms: 0,
            max_stack_depth: 1024,
        },
    });
    Box::into_raw(vm) as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_vm_free(vm: *mut u8) {
    if !vm.is_null() {
        unsafe {
            drop(Box::from_raw(vm as *mut MogVM));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_vm_set_global(vm: *mut u8) {
    unsafe {
        G_MOG_VM = vm as *mut MogVM;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_vm_get_global() -> *mut u8 {
    unsafe { G_MOG_VM as *mut u8 }
}

// ---------------------------------------------------------------------------
// Capability registration
// ---------------------------------------------------------------------------

/// Register a capability. `entries` is a pointer to an array of MogCapEntry;
/// the C convention uses a NULL-sentinel name to mark the end, but the user
/// spec also passes an explicit `count`. We honour `count` if > 0, otherwise
/// scan for sentinel.
#[unsafe(no_mangle)]
pub extern "C" fn mog_register_capability(
    vm: *mut u8,
    name: *const u8,
    entries: *const MogCapEntry,
) -> i32 {
    if vm.is_null() || name.is_null() || entries.is_null() {
        return -1;
    }
    let vm = unsafe { &mut *(vm as *mut MogVM) };
    if vm.cap_count as usize >= MAX_CAPABILITIES {
        eprintln!("mog: max capabilities ({MAX_CAPABILITIES}) exceeded");
        return -1;
    }

    let cap = &mut vm.capabilities[vm.cap_count as usize];

    // Copy capability name
    unsafe {
        let name_len = libc::strlen(name as *const libc::c_char);
        let copy_len = name_len.min(127);
        std::ptr::copy_nonoverlapping(name, cap.name.as_mut_ptr(), copy_len);
        cap.name[copy_len] = 0;
    }

    cap.func_count = 0;

    // Scan for NULL sentinel to determine entry count
    let n = {
        let mut n = 0usize;
        unsafe {
            while !(*entries.add(n)).name.is_null() {
                n += 1;
            }
        }
        n
    };

    for i in 0..n {
        if cap.func_count as usize >= MAX_FUNCTIONS_PER_CAP {
            eprintln!("mog: max functions per capability ({MAX_FUNCTIONS_PER_CAP}) exceeded");
            return -1;
        }
        let src = unsafe { &*entries.add(i) };
        let dst = &mut cap.functions[cap.func_count as usize];
        dst.name = src.name;
        dst.func = Some(src.func);

        cap.func_count += 1;
    }

    vm.cap_count += 1;
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_has_capability(vm: *mut u8, name: *const u8) -> i32 {
    if vm.is_null() || name.is_null() {
        return 0;
    }
    let vm = unsafe { &*(vm as *const MogVM) };
    for i in 0..vm.cap_count as usize {
        if cap_name_eq(&vm.capabilities[i].name, name) {
            return 1;
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Capability call
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_cap_call(
    vm: *mut u8,
    cap_name: *const u8,
    fn_name: *const u8,
    args: *const MogValue,
    nargs: i32,
) -> MogValue {
    // Fall back to global VM
    let vm_ptr = if vm.is_null() {
        unsafe { G_MOG_VM as *mut u8 }
    } else {
        vm
    };
    if vm_ptr.is_null() {
        return mog_error(b"mog_cap_call: no VM (pass one or call mog_vm_set_global)\0".as_ptr());
    }
    if cap_name.is_null() {
        return mog_error(b"mog_cap_call: NULL cap_name\0".as_ptr());
    }
    if fn_name.is_null() {
        return mog_error(b"mog_cap_call: NULL func_name\0".as_ptr());
    }

    let vm = unsafe { &*(vm_ptr as *const MogVM) };

    for i in 0..vm.cap_count as usize {
        if cap_name_eq(&vm.capabilities[i].name, cap_name) {
            let cap = &vm.capabilities[i];
            for j in 0..cap.func_count as usize {
                if cstr_eq(cap.functions[j].name, fn_name) {
                    if let Some(func) = cap.functions[j].func {
                        return func(vm_ptr, args, nargs);
                    }
                }
            }
            // Function not found
            return mog_error(b"capability function not found\0".as_ptr());
        }
    }

    // Capability not found
    mog_error(b"capability not registered\0".as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_cap_call_out(
    out: *mut MogValue,
    vm: *mut u8,
    cap_name: *const u8,
    fn_name: *const u8,
    args: *const MogValue,
    nargs: i32,
) {
    let result = mog_cap_call(vm, cap_name, fn_name, args, nargs);
    if !out.is_null() {
        unsafe {
            *out = result;
        }
    }
}

// ---------------------------------------------------------------------------
// Value constructors
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_int(v: i64) -> MogValue {
    MogValue {
        tag: MOG_INT,
        _pad: 0,
        data: MogValueData { i: v },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_float(v: f64) -> MogValue {
    MogValue {
        tag: MOG_FLOAT,
        _pad: 0,
        data: MogValueData { f: v },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_bool(v: i32) -> MogValue {
    MogValue {
        tag: MOG_BOOL,
        _pad: 0,
        data: MogValueData {
            b: if v != 0 { 1 } else { 0 },
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_string(s: *const u8) -> MogValue {
    MogValue {
        tag: MOG_STRING,
        _pad: 0,
        data: MogValueData { s },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_none() -> MogValue {
    MogValue {
        tag: MOG_NONE,
        _pad: 0,
        data: MogValueData { i: 0 },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_handle(ptr: *mut u8, type_name: *const u8) -> MogValue {
    MogValue {
        tag: MOG_HANDLE,
        _pad: 0,
        data: MogValueData {
            handle: MogHandle { ptr, type_name },
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_error(msg: *const u8) -> MogValue {
    MogValue {
        tag: MOG_ERROR,
        _pad: 0,
        data: MogValueData { error: msg },
    }
}

// ---------------------------------------------------------------------------
// Arg extractors — args is a raw *const MogValue array, index into it
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_arg_int(args: *const MogValue, index: i32) -> i64 {
    if args.is_null() {
        return 0;
    }
    unsafe {
        let v = &*args.offset(index as isize);
        if v.tag == MOG_INT {
            v.data.i
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_arg_float(args: *const MogValue, index: i32) -> f64 {
    if args.is_null() {
        return 0.0;
    }
    unsafe {
        let v = &*args.offset(index as isize);
        if v.tag == MOG_FLOAT {
            v.data.f
        } else {
            0.0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_arg_bool(args: *const MogValue, index: i32) -> i32 {
    if args.is_null() {
        return 0;
    }
    unsafe {
        let v = &*args.offset(index as isize);
        if v.tag == MOG_BOOL {
            v.data.b as i32
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_arg_string(args: *const MogValue, index: i32) -> *const u8 {
    if args.is_null() {
        return std::ptr::null();
    }
    unsafe {
        let v = &*args.offset(index as isize);
        if v.tag == MOG_STRING {
            v.data.s
        } else {
            std::ptr::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_arg_handle(args: *const MogValue, index: i32) -> *mut u8 {
    if args.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let v = &*args.offset(index as isize);
        if v.tag == MOG_HANDLE {
            v.data.handle.ptr
        } else {
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_arg_error(args: *const MogValue, index: i32) -> *const u8 {
    if args.is_null() {
        return std::ptr::null();
    }
    unsafe {
        let v = &*args.offset(index as isize);
        if v.tag == MOG_ERROR {
            v.data.error
        } else {
            std::ptr::null()
        }
    }
}

// ---------------------------------------------------------------------------
// Direct extractors — from a single MogValue pointer
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_get_int(v: *const MogValue) -> i64 {
    if v.is_null() {
        return 0;
    }
    unsafe {
        let v = &*v;
        if v.tag == MOG_INT {
            v.data.i
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_get_float(v: *const MogValue) -> f64 {
    if v.is_null() {
        return 0.0;
    }
    unsafe {
        let v = &*v;
        if v.tag == MOG_FLOAT {
            v.data.f
        } else {
            0.0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_get_string(v: *const MogValue) -> *const u8 {
    if v.is_null() {
        return std::ptr::null();
    }
    unsafe {
        let v = &*v;
        if v.tag == MOG_STRING {
            v.data.s
        } else {
            std::ptr::null()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_get_bool(v: *const MogValue) -> i32 {
    if v.is_null() {
        return 0;
    }
    unsafe {
        let v = &*v;
        if v.tag == MOG_BOOL {
            v.data.b as i32
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Result helpers on MogValue
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_is_error(v: *const MogValue) -> i32 {
    if v.is_null() {
        return 0;
    }
    unsafe {
        if (*v).tag == MOG_ERROR {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_ok(v: MogValue) -> MogValue {
    // Pass-through: an "ok" result is just the value itself
    v
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_err(msg: *const u8) -> MogValue {
    mog_error(msg)
}

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_vm_set_limits(vm: *mut u8, limits: *const MogLimits) {
    if vm.is_null() || limits.is_null() {
        return;
    }
    unsafe {
        let vm = &mut *(vm as *mut MogVM);
        vm.limits = *limits;
        if vm.limits.max_cpu_ms > 0 {
            mog_arm_timeout(vm.limits.max_cpu_ms);
        }
    }
}

// ---------------------------------------------------------------------------
// Cooperative interrupt
// ---------------------------------------------------------------------------
//
// The interrupt flag MUST be a global `i32` symbol named `mog_interrupt_flag`
// because compiled Mog code references it as:
//     @mog_interrupt_flag = external global i32
//
// We use an AtomicI32 for thread safety but expose the raw storage so the
// linker resolves the IR reference.

#[unsafe(no_mangle)]
pub static mog_interrupt_flag: AtomicI32 = AtomicI32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn mog_request_interrupt() {
    mog_interrupt_flag.store(1, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_clear_interrupt() {
    mog_interrupt_flag.store(0, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_interrupt_requested() -> i32 {
    mog_interrupt_flag.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Timeout thread
// ---------------------------------------------------------------------------

static TIMEOUT_ACTIVE: AtomicBool = AtomicBool::new(false);
static TIMEOUT_MS: AtomicI32 = AtomicI32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn mog_arm_timeout(ms: i32) {
    if ms <= 0 {
        return;
    }
    // Cancel any existing timeout
    mog_cancel_timeout();
    mog_clear_interrupt();

    TIMEOUT_MS.store(ms, Ordering::Relaxed);
    TIMEOUT_ACTIVE.store(true, Ordering::Relaxed);

    std::thread::spawn(move || {
        let mut remaining = ms;
        while remaining > 0 && TIMEOUT_ACTIVE.load(Ordering::Relaxed) {
            let chunk = if remaining > 10 { 10 } else { remaining };
            std::thread::sleep(std::time::Duration::from_millis(chunk as u64));
            remaining -= chunk;
        }
        if TIMEOUT_ACTIVE.load(Ordering::Relaxed) {
            mog_request_interrupt();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_cancel_timeout() {
    TIMEOUT_ACTIVE.store(false, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Helpers (internal)
// ---------------------------------------------------------------------------

/// Compare a fixed-size capability name buffer against a C string.
fn cap_name_eq(buf: &[u8; 128], cstr: *const u8) -> bool {
    unsafe {
        let len = libc::strlen(cstr as *const libc::c_char);
        // Find length of buf (null-terminated)
        let buf_len = buf.iter().position(|&b| b == 0).unwrap_or(128);
        if len != buf_len {
            return false;
        }
        &buf[..buf_len] == std::slice::from_raw_parts(cstr, len)
    }
}

/// Compare two C strings for equality.
fn cstr_eq(a: *const u8, b: *const u8) -> bool {
    if a.is_null() || b.is_null() {
        return a == b;
    }
    unsafe { libc::strcmp(a as *const libc::c_char, b as *const libc::c_char) == 0 }
}
