// Stack overflow protection via guard pages for the Mog runtime.
//
// Provides a dedicated mmap'd stack region with a PROT_NONE guard page at the
// bottom.  A SIGSEGV handler catches accesses to the guard page and recovers
// via siglongjmp back to the sigsetjmp site in `mog_protected_call`.

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Usable Mog stack size (1 MiB).
const MOG_STACK_SIZE: usize = 1024 * 1024;

/// Number of guard pages at the bottom of the stack region.
const MOG_GUARD_PAGES: usize = 1;

/// Size of the alternate signal stack (64 KiB).
const MOG_SIGALT_SIZE: usize = 64 * 1024;

/// Sentinel value returned by protected calls on stack overflow.
const MOG_OVERFLOW_SENTINEL: i64 = i64::MIN;

// ---------------------------------------------------------------------------
// FFI — sigsetjmp / siglongjmp
// ---------------------------------------------------------------------------
//
// The libc crate doesn't expose sigjmp_buf or sigsetjmp/siglongjmp on macOS.
// On ARM64 macOS: sigjmp_buf = [i32; 49] (192 bytes jmp_buf + 4 byte mask flag).

#[cfg(target_os = "macos")]
const SIGJMP_BUF_LEN: usize = 49; // int[49]

#[cfg(target_os = "linux")]
const SIGJMP_BUF_LEN: usize = 49; // conservative — actual varies by arch

type SigJmpBuf = [i32; SIGJMP_BUF_LEN];

unsafe extern "C" {
    /// Save the current execution context.  If `savemask` is nonzero the
    /// current signal mask is saved as well.  Returns 0 on the direct call;
    /// returns the `val` passed to `siglongjmp` when resuming.
    #[cfg_attr(target_os = "linux", link_name = "__sigsetjmp")]
    #[cfg_attr(not(target_os = "linux"), link_name = "sigsetjmp")]
    fn c_sigsetjmp(env: *mut SigJmpBuf, savemask: libc::c_int) -> libc::c_int;

    /// Jump back to the context saved by `sigsetjmp`.  `val` must be nonzero.
    #[link_name = "siglongjmp"]
    fn c_siglongjmp(env: *mut SigJmpBuf, val: libc::c_int) -> !;
}

// ---------------------------------------------------------------------------
// Global state (single-threaded Mog VM)
// ---------------------------------------------------------------------------

/// Whether `mog_stack_guard_init` has been called successfully.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Whether the last protected call experienced a stack overflow.
static OVERFLOW_FLAG: AtomicBool = AtomicBool::new(false);

struct GuardState {
    /// Base of the entire mmap'd region (guard + usable stack).
    region_base: *mut u8,
    /// Total size of the region (guard + stack).
    region_size: usize,

    /// Low bound of the guard page(s) (inclusive).
    guard_lo: usize,
    /// High bound of the guard page(s) (exclusive).
    guard_hi: usize,

    /// Top of the usable Mog stack (highest address, 16-byte aligned).
    stack_top: *mut u8,

    /// mmap'd alternate signal stack region.
    altstack_base: *mut u8,

    /// Previous SIGSEGV handler so we can restore it on destroy.
    old_segv_action: libc::sigaction,
    /// Previous SIGBUS handler.
    old_bus_action: libc::sigaction,

    /// Previous alternate signal stack so we can restore it.
    old_altstack: libc::stack_t,

    /// sigsetjmp buffer used for recovery.
    jmpbuf: SigJmpBuf,
    /// Flag readable from the signal handler (sig_atomic_t sized).
    /// 1 = jmpbuf is valid and we may longjmp.
    jmpbuf_valid: i32,
}

// Safety: Mog runtime is single-threaded within one VM; these statics are only
// accessed from that thread and from the synchronous signal handler.
static mut STATE: GuardState = GuardState {
    region_base: ptr::null_mut(),
    region_size: 0,
    guard_lo: 0,
    guard_hi: 0,
    stack_top: ptr::null_mut(),
    altstack_base: ptr::null_mut(),
    old_segv_action: unsafe { std::mem::zeroed() },
    old_bus_action: unsafe { std::mem::zeroed() },
    old_altstack: unsafe { std::mem::zeroed() },
    jmpbuf: [0i32; SIGJMP_BUF_LEN],
    jmpbuf_valid: 0,
};

// ---------------------------------------------------------------------------
// Signal handler
// ---------------------------------------------------------------------------

/// SIGSEGV / SIGBUS handler.  Runs on the alternate signal stack.
///
/// If the faulting address falls within the guard page and we have a valid
/// jmpbuf, recover via `siglongjmp`.  Otherwise re-raise with default action
/// (real crash).
unsafe extern "C" fn sigsegv_handler(
    sig: libc::c_int,
    info: *mut libc::siginfo_t,
    _ucontext: *mut libc::c_void,
) {
    unsafe {
        let fault_addr = (*info).si_addr() as usize;
        let state = &raw mut STATE;

        let in_guard = fault_addr >= (*state).guard_lo && fault_addr < (*state).guard_hi;

        if in_guard && (*state).jmpbuf_valid != 0 {
            // Mark the jmpbuf consumed so a second fault doesn't loop.
            (*state).jmpbuf_valid = 0;
            // Jump back to the sigsetjmp site.  The `1` argument becomes the
            // return value of sigsetjmp on the recovery path.
            c_siglongjmp(&raw mut (*state).jmpbuf, 1);
        }

        // Not our guard page — re-raise with default handler.
        let mut dfl: libc::sigaction = std::mem::zeroed();
        dfl.sa_sigaction = libc::SIG_DFL;
        libc::sigemptyset(&raw mut dfl.sa_mask);
        dfl.sa_flags = 0;
        libc::sigaction(sig, &dfl, ptr::null_mut());
        libc::raise(sig);
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the Mog stack guard.  Returns 0 on success, -1 on failure.
///
/// Allocates a stack region with a guard page, sets up an alternate signal
/// stack, and installs SIGSEGV/SIGBUS handlers.
#[unsafe(no_mangle)]
pub extern "C" fn mog_stack_guard_init() -> i32 {
    if INITIALIZED.load(Ordering::SeqCst) {
        return 0; // already initialised
    }

    unsafe {
        // -- Page size --
        let page_size = libc::sysconf(libc::_SC_PAGE_SIZE) as usize;
        if page_size == 0 {
            return -1;
        }
        let guard_size = MOG_GUARD_PAGES * page_size;
        let total_size = guard_size + MOG_STACK_SIZE;

        // -- mmap the stack region (guard + usable) --
        let region = libc::mmap(
            ptr::null_mut(),
            total_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        );
        if region == libc::MAP_FAILED {
            return -1;
        }

        // -- Protect the guard page(s) at the bottom --
        if libc::mprotect(region, guard_size, libc::PROT_NONE) != 0 {
            libc::munmap(region, total_size);
            return -1;
        }

        // -- Allocate and register alternate signal stack --
        let alt_mem = libc::mmap(
            ptr::null_mut(),
            MOG_SIGALT_SIZE,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        );
        if alt_mem == libc::MAP_FAILED {
            libc::munmap(region, total_size);
            return -1;
        }

        let ss = libc::stack_t {
            ss_sp: alt_mem,
            ss_size: MOG_SIGALT_SIZE,
            ss_flags: 0,
        };
        let state = &raw mut STATE;

        if libc::sigaltstack(&ss, &raw mut (*state).old_altstack) != 0 {
            libc::munmap(alt_mem, MOG_SIGALT_SIZE);
            libc::munmap(region, total_size);
            return -1;
        }

        // -- Install SIGSEGV handler --
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = sigsegv_handler as *const () as libc::sighandler_t;
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
        libc::sigemptyset(&raw mut sa.sa_mask);

        if libc::sigaction(libc::SIGSEGV, &sa, &raw mut (*state).old_segv_action) != 0 {
            libc::sigaltstack(&(*state).old_altstack, ptr::null_mut());
            libc::munmap(alt_mem, MOG_SIGALT_SIZE);
            libc::munmap(region, total_size);
            return -1;
        }

        // -- Install SIGBUS handler (macOS sends SIGBUS for some guard faults) --
        if libc::sigaction(libc::SIGBUS, &sa, &raw mut (*state).old_bus_action) != 0 {
            libc::sigaction(libc::SIGSEGV, &(*state).old_segv_action, ptr::null_mut());
            libc::sigaltstack(&(*state).old_altstack, ptr::null_mut());
            libc::munmap(alt_mem, MOG_SIGALT_SIZE);
            libc::munmap(region, total_size);
            return -1;
        }

        // -- Populate state --
        let base = region as *mut u8;
        (*state).region_base = base;
        (*state).region_size = total_size;
        (*state).guard_lo = base as usize;
        (*state).guard_hi = base as usize + guard_size;
        // Stack grows downward: top is the highest address, aligned to 16.
        let top = base as usize + total_size;
        (*state).stack_top = (top & !0xF) as *mut u8;
        (*state).altstack_base = alt_mem as *mut u8;
        (*state).jmpbuf_valid = 0;

        OVERFLOW_FLAG.store(false, Ordering::SeqCst);
        INITIALIZED.store(true, Ordering::SeqCst);
    }

    0
}

/// Tear down the stack guard — restore old handlers, free mapped memory.
#[unsafe(no_mangle)]
pub extern "C" fn mog_stack_guard_destroy() {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return;
    }

    unsafe {
        let state = &raw mut STATE;

        // Restore old signal handlers.
        libc::sigaction(libc::SIGBUS, &(*state).old_bus_action, ptr::null_mut());
        libc::sigaction(libc::SIGSEGV, &(*state).old_segv_action, ptr::null_mut());

        // Restore old alternate signal stack.
        libc::sigaltstack(&(*state).old_altstack, ptr::null_mut());

        // Unmap the alternate stack.
        libc::munmap((*state).altstack_base as *mut libc::c_void, MOG_SIGALT_SIZE);

        // Unmap the stack region.
        libc::munmap(
            (*state).region_base as *mut libc::c_void,
            (*state).region_size,
        );

        (*state).region_base = ptr::null_mut();
        (*state).region_size = 0;
        (*state).guard_lo = 0;
        (*state).guard_hi = 0;
        (*state).stack_top = ptr::null_mut();
        (*state).altstack_base = ptr::null_mut();
        (*state).jmpbuf_valid = 0;
    }

    INITIALIZED.store(false, Ordering::SeqCst);
}

/// Returns 1 if the guard has been initialised, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn mog_stack_guard_is_init() -> i32 {
    INITIALIZED.load(Ordering::SeqCst) as i32
}

/// Returns 1 if the most recent protected call overflowed, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn mog_stack_overflow_occurred() -> i32 {
    OVERFLOW_FLAG.load(Ordering::SeqCst) as i32
}

/// Print a stack overflow diagnostic to stderr.  Called from generated main()
/// when `mog_protected_call_0` returns the overflow sentinel.
#[unsafe(no_mangle)]
pub extern "C" fn mog_stack_overflow_print() {
    const MSG: &[u8] = b"mog: stack overflow\n";
    unsafe {
        libc::write(2, MSG.as_ptr() as *const libc::c_void, MSG.len());
    }
}

/// Call `func(arg)` on the Mog stack with guard page protection.
///
/// Returns the function's result on success, or `i64::MIN` if a stack
/// overflow was detected.
#[unsafe(no_mangle)]
pub extern "C" fn mog_protected_call(func: extern "C" fn(i64) -> i64, arg: i64) -> i64 {
    assert!(
        INITIALIZED.load(Ordering::SeqCst),
        "stack guard not initialised"
    );

    OVERFLOW_FLAG.store(false, Ordering::SeqCst);

    unsafe {
        let state = &raw mut STATE;

        // sigsetjmp returns 0 on the direct call, nonzero from siglongjmp.
        let rc = c_sigsetjmp(&raw mut (*state).jmpbuf, 1);
        if rc != 0 {
            // Recovery path — we longjmp'd out of a guard page fault.
            (*state).jmpbuf_valid = 0;
            OVERFLOW_FLAG.store(true, Ordering::SeqCst);
            return MOG_OVERFLOW_SENTINEL;
        }

        // Mark jmpbuf as valid so the signal handler may use it.
        (*state).jmpbuf_valid = 1;

        let stack_top = (*state).stack_top;
        let func_ptr = func as *const u8;
        let result: i64;

        #[cfg(target_arch = "aarch64")]
        {
            // Save host SP on the Mog stack so the callee can't clobber it.
            // x9 is caller-saved and the Mog function may trash it.
            std::arch::asm!(
                "mov x9, sp",          // save host SP to x9 temporarily
                "mov sp, {stack}",     // switch to Mog stack
                "str x9, [sp, #-16]!", // push host SP onto Mog stack (16-byte aligned)
                "blr {func}",          // call the target function
                "ldr x9, [sp], #16",   // pop host SP from Mog stack
                "mov sp, x9",          // restore host SP
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                in("x0") arg,
                lateout("x0") result,
                clobber_abi("C"),
                out("x9") _,
            );
        }

        #[cfg(target_arch = "x86_64")]
        {
            std::arch::asm!(
                "mov r11, rsp",
                "mov rsp, {stack}",
                "call {func}",
                "mov rsp, r11",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                in("rdi") arg,
                lateout("rax") result,
                out("r11") _,
                clobber_abi("C"),
            );
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            // Fallback: call without stack switch (for testing on x86_64 etc.)
            let f: extern "C" fn(i64) -> i64 = std::mem::transmute(func_ptr);
            result = f(arg);
        }

        (*state).jmpbuf_valid = 0;
        result
    }
}

/// Call `func()` (no arguments) on the Mog stack with guard page protection.
#[unsafe(no_mangle)]
pub extern "C" fn mog_protected_call_0(func: extern "C" fn() -> i64) -> i64 {
    assert!(
        INITIALIZED.load(Ordering::SeqCst),
        "stack guard not initialised"
    );

    OVERFLOW_FLAG.store(false, Ordering::SeqCst);

    unsafe {
        let state = &raw mut STATE;

        let rc = c_sigsetjmp(&raw mut (*state).jmpbuf, 1);
        if rc != 0 {
            (*state).jmpbuf_valid = 0;
            OVERFLOW_FLAG.store(true, Ordering::SeqCst);
            return MOG_OVERFLOW_SENTINEL;
        }

        (*state).jmpbuf_valid = 1;

        let stack_top = (*state).stack_top;
        let func_ptr = func as *const u8;
        let result: i64;

        #[cfg(target_arch = "aarch64")]
        {
            std::arch::asm!(
                "mov x9, sp",
                "mov sp, {stack}",
                "str x9, [sp, #-16]!",
                "blr {func}",
                "ldr x9, [sp], #16",
                "mov sp, x9",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                lateout("x0") result,
                clobber_abi("C"),
                out("x9") _,
            );
        }

        #[cfg(target_arch = "x86_64")]
        {
            std::arch::asm!(
                "mov r11, rsp",
                "mov rsp, {stack}",
                "push r11",
                "call {func}",
                "pop r11",
                "mov rsp, r11",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                lateout("rax") result,
                out("r11") _,
                clobber_abi("C"),
            );
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            let f: extern "C" fn() -> i64 = std::mem::transmute(func_ptr);
            result = f();
        }

        (*state).jmpbuf_valid = 0;
        result
    }
}

/// Call `func(arg)` on the Mog stack with guard page protection.
///
/// This variant takes a `*mut u8` argument and returns `i32`, suitable for
/// entry points that receive an opaque context pointer.
#[unsafe(no_mangle)]
pub extern "C" fn mog_protected_call_void(
    func: extern "C" fn(*mut u8) -> i32,
    arg: *mut u8,
) -> i32 {
    assert!(
        INITIALIZED.load(Ordering::SeqCst),
        "stack guard not initialised"
    );

    OVERFLOW_FLAG.store(false, Ordering::SeqCst);

    unsafe {
        let state = &raw mut STATE;

        let rc = c_sigsetjmp(&raw mut (*state).jmpbuf, 1);
        if rc != 0 {
            (*state).jmpbuf_valid = 0;
            OVERFLOW_FLAG.store(true, Ordering::SeqCst);
            return -1;
        }

        (*state).jmpbuf_valid = 1;

        let stack_top = (*state).stack_top;
        let func_ptr = func as *const u8;
        let result: i32;

        #[cfg(target_arch = "aarch64")]
        {
            let raw_result: i64;
            std::arch::asm!(
                "mov x9, sp",
                "mov sp, {stack}",
                "str x9, [sp, #-16]!",
                "blr {func}",
                "ldr x9, [sp], #16",
                "mov sp, x9",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                in("x0") arg,
                lateout("x0") raw_result,
                clobber_abi("C"),
                out("x9") _,
            );
            result = raw_result as i32;
        }

        #[cfg(target_arch = "x86_64")]
        {
            let raw_result: i64;
            std::arch::asm!(
                "mov r11, rsp",
                "mov rsp, {stack}",
                "push r11",
                "call {func}",
                "pop r11",
                "mov rsp, r11",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                in("rdi") arg,
                lateout("rax") raw_result,
                out("r11") _,
                clobber_abi("C"),
            );
            result = raw_result as i32;
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            let f: extern "C" fn(*mut u8) -> i32 = std::mem::transmute(func_ptr);
            result = f(arg);
        }

        (*state).jmpbuf_valid = 0;
        result
    }
}

// ---------------------------------------------------------------------------
// Protected call for coroutine resume — fn(*mut u8) -> ()
//
// Returns 0 on success, -1 on stack overflow.
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_protected_call_coro(func: extern "C" fn(*mut u8), handle: *mut u8) -> i32 {
    if !INITIALIZED.load(Ordering::SeqCst) {
        // Guard not initialised — call directly (fallback).
        func(handle);
        return 0;
    }

    OVERFLOW_FLAG.store(false, Ordering::SeqCst);

    unsafe {
        let state = &raw mut STATE;

        let rc = c_sigsetjmp(&raw mut (*state).jmpbuf, 1);
        if rc != 0 {
            (*state).jmpbuf_valid = 0;
            OVERFLOW_FLAG.store(true, Ordering::SeqCst);
            return -1;
        }

        (*state).jmpbuf_valid = 1;

        let stack_top = (*state).stack_top;
        let func_ptr = func as *const u8;

        #[cfg(target_arch = "aarch64")]
        {
            std::arch::asm!(
                "mov x9, sp",
                "mov sp, {stack}",
                "str x9, [sp, #-16]!",
                "blr {func}",
                "ldr x9, [sp], #16",
                "mov sp, x9",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                in("x0") handle,
                clobber_abi("C"),
                out("x9") _,
            );
        }

        #[cfg(target_arch = "x86_64")]
        {
            std::arch::asm!(
                "mov r11, rsp",
                "mov rsp, {stack}",
                "push r11",
                "call {func}",
                "pop r11",
                "mov rsp, r11",
                func = in(reg) func_ptr,
                stack = in(reg) stack_top,
                in("rdi") handle,
                out("r11") _,
                clobber_abi("C"),
            );
        }

        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
        {
            let f: extern "C" fn(*mut u8) = std::mem::transmute(func_ptr);
            f(handle);
        }

        (*state).jmpbuf_valid = 0;
        0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Tests that touch global signal state must not run concurrently.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    // -- Helper functions with C calling convention --

    extern "C" fn add_one(n: i64) -> i64 {
        n + 1
    }

    extern "C" fn return_42() -> i64 {
        42
    }

    extern "C" fn noop_void(_ctx: *mut u8) -> i32 {
        7
    }

    #[allow(unconditional_recursion)]
    extern "C" fn infinite_recurse(n: i64) -> i64 {
        infinite_recurse(n + 1)
    }

    #[allow(unconditional_recursion)]
    extern "C" fn infinite_recurse_0() -> i64 {
        infinite_recurse_0()
    }

    #[allow(unconditional_recursion)]
    extern "C" fn infinite_recurse_void(_ctx: *mut u8) -> i32 {
        infinite_recurse_void(_ctx)
    }

    #[test]
    fn test_init_and_destroy() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_is_init(), 0);
        assert_eq!(mog_stack_guard_init(), 0);
        assert_eq!(mog_stack_guard_is_init(), 1);

        // Double init is a no-op and returns success.
        assert_eq!(mog_stack_guard_init(), 0);

        mog_stack_guard_destroy();
        assert_eq!(mog_stack_guard_is_init(), 0);
    }

    #[test]
    fn test_normal_call() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        let result = mog_protected_call(add_one, 99);
        assert_eq!(result, 100);
        assert_eq!(mog_stack_overflow_occurred(), 0);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_normal_call_0() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        let result = mog_protected_call_0(return_42);
        assert_eq!(result, 42);
        assert_eq!(mog_stack_overflow_occurred(), 0);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_normal_call_void() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        let result = mog_protected_call_void(noop_void, ptr::null_mut());
        assert_eq!(result, 7);
        assert_eq!(mog_stack_overflow_occurred(), 0);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_stack_overflow_detection() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        let result = mog_protected_call(infinite_recurse, 0);
        assert_eq!(result, MOG_OVERFLOW_SENTINEL);
        assert_eq!(mog_stack_overflow_occurred(), 1);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_stack_overflow_detection_0() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        let result = mog_protected_call_0(infinite_recurse_0);
        assert_eq!(result, MOG_OVERFLOW_SENTINEL);
        assert_eq!(mog_stack_overflow_occurred(), 1);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_stack_overflow_detection_void() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        let result = mog_protected_call_void(infinite_recurse_void, ptr::null_mut());
        assert_eq!(result, -1);
        assert_eq!(mog_stack_overflow_occurred(), 1);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_recovery_allows_subsequent_calls() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);

        // First call overflows.
        let r1 = mog_protected_call(infinite_recurse, 0);
        assert_eq!(r1, MOG_OVERFLOW_SENTINEL);
        assert_eq!(mog_stack_overflow_occurred(), 1);

        // Second call succeeds normally — recovery didn't break anything.
        let r2 = mog_protected_call(add_one, 41);
        assert_eq!(r2, 42);
        assert_eq!(mog_stack_overflow_occurred(), 0);

        // Another overflow.
        let r3 = mog_protected_call(infinite_recurse, 0);
        assert_eq!(r3, MOG_OVERFLOW_SENTINEL);
        assert_eq!(mog_stack_overflow_occurred(), 1);

        // And another normal call.
        let r4 = mog_protected_call(add_one, 0);
        assert_eq!(r4, 1);
        assert_eq!(mog_stack_overflow_occurred(), 0);

        mog_stack_guard_destroy();
    }

    #[test]
    fn test_destroy_and_reinit() {
        let _lock = TEST_LOCK.lock().unwrap();

        assert_eq!(mog_stack_guard_init(), 0);
        let r1 = mog_protected_call(add_one, 10);
        assert_eq!(r1, 11);
        mog_stack_guard_destroy();

        // Re-initialise and use again.
        assert_eq!(mog_stack_guard_init(), 0);
        let r2 = mog_protected_call(add_one, 20);
        assert_eq!(r2, 21);
        mog_stack_guard_destroy();
    }
}
