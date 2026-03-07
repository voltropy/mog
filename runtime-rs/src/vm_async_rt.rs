// Mog async runtime — event loop, futures, coroutine scheduling.
//
// Port of runtime/mog_async.c.  All allocations use Box/Vec (malloc-backed),
// NOT the main GC, to stay independent of managed objects and avoid problems with
// LLVM coroutine frames. Host-managed runtime structures use gc_external_alloc/free
// so they stay under guest memory-limit accounting.

use crate::gc::{gc_external_alloc, gc_external_free};
use std::ffi::c_void;
use std::ptr;

fn build_all_results_array(sub_results: *const i64, count: i32) -> *mut u8 {
    let dimensions = [count as u64];
    let values = crate::array::array_alloc(8, 1, dimensions.as_ptr());
    if values.is_null() {
        return ptr::null_mut();
    }

    for i in 0..count as usize {
        unsafe {
            crate::array::array_set(values, i as i64, *sub_results.add(i));
        }
    }
    values
}

type MogTimerDispatch = extern "C" fn(*mut u8, *mut u8, i64);

static mut MOG_TIMER_DISPATCH: *const c_void = ptr::null();

const CAP_TIMER_NAME: &[u8] = b"timer\0";
const CAP_TIMER_SET_TIMEOUT_NAME: &[u8] = b"setTimeout\0";

const ERR_TIMER_MISSING_ARG: &[u8] = b"timer.setTimeout: expected one int argument\0";
const ERR_TIMER_BAD_ARG: &[u8] = b"timer.setTimeout: expected int argument\0";

// ---------------------------------------------------------------------------
// Monotonic clock helpers
// ---------------------------------------------------------------------------

fn now_ns() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_nanos() as u64
}

// ---------------------------------------------------------------------------
// Future states
// ---------------------------------------------------------------------------

const MOG_FUTURE_PENDING: i32 = 0;
const MOG_FUTURE_READY: i32 = 1;
const MOG_FUTURE_ERROR: i32 = 2;
const ERR_FUTURE_ERROR: i64 = -1;
const ERR_ALL_RESULT_ARRAY_OOM: i64 = -2;

// ---------------------------------------------------------------------------
// MogFuture — matches the C layout exactly.
//
// struct MogFuture {
//     state: i32,           // offset  0
//     _pad: i32,            // offset  4   (implicit padding for alignment)
//     result: i64,          // offset  8
//     coro_handle: *mut u8, // offset 16
//     coro_frame: *mut u8,  // offset 24
//     next: *mut MogFuture, // offset 32
//     sub_futures: *mut *mut MogFuture, // offset 40
//     sub_count: i32,       // offset 48
//     sub_done: i32,        // offset 52
//     sub_results: *mut i64,// offset 56
//     parent: *mut MogFuture, // offset 64
// }
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct MogFuture {
    pub state: i32,
    _pad: i32,
    pub result: i64,
    pub coro_handle: *mut u8,
    pub coro_frame: *mut u8,
    pub next: *mut MogFuture,
    pub sub_futures: *mut *mut MogFuture,
    pub sub_count: i32,
    pub sub_done: i32,
    pub sub_results: *mut i64,
    pub parent: *mut MogFuture,
}

// ---------------------------------------------------------------------------
// MogTimer — intrusive sorted linked list node.
// ---------------------------------------------------------------------------

#[repr(C)]
struct MogTimer {
    deadline_ns: u64,
    future: *mut MogFuture,
    result_value: i64,
    next: *mut MogTimer,
}

// ---------------------------------------------------------------------------
// MogFdWatcher — intrusive linked list node.
// ---------------------------------------------------------------------------

const MOG_FD_READ: i32 = 1;
const MOG_FD_WRITE: i32 = 2;

#[repr(C)]
struct MogFdWatcher {
    fd: i32,
    events: i32,
    future: *mut MogFuture,
    next: *mut MogFdWatcher,
}

// ---------------------------------------------------------------------------
// MogEventLoop
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct MogEventLoop {
    ready_head: *mut MogFuture,
    ready_tail: *mut MogFuture,
    timers: *mut MogTimer,
    watchers: *mut MogFdWatcher,
    running: i32,
    pending_count: i32,
}

// ---------------------------------------------------------------------------
// Global event loop
// ---------------------------------------------------------------------------

static mut G_LOOP: *mut MogEventLoop = ptr::null_mut();

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_set_global(loop_ptr: *mut u8) {
    unsafe {
        G_LOOP = loop_ptr as *mut MogEventLoop;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_get_global() -> *mut u8 {
    unsafe { G_LOOP as *mut u8 }
}

// ---------------------------------------------------------------------------
// Event loop lifecycle
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_new() -> *mut u8 {
    let raw = gc_external_alloc(std::mem::size_of::<MogEventLoop>());
    if raw.is_null() {
        return ptr::null_mut();
    }
    let el = raw as *mut MogEventLoop;
    unsafe {
        *el = MogEventLoop {
            ready_head: ptr::null_mut(),
            ready_tail: ptr::null_mut(),
            timers: ptr::null_mut(),
            watchers: ptr::null_mut(),
            running: 0,
            pending_count: 0,
        };
    }
    el as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_new() -> *mut u8 {
    let raw = gc_external_alloc(std::mem::size_of::<MogFuture>());
    if raw.is_null() {
        return ptr::null_mut();
    }
    let ptr = raw as *mut MogFuture;
    unsafe {
        ptr::write(
            ptr,
            MogFuture {
                state: MOG_FUTURE_PENDING,
                _pad: 0,
                result: 0,
                coro_handle: ptr::null_mut(),
                coro_frame: ptr::null_mut(),
                next: ptr::null_mut(),
                sub_futures: ptr::null_mut(),
                sub_count: 0,
                sub_done: 0,
                sub_results: ptr::null_mut(),
                parent: ptr::null_mut(),
            },
        );
    }
    unsafe {
        if !G_LOOP.is_null() {
            (*G_LOOP).pending_count += 1;
        }
    }
    ptr as *mut u8
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_free(loop_ptr: *mut u8) {
    if loop_ptr.is_null() {
        return;
    }
    unsafe {
        let el = &mut *(loop_ptr as *mut MogEventLoop);

        // Free remaining timers
        let mut t = el.timers;
        while !t.is_null() {
            let next = (*t).next;
            gc_external_free(t as *mut u8);
            t = next;
        }

        // Free remaining fd watchers
        let mut w = el.watchers;
        while !w.is_null() {
            let next = (*w).next;
            gc_external_free(w as *mut u8);
            w = next;
        }

        gc_external_free(loop_ptr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_complete(f: *mut u8, value: i64) {
    if f.is_null() {
        return;
    }
    unsafe {
        let future = &mut *(f as *mut MogFuture);
        if future.state != MOG_FUTURE_PENDING {
            return;
        }
        future.state = MOG_FUTURE_READY;
        future.result = value;

        if !G_LOOP.is_null() {
            (*G_LOOP).pending_count -= 1;
        }

        // If a coroutine is waiting, schedule it for resumption
        if !future.coro_handle.is_null() {
            let loop_ptr = G_LOOP;
            if !loop_ptr.is_null() {
                mog_loop_enqueue_ready(loop_ptr as *mut u8, f);
            }
        }

        // If this future belongs to a parent combinator (all/race)
        if !future.parent.is_null() {
            let parent = &mut *future.parent;
            if parent.state == MOG_FUTURE_PENDING {
                if !parent.sub_futures.is_null() {
                    // all() combinator
                    parent.sub_done += 1;
                    // Store result at the index of this sub-future
                    for i in 0..parent.sub_count {
                        if *parent.sub_futures.add(i as usize) == (f as *mut MogFuture) {
                            if !parent.sub_results.is_null() {
                                *parent.sub_results.add(i as usize) = value;
                            }
                            break;
                        }
                    }
                    if parent.sub_done >= parent.sub_count {
                        let sub_results_ptr = parent.sub_results;
                        let parent_ptr = future.parent as *mut u8;
                        let array_ptr = build_all_results_array(sub_results_ptr, parent.sub_count);
                        if array_ptr.is_null() {
                            crate::vm::mog_request_interrupt();
                            mog_future_set_error(parent_ptr, ERR_ALL_RESULT_ARRAY_OOM);
                            return;
                        }
                        parent.result = array_ptr as i64;
                        if !parent.sub_results.is_null() {
                            gc_external_free(parent.sub_results as *mut u8);
                            parent.sub_results = ptr::null_mut();
                        }
                        // All sub-futures done — complete parent with pointer to array
                        mog_future_complete(parent_ptr, parent.result);
                    }
                } else {
                    // race() combinator — first to finish wins
                    let parent_ptr = future.parent as *mut u8;
                    mog_future_complete(parent_ptr, value);
                }
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_set_error(f: *mut u8, err: i64) {
    if f.is_null() {
        return;
    }
    unsafe {
        let future = &mut *(f as *mut MogFuture);
        if future.state != MOG_FUTURE_PENDING {
            return;
        }
        future.state = MOG_FUTURE_ERROR;
        future.result = err;

        if !G_LOOP.is_null() {
            (*G_LOOP).pending_count -= 1;
        }
        if !future.coro_handle.is_null() && !G_LOOP.is_null() {
            mog_loop_enqueue_ready(G_LOOP as *mut u8, f);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_is_ready(f: *const u8) -> i32 {
    if f.is_null() {
        return 0;
    }
    unsafe {
        let future = &*(f as *const MogFuture);
        if future.state != MOG_FUTURE_PENDING {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_get_result(f: *const u8) -> i64 {
    if f.is_null() {
        return 0;
    }
    unsafe { (*(f as *const MogFuture)).result }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_set_waiter(f: *mut u8, waiter: *mut u8) {
    if f.is_null() {
        return;
    }
    unsafe {
        (*(f as *mut MogFuture)).coro_handle = waiter;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_get_coro_handle(f: *const u8) -> *mut u8 {
    if f.is_null() {
        return ptr::null_mut();
    }
    unsafe { (*(f as *const MogFuture)).coro_handle }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_set_coro_frame(f: *mut u8, frame: *mut u8) {
    if f.is_null() {
        return;
    }
    unsafe {
        (*(f as *mut MogFuture)).coro_frame = frame;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_free(f: *mut u8) {
    if f.is_null() {
        return;
    }
    unsafe {
        let future = &mut *(f as *mut MogFuture);
        // Free coro_frame if allocated
        if !future.coro_frame.is_null() {
            gc_external_free(future.coro_frame);
            future.coro_frame = ptr::null_mut();
        }
        // Free sub_futures array
        if !future.sub_futures.is_null() {
            gc_external_free(future.sub_futures as *mut u8);
        }
        // Free sub_results array
        if !future.sub_results.is_null() {
            gc_external_free(future.sub_results as *mut u8);
        }
        // Free the future itself
        gc_external_free(f);
    }
}

// ---------------------------------------------------------------------------
// Scheduling
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_enqueue_ready(loop_ptr: *mut u8, f: *mut u8) {
    if loop_ptr.is_null() || f.is_null() {
        return;
    }
    unsafe {
        let el = &mut *(loop_ptr as *mut MogEventLoop);
        let future = &mut *(f as *mut MogFuture);
        future.next = ptr::null_mut();
        if !el.ready_tail.is_null() {
            (*el.ready_tail).next = f as *mut MogFuture;
        } else {
            el.ready_head = f as *mut MogFuture;
        }
        el.ready_tail = f as *mut MogFuture;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_schedule(loop_ptr: *mut u8, coro_handle: *mut u8) {
    if loop_ptr.is_null() || coro_handle.is_null() {
        return;
    }
    // Create a transient heap-allocated future to carry the handle through the ready queue
    let raw = gc_external_alloc(std::mem::size_of::<MogFuture>());
    if raw.is_null() {
        crate::vm::mog_request_interrupt();
        return;
    }
    let ptr = raw as *mut MogFuture;
    unsafe {
        ptr::write(
            ptr,
            MogFuture {
                state: MOG_FUTURE_READY,
                _pad: 0,
                result: 0,
                coro_handle,
                coro_frame: ptr::null_mut(),
                next: ptr::null_mut(),
                sub_futures: ptr::null_mut(),
                sub_count: 0,
                sub_done: 0,
                sub_results: ptr::null_mut(),
                parent: ptr::null_mut(),
            },
        );
    }
    mog_loop_enqueue_ready(loop_ptr, ptr as *mut u8);
}

// ---------------------------------------------------------------------------
// Timers
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_add_timer(loop_ptr: *mut u8, ms: i64, future: *mut u8) {
    if loop_ptr.is_null() || future.is_null() {
        return;
    }
    unsafe {
        add_timer_inner(
            &mut *(loop_ptr as *mut MogEventLoop),
            ms as u64,
            future as *mut MogFuture,
            0,
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_add_timer_with_value(
    loop_ptr: *mut u8,
    ms: i64,
    future: *mut u8,
    value: i64,
) {
    if loop_ptr.is_null() || future.is_null() {
        return;
    }
    unsafe {
        add_timer_inner(
            &mut *(loop_ptr as *mut MogEventLoop),
            ms as u64,
            future as *mut MogFuture,
            value,
        );
    }
}

unsafe fn add_timer_inner(
    el: &mut MogEventLoop,
    delay_ms: u64,
    future: *mut MogFuture,
    result_value: i64,
) {
    unsafe {
        let timer = gc_external_alloc(std::mem::size_of::<MogTimer>());
        if timer.is_null() {
            crate::vm::mog_request_interrupt();
            return;
        }
        let timer = timer as *mut MogTimer;
        ptr::write(
            timer,
            MogTimer {
                deadline_ns: now_ns() + delay_ms * 1_000_000,
                future,
                result_value,
                next: ptr::null_mut(),
            },
        );

        // Insert sorted by deadline
        if el.timers.is_null() || (*timer).deadline_ns < (*el.timers).deadline_ns {
            (*timer).next = el.timers;
            el.timers = timer;
        } else {
            let mut cur = el.timers;
            while !(*cur).next.is_null() && (*(*cur).next).deadline_ns <= (*timer).deadline_ns {
                cur = (*cur).next;
            }
            (*timer).next = (*cur).next;
            (*cur).next = timer;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_set_timer_dispatcher(dispatch: *const c_void) {
    unsafe {
        MOG_TIMER_DISPATCH = dispatch;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_clear_timer_dispatcher() {
    unsafe {
        MOG_TIMER_DISPATCH = ptr::null();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_register_timer_host(vm: *mut u8) -> i32 {
    if vm.is_null() {
        return -1;
    }

    if mog_has_capability(vm, CAP_TIMER_NAME.as_ptr()) != 0 {
        return 0;
    }

    const ENTRIES: &[MogCapEntry] = &[
        MogCapEntry {
            name: CAP_TIMER_SET_TIMEOUT_NAME.as_ptr(),
            func: mog_timer_set_timeout,
        },
        MogCapEntry {
            name: ptr::null(),
            func: mog_timer_sentinel,
        },
    ];

    mog_register_capability(vm, CAP_TIMER_NAME.as_ptr(), ENTRIES.as_ptr())
}

#[unsafe(no_mangle)]
extern "C" fn mog_timer_sentinel(_vm: *mut u8, _args: *const MogValue, _nargs: i32) -> MogValue {
    crate::vm::mog_none()
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_timer_set_timeout(
    _vm: *mut u8,
    args: *const MogValue,
    nargs: i32,
) -> MogValue {
    if nargs != 1 || args.is_null() {
        return mog_error(ERR_TIMER_MISSING_ARG.as_ptr());
    }

    let ms = unsafe {
        let arg = &*args;
        if arg.tag != MOG_INT {
            return mog_error(ERR_TIMER_BAD_ARG.as_ptr());
        }
        arg.data.i
    };

    let future = mog_future_new();
    if future.is_null() {
        return mog_error("timer.setTimeout: out of memory\0".as_ptr());
    }

    let delay = if ms < 0 { 0 } else { ms };
    let loop_ptr = mog_loop_get_global();

    let dispatch = unsafe { MOG_TIMER_DISPATCH };
    if !dispatch.is_null() {
        let cb: MogTimerDispatch = unsafe { std::mem::transmute(dispatch) };
        cb(_vm, future as *mut u8, delay);
    } else if loop_ptr.is_null() {
        mog_future_complete(future, delay);
    } else {
        mog_loop_add_timer_with_value(loop_ptr, delay, future, delay);
    }

    mog_int(future as i64)
}

// ---------------------------------------------------------------------------
// FD Watchers
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_add_fd_watcher(loop_ptr: *mut u8, fd: i32, future: *mut u8) {
    mog_loop_add_fd_watcher_events(loop_ptr, fd, MOG_FD_READ, future);
}

/// Internal: add fd watcher with explicit event mask.
#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_add_fd_watcher_events(
    loop_ptr: *mut u8,
    fd: i32,
    events: i32,
    future: *mut u8,
) {
    if loop_ptr.is_null() || future.is_null() {
        return;
    }
    unsafe {
        let el = &mut *(loop_ptr as *mut MogEventLoop);
        let w = gc_external_alloc(std::mem::size_of::<MogFdWatcher>());
        if w.is_null() {
            crate::vm::mog_request_interrupt();
            return;
        }
        let w = w as *mut MogFdWatcher;
        ptr::write(
            w,
            MogFdWatcher {
                fd,
                events,
                future: future as *mut MogFuture,
                next: el.watchers,
            },
        );
        el.watchers = w;
    }
}

// ---------------------------------------------------------------------------
// Await helper — called from LLVM IR
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_await(f: *mut u8, coro_handle: *mut u8) -> i32 {
    if f.is_null() {
        return 0;
    }
    unsafe {
        let future = &mut *(f as *mut MogFuture);
        // Always register the coroutine handle so the event loop can resume us
        future.coro_handle = coro_handle;
        if future.state != MOG_FUTURE_PENDING {
            // Already ready — enqueue for immediate resumption
            if !G_LOOP.is_null() {
                mog_loop_enqueue_ready(G_LOOP as *mut u8, f);
            }
        }
        0
    }
}

// ---------------------------------------------------------------------------
// Coroutine resume — reads fn pointer from handle[0] and calls fn(handle)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_coro_resume(handle: *mut u8) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let fn_ptr: extern "C" fn(*mut u8) = std::mem::transmute(*(handle as *const usize));
        let rc = crate::stack_guard::mog_protected_call_coro(fn_ptr, handle);
        if rc < 0 {
            // Stack overflow during coroutine resume — print diagnostic and abort.
            libc::write(
                2,
                b"mog: stack overflow in async coroutine\n".as_ptr() as *const libc::c_void,
                39,
            );
            libc::_exit(2);
        }
    }
}

// ---------------------------------------------------------------------------
// Event loop — main run loop
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_run(loop_ptr: *mut u8) {
    if loop_ptr.is_null() {
        return;
    }
    unsafe {
        let el = &mut *(loop_ptr as *mut MogEventLoop);
        el.running = 1;

        while el.running != 0 {
            // 1. Fire expired timers
            let t = now_ns();
            while !el.timers.is_null() && (*el.timers).deadline_ns <= t {
                let timer = el.timers;
                el.timers = (*timer).next;
                let future_ptr = (*timer).future as *mut u8;
                let result_val = (*timer).result_value;
                gc_external_free(timer as *mut u8);
                mog_future_complete(future_ptr, result_val);
            }

            // 2. Process ready queue
            let mut processed = 0;
            while !el.ready_head.is_null() {
                let f = el.ready_head;
                el.ready_head = (*f).next;
                if el.ready_head.is_null() {
                    el.ready_tail = ptr::null_mut();
                }
                (*f).next = ptr::null_mut();

                if !(*f).coro_handle.is_null() {
                    let hdl = (*f).coro_handle;
                    (*f).coro_handle = ptr::null_mut();
                    mog_coro_resume(hdl);
                    processed += 1;
                }
            }

            // 2b. Stop promptly when host timeout is requested.
            if crate::vm::mog_interrupt_requested() != 0 {
                break;
            }

            // 3. Exit when all known work is complete.
            if el.timers.is_null()
                && el.ready_head.is_null()
                && el.watchers.is_null()
                && el.pending_count <= 0
            {
                break;
            }

            // 3b. Wait for fd events, timer deadlines, or host timeout.
            if el.ready_head.is_null() {
                let mut read_fds: libc::fd_set = std::mem::zeroed();
                let mut write_fds: libc::fd_set = std::mem::zeroed();
                libc::FD_ZERO(&mut read_fds);
                libc::FD_ZERO(&mut write_fds);
                let mut max_fd: i32 = -1;

                // Add all fd watchers
                let mut w = el.watchers;
                while !w.is_null() {
                    if (*w).events & MOG_FD_READ != 0 {
                        libc::FD_SET((*w).fd, &mut read_fds);
                    }
                    if (*w).events & MOG_FD_WRITE != 0 {
                        libc::FD_SET((*w).fd, &mut write_fds);
                    }
                    if (*w).fd > max_fd {
                        max_fd = (*w).fd;
                    }
                    w = (*w).next;
                }

                // Calculate timeout from next timer.
                let mut wait_ns: Option<u64> = None;

                let mut tv = libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                };

                if !el.timers.is_null() {
                    let tnow = now_ns();
                    let mut timer_wait_ns = 0;
                    if (*el.timers).deadline_ns > tnow {
                        timer_wait_ns = (*el.timers).deadline_ns - tnow;
                    }
                    // else: deadline already passed, tv stays 0,0 (immediate)
                    wait_ns = Some(timer_wait_ns);
                }

                if let Some(timeout_ns) = crate::vm::mog_timeout_remaining_ns() {
                    wait_ns = Some(match wait_ns {
                        Some(timer_wait_ns) => timer_wait_ns.min(timeout_ns),
                        None => timeout_ns,
                    });
                }

                let mut tv_ptr = ptr::null_mut();
                if let Some(wait_ns) = wait_ns {
                    tv.tv_sec = (wait_ns / 1_000_000_000) as libc::time_t;
                    tv.tv_usec = ((wait_ns % 1_000_000_000) / 1000) as libc::suseconds_t;
                    tv_ptr = &mut tv as *mut libc::timeval;
                }

                if max_fd >= 0 {
                    let nready = libc::select(
                        max_fd + 1,
                        &mut read_fds,
                        &mut write_fds,
                        ptr::null_mut(),
                        tv_ptr,
                    );
                    if nready > 0 {
                        // Check which fd watchers fired
                        let mut prev_ptr: *mut *mut MogFdWatcher = &mut el.watchers;
                        let mut w = el.watchers;
                        while !w.is_null() {
                            let next = (*w).next;
                            let mut fired = false;
                            if ((*w).events & MOG_FD_READ != 0)
                                && libc::FD_ISSET((*w).fd, &read_fds)
                            {
                                fired = true;
                            }
                            if ((*w).events & MOG_FD_WRITE != 0)
                                && libc::FD_ISSET((*w).fd, &write_fds)
                            {
                                fired = true;
                            }

                            if fired {
                                // Remove from list
                                *prev_ptr = next;
                                let watcher_fd = (*w).fd;
                                let watcher_future = (*w).future as *mut u8;

                                if watcher_fd == libc::STDIN_FILENO
                                    && ((*w).events & MOG_FD_READ != 0)
                                {
                                    // Read a line from stdin
                                    let mut buf = [0u8; 4096];
                                    let result = libc::fgets(
                                        buf.as_mut_ptr() as *mut libc::c_char,
                                        buf.len() as i32,
                                        stdin_file(),
                                    );
                                    if !result.is_null() {
                                        let len = libc::strlen(buf.as_ptr() as *const libc::c_char);
                                        let mut slen = len;
                                        // Remove trailing newline
                                        if slen > 0 && buf[slen - 1] == b'\n' {
                                            slen -= 1;
                                            buf[slen] = 0;
                                        }
                                        // Allocate via gc_external_alloc
                                        let dest = gc_external_alloc(slen + 1);
                                        if dest.is_null() {
                                            crate::vm::mog_request_interrupt();
                                            mog_future_complete(watcher_future, 0);
                                            *prev_ptr = next;
                                            gc_external_free(w as *mut u8);
                                            w = next;
                                            continue;
                                        }
                                        ptr::copy_nonoverlapping(buf.as_ptr(), dest, slen + 1);
                                        mog_future_complete(watcher_future, dest as i64);
                                    } else {
                                        // EOF or error
                                        mog_future_complete(watcher_future, 0);
                                    }
                                } else {
                                    // Generic fd ready — complete with fd number
                                    mog_future_complete(watcher_future, watcher_fd as i64);
                                }
                                gc_external_free(w as *mut u8);
                            } else {
                                prev_ptr = &mut (*w).next;
                            }
                            w = next;
                        }
                    }
                } else {
                    let nready =
                        libc::select(0, ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), tv_ptr);
                    if nready == -1 {
                        crate::vm::mog_request_interrupt();
                    }
                }
            }

            // Safety: if nothing happened and nothing is pending, exit
            if processed == 0
                && el.timers.is_null()
                && el.ready_head.is_null()
                && el.watchers.is_null()
                && el.pending_count <= 0
            {
                break;
            }
        }

        el.running = 0;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_loop_step(loop_ptr: *mut u8) -> i32 {
    if loop_ptr.is_null() {
        return -1;
    }

    unsafe {
        let el = &mut *(loop_ptr as *mut MogEventLoop);

        if el.running != 0 {
            return 1;
        }

        let mut did_work = false;

        // Process timers and ready entries repeatedly until there is no immediate
        // forward progress.
        loop {
            let mut progress = false;

            // 1. Fire expired timers
            let t = now_ns();
            while !el.timers.is_null() && (*el.timers).deadline_ns <= t {
                let timer = el.timers;
                el.timers = (*timer).next;
                let future_ptr = (*timer).future as *mut u8;
                let result_val = (*timer).result_value;
                gc_external_free(timer as *mut u8);
                mog_future_complete(future_ptr, result_val);
                progress = true;
            }

            // 2. Process ready queue
            while !el.ready_head.is_null() {
                let f = el.ready_head;
                el.ready_head = (*f).next;
                if el.ready_head.is_null() {
                    el.ready_tail = ptr::null_mut();
                }
                (*f).next = ptr::null_mut();

                if !(*f).coro_handle.is_null() {
                    let hdl = (*f).coro_handle;
                    (*f).coro_handle = ptr::null_mut();
                    mog_coro_resume(hdl);
                    progress = true;
                }
            }

            if progress {
                did_work = true;
            }

            // 3. Poll descriptors with zero timeout to avoid blocking.
            if el.watchers.is_null() {
                break;
            }

            let mut read_fds: libc::fd_set = std::mem::zeroed();
            let mut write_fds: libc::fd_set = std::mem::zeroed();
            libc::FD_ZERO(&mut read_fds);
            libc::FD_ZERO(&mut write_fds);

            let mut max_fd: i32 = -1;
            let mut w = el.watchers;
            while !w.is_null() {
                if (*w).events & MOG_FD_READ != 0 {
                    libc::FD_SET((*w).fd, &mut read_fds);
                }
                if (*w).events & MOG_FD_WRITE != 0 {
                    libc::FD_SET((*w).fd, &mut write_fds);
                }
                if (*w).fd > max_fd {
                    max_fd = (*w).fd;
                }
                w = (*w).next;
            }

            let mut tv = libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            };

            let nready = if max_fd >= 0 {
                libc::select(
                    max_fd + 1,
                    &mut read_fds,
                    &mut write_fds,
                    ptr::null_mut(),
                    &mut tv,
                )
            } else {
                libc::select(
                    0,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut tv,
                )
            };

            if nready > 0 {
                progress = false;
                let mut prev_ptr: *mut *mut MogFdWatcher = &mut el.watchers;
                let mut watcher = el.watchers;
                while !watcher.is_null() {
                    let next = (*watcher).next;
                    let mut fired = false;
                    if ((*watcher).events & MOG_FD_READ != 0)
                        && libc::FD_ISSET((*watcher).fd, &read_fds)
                    {
                        fired = true;
                    }
                    if ((*watcher).events & MOG_FD_WRITE != 0)
                        && libc::FD_ISSET((*watcher).fd, &write_fds)
                    {
                        fired = true;
                    }

                    if fired {
                        *prev_ptr = next;
                        let watcher_fd = (*watcher).fd;
                        let watcher_future = (*watcher).future as *mut u8;

                        if watcher_fd == libc::STDIN_FILENO
                            && ((*watcher).events & MOG_FD_READ != 0)
                        {
                            let mut buf = [0u8; 4096];
                            let result = libc::fgets(
                                buf.as_mut_ptr() as *mut libc::c_char,
                                buf.len() as i32,
                                stdin_file(),
                            );
                            if !result.is_null() {
                                let len = libc::strlen(buf.as_ptr() as *const libc::c_char);
                                let mut slen = len;
                                if slen > 0 && buf[slen - 1] == b'\n' {
                                    slen -= 1;
                                    buf[slen] = 0;
                                }

                                let dest = gc_external_alloc(slen + 1);
                                if dest.is_null() {
                                    crate::vm::mog_request_interrupt();
                                    mog_future_complete(watcher_future, 0);
                                    gc_external_free(watcher as *mut u8);
                                    progress = true;
                                    watcher = next;
                                    continue;
                                }

                                ptr::copy_nonoverlapping(buf.as_ptr(), dest, slen + 1);
                                mog_future_complete(watcher_future, dest as i64);
                            } else {
                                mog_future_complete(watcher_future, 0);
                            }
                        } else {
                            mog_future_complete(watcher_future, watcher_fd as i64);
                        }

                        gc_external_free(watcher as *mut u8);
                        progress = true;
                    } else {
                        prev_ptr = &mut (*watcher).next;
                    }

                    watcher = next;
                }
            } else if nready == -1 {
                crate::vm::mog_request_interrupt();
                progress = true;
            }

            if !progress {
                break;
            }
            did_work = true;
        }

        if did_work {
            1
        } else if el.timers.is_null()
            && el.ready_head.is_null()
            && el.watchers.is_null()
            && el.pending_count <= 0
        {
            0
        } else {
            1
        }
    }
}

/// Get C stdin FILE*.
#[cfg(target_os = "macos")]
unsafe fn stdin_file() -> *mut libc::FILE {
    unsafe {
        unsafe extern "C" {
            static __stdinp: *mut libc::FILE;
        }
        __stdinp
    }
}

#[cfg(not(target_os = "macos"))]
unsafe fn stdin_file() -> *mut libc::FILE {
    unsafe {
        unsafe extern "C" {
            static stdin: *mut libc::FILE;
        }
        stdin
    }
}

// ---------------------------------------------------------------------------
// Async I/O
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn async_read_line(coro_handle: *mut u8) -> *mut u8 {
    let future_ptr = mog_future_new();
    let loop_ptr = mog_loop_get_global();
    if loop_ptr.is_null() {
        // Fallback: synchronous read
        unsafe {
            let mut buf = [0u8; 4096];
            let result = libc::fgets(
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len() as i32,
                stdin_file(),
            );
            if !result.is_null() {
                let len = libc::strlen(buf.as_ptr() as *const libc::c_char);
                let mut slen = len;
                if slen > 0 && buf[slen - 1] == b'\n' {
                    slen -= 1;
                    buf[slen] = 0;
                }
                let dest = gc_external_alloc(slen + 1);
                if dest.is_null() {
                    crate::vm::mog_request_interrupt();
                    mog_future_complete(future_ptr, 0);
                    return future_ptr;
                }
                ptr::copy_nonoverlapping(buf.as_ptr(), dest, slen + 1);
                mog_future_complete(future_ptr, dest as i64);
            } else {
                mog_future_complete(future_ptr, 0);
            }
        }
    } else {
        // Register stdin fd watcher — future will be completed when stdin is ready
        mog_loop_add_fd_watcher(loop_ptr, libc::STDIN_FILENO, future_ptr);
    }
    let _ = coro_handle; // available for caller to wire up separately
    future_ptr
}

// ---------------------------------------------------------------------------
// Combinators
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_all(futures: *const *mut u8, count: i32) -> *mut u8 {
    let parent_ptr = mog_future_new();
    if parent_ptr.is_null() {
        return ptr::null_mut();
    }
    if count == 0 {
        mog_future_complete(parent_ptr, 0);
        return parent_ptr;
    }

    unsafe {
        let parent = &mut *(parent_ptr as *mut MogFuture);

        let sub_futures_bytes =
            match (count as usize).checked_mul(std::mem::size_of::<*mut MogFuture>()) {
                Some(bytes) => bytes,
                None => {
                    crate::vm::mog_request_interrupt();
                    mog_future_set_error(parent_ptr, ERR_FUTURE_ERROR);
                    return parent_ptr;
                }
            };
        let sub_results_bytes = match (count as usize).checked_mul(std::mem::size_of::<i64>()) {
            Some(bytes) => bytes,
            None => {
                crate::vm::mog_request_interrupt();
                mog_future_set_error(parent_ptr, ERR_FUTURE_ERROR);
                return parent_ptr;
            }
        };

        let sub_futures = gc_external_alloc(sub_futures_bytes) as *mut *mut MogFuture;
        if sub_futures.is_null() {
            crate::vm::mog_request_interrupt();
            mog_future_set_error(parent_ptr, ERR_FUTURE_ERROR);
            return parent_ptr;
        }
        let sub_results = gc_external_alloc(sub_results_bytes) as *mut i64;
        if sub_results.is_null() {
            gc_external_free(sub_futures as *mut u8);
            crate::vm::mog_request_interrupt();
            mog_future_set_error(parent_ptr, ERR_FUTURE_ERROR);
            return parent_ptr;
        }
        ptr::write_bytes(sub_futures as *mut u8, 0, sub_futures_bytes);
        ptr::write_bytes(sub_results as *mut u8, 0, sub_results_bytes);

        parent.sub_futures = sub_futures;
        parent.sub_results = sub_results;
        parent.sub_count = count;
        parent.sub_done = 0;

        let mut already_done = 0;
        for i in 0..count as usize {
            let child = *(futures.add(i)) as *mut MogFuture;
            *parent.sub_futures.add(i) = child;
            (*child).parent = parent_ptr as *mut MogFuture;
            if (*child).state != MOG_FUTURE_PENDING {
                *parent.sub_results.add(i) = (*child).result;
                already_done += 1;
            }
        }
        parent.sub_done = already_done;

        if already_done >= count {
            let array_ptr = build_all_results_array(parent.sub_results, count);
            if array_ptr.is_null() {
                crate::vm::mog_request_interrupt();
                mog_future_set_error(parent_ptr, ERR_ALL_RESULT_ARRAY_OOM);
                return parent_ptr;
            }
            parent.result = array_ptr as i64;
            gc_external_free(parent.sub_results as *mut u8);
            parent.sub_results = ptr::null_mut();
            parent.state = MOG_FUTURE_READY;
            if !G_LOOP.is_null() {
                (*G_LOOP).pending_count -= 1; // was incremented in mog_future_new
            }
        }
    }

    parent_ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn mog_race(futures: *const *mut u8, count: i32) -> *mut u8 {
    let parent_ptr = mog_future_new();
    if count == 0 {
        mog_future_complete(parent_ptr, 0);
        return parent_ptr;
    }

    unsafe {
        let parent = &mut *(parent_ptr as *mut MogFuture);

        // race: no sub_futures array, just set parent on children
        for i in 0..count as usize {
            let child = *(futures.add(i)) as *mut MogFuture;
            (*child).parent = parent_ptr as *mut MogFuture;
            if (*child).state != MOG_FUTURE_PENDING && parent.state == MOG_FUTURE_PENDING {
                // Already done — complete parent immediately
                parent.state = MOG_FUTURE_READY;
                parent.result = (*child).result;
                if !G_LOOP.is_null() {
                    (*G_LOOP).pending_count -= 1;
                }
            }
        }
    }

    parent_ptr
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn mog_all_returns_indexable_array_results() {
        crate::gc::gc_set_memory_limits(0, 0);
        crate::gc::gc_init();
        crate::vm::mog_clear_interrupt();
        let first = mog_future_new();
        let second = mog_future_new();
        assert_ne!(first, ptr::null_mut());
        assert_ne!(second, ptr::null_mut());

        mog_future_complete(first, 45);
        mog_future_complete(second, 65);

        let futures = [first, second];
        let parent = mog_all(futures.as_ptr(), 2);
        assert_ne!(parent, ptr::null_mut());
        assert_eq!(mog_future_is_ready(parent), 1);

        let result = mog_future_get_result(parent) as *mut u8;
        assert_ne!(result, ptr::null_mut());
        assert_eq!(crate::array::array_length(result), 2);
        assert_eq!(crate::array::array_get(result, 0), 45);
        assert_eq!(crate::array::array_get(result, 1), 65);

        mog_future_free(parent);
        mog_future_free(first);
        mog_future_free(second);
    }

    #[test]
    fn mog_all_distinguishes_array_allocation_oom_from_generic_future_error() {
        crate::gc::gc_set_memory_limits(0, 0);
        crate::gc::gc_init();
        crate::vm::mog_clear_interrupt();

        let mut hit_distinct_error = false;
        let candidate_limits = [256usize, 272, 288, 304, 320, 336, 352, 368, 384];

        for max_memory in candidate_limits {
            crate::gc::gc_set_memory_limits(max_memory, max_memory);
            crate::gc::gc_init();
            crate::vm::mog_clear_interrupt();

            let first = mog_future_new();
            let second = mog_future_new();
            if first.is_null() || second.is_null() {
                if !first.is_null() {
                    mog_future_free(first);
                }
                if !second.is_null() {
                    mog_future_free(second);
                }
                continue;
            }

            mog_future_complete(first, 45);
            mog_future_complete(second, 65);

            let futures = [first, second];
            let parent = mog_all(futures.as_ptr(), 2);
            if parent.is_null() {
                mog_future_free(first);
                mog_future_free(second);
                continue;
            }

            assert_eq!(mog_future_is_ready(parent), 1);
            let result = mog_future_get_result(parent);

            if result == ERR_ALL_RESULT_ARRAY_OOM {
                hit_distinct_error = true;
                mog_future_free(parent);
                mog_future_free(first);
                mog_future_free(second);
                break;
            }

            mog_future_free(parent);
            mog_future_free(first);
            mog_future_free(second);
        }

        crate::gc::gc_set_memory_limits(0, 0);

        assert!(
            hit_distinct_error,
            "expected all() to surface array allocation failure with ERR_ALL_RESULT_ARRAY_OOM"
        );
    }
}
