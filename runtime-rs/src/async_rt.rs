// Mog async runtime — event loop, futures, coroutine scheduling.
//
// Port of runtime/mog_async.c.  All allocations use Box/Vec (malloc-backed),
// NOT the GC, to stay independent of the collector and avoid problems with
// LLVM coroutine frames.

use std::ptr;

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

fn sleep_ns(ns: u64) {
    let ts = libc::timespec {
        tv_sec: (ns / 1_000_000_000) as libc::time_t,
        tv_nsec: (ns % 1_000_000_000) as libc::c_long,
    };
    unsafe {
        libc::nanosleep(&ts, ptr::null_mut());
    }
}

// ---------------------------------------------------------------------------
// Future states
// ---------------------------------------------------------------------------

const MOG_FUTURE_PENDING: i32 = 0;
const MOG_FUTURE_READY: i32 = 1;
const MOG_FUTURE_ERROR: i32 = 2;

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
    let el = Box::new(MogEventLoop {
        ready_head: ptr::null_mut(),
        ready_tail: ptr::null_mut(),
        timers: ptr::null_mut(),
        watchers: ptr::null_mut(),
        running: 0,
        pending_count: 0,
    });
    Box::into_raw(el) as *mut u8
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
            drop(Box::from_raw(t));
            t = next;
        }

        // Free remaining fd watchers
        let mut w = el.watchers;
        while !w.is_null() {
            let next = (*w).next;
            drop(Box::from_raw(w));
            w = next;
        }

        drop(Box::from_raw(el));
    }
}

// ---------------------------------------------------------------------------
// Future operations
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn mog_future_new() -> *mut u8 {
    let f = Box::new(MogFuture {
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
    });
    let ptr = Box::into_raw(f);
    unsafe {
        if !G_LOOP.is_null() {
            (*G_LOOP).pending_count += 1;
        }
    }
    ptr as *mut u8
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
                        // All sub-futures done — complete parent with pointer to results
                        let results_ptr = parent.sub_results as i64;
                        let parent_ptr = future.parent as *mut u8;
                        mog_future_complete(parent_ptr, results_ptr);
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
            libc::free(future.coro_frame as *mut libc::c_void);
            future.coro_frame = ptr::null_mut();
        }
        // Free sub_futures array
        if !future.sub_futures.is_null() {
            libc::free(future.sub_futures as *mut libc::c_void);
        }
        // Free sub_results array
        if !future.sub_results.is_null() {
            libc::free(future.sub_results as *mut libc::c_void);
        }
        // Free the future itself
        drop(Box::from_raw(f as *mut MogFuture));
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
    let f = Box::new(MogFuture {
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
    });
    let ptr = Box::into_raw(f) as *mut u8;
    mog_loop_enqueue_ready(loop_ptr, ptr);
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
        let timer = Box::into_raw(Box::new(MogTimer {
            deadline_ns: now_ns() + delay_ms * 1_000_000,
            future,
            result_value,
            next: ptr::null_mut(),
        }));

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
        let w = Box::into_raw(Box::new(MogFdWatcher {
            fd,
            events,
            future: future as *mut MogFuture,
            next: el.watchers,
        }));
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
        let fn_ptr: fn(*mut u8) = std::mem::transmute(*(handle as *const usize));
        fn_ptr(handle);
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
                drop(Box::from_raw(timer));
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

            // 3. Check if anything remains
            if el.timers.is_null()
                && el.ready_head.is_null()
                && el.watchers.is_null()
                && el.pending_count <= 0
            {
                break;
            }

            // 4. Use select() to wait for fd events and/or timer deadlines
            if el.ready_head.is_null() && (!el.timers.is_null() || !el.watchers.is_null()) {
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

                // Calculate timeout from next timer
                let mut tv = libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                };
                let mut use_tv = false;

                if !el.timers.is_null() {
                    let tnow = now_ns();
                    if (*el.timers).deadline_ns > tnow {
                        let wait_ns = (*el.timers).deadline_ns - tnow;
                        tv.tv_sec = (wait_ns / 1_000_000_000) as libc::time_t;
                        tv.tv_usec = ((wait_ns % 1_000_000_000) / 1000) as libc::suseconds_t;
                    }
                    // else: deadline already passed, tv stays 0,0 (immediate)
                    use_tv = true;
                } else if el.watchers.is_null() || max_fd < 0 {
                    // Nothing to wait on — poll with 10ms
                    tv.tv_sec = 0;
                    tv.tv_usec = 10_000;
                    use_tv = true;
                }
                // else: no timers but have fd watchers — block indefinitely (tv_ptr = null)

                if max_fd >= 0 {
                    let tv_ptr = if use_tv {
                        &mut tv as *mut libc::timeval
                    } else {
                        ptr::null_mut()
                    };
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
                                        // Allocate via gc_alloc
                                        unsafe extern "C" {
                                            fn gc_alloc(size: usize) -> *mut u8;
                                        }
                                        let dest = gc_alloc(slen + 1);
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
                                drop(Box::from_raw(w));
                            } else {
                                prev_ptr = &mut (*w).next;
                            }
                            w = next;
                        }
                    }
                } else if use_tv {
                    // No fd watchers, just sleep for the timeout
                    let sleep_total = tv.tv_sec as u64 * 1_000_000_000 + tv.tv_usec as u64 * 1000;
                    if sleep_total > 0 {
                        sleep_ns(sleep_total);
                    }
                }
            }

            // Safety: if nothing happened and nothing is pending, exit
            if processed == 0
                && el.timers.is_null()
                && el.ready_head.is_null()
                && el.watchers.is_null()
            {
                break;
            }
        }

        el.running = 0;
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
                unsafe extern "C" {
                    fn gc_alloc(size: usize) -> *mut u8;
                }
                let dest = gc_alloc(slen + 1);
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
    if count == 0 {
        mog_future_complete(parent_ptr, 0);
        return parent_ptr;
    }

    unsafe {
        let parent = &mut *(parent_ptr as *mut MogFuture);

        // Allocate sub_futures and sub_results arrays via malloc (C-compatible)
        let sub_futures_bytes = (count as usize) * std::mem::size_of::<*mut MogFuture>();
        let _sub_results_bytes = (count as usize) * std::mem::size_of::<i64>();
        parent.sub_futures = libc::malloc(sub_futures_bytes) as *mut *mut MogFuture;
        parent.sub_results = libc::calloc(count as usize, std::mem::size_of::<i64>()) as *mut i64;
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
            parent.state = MOG_FUTURE_READY;
            parent.result = parent.sub_results as i64;
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
