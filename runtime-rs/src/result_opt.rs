// Result<T> and Optional<T> runtime helpers.
//
// Both are 16-byte GC-allocated tagged unions:
//   offset 0: tag  (i64)
//   offset 8: payload (i64)
//
// Result:   tag 0 = ok (payload = value), tag 1 = err (payload = pointer to error string)
// Optional: tag 0 = none (payload unused), tag 1 = some (payload = value)

unsafe extern "C" {
    fn gc_alloc(size: usize) -> *mut u8;
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn result_ok(value: i64) -> *mut u8 {
    unsafe {
        let ptr = gc_alloc(16);
        let slot = ptr as *mut i64;
        *slot = 0; // tag = ok
        *slot.add(1) = value;
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn result_err(msg: *const u8) -> *mut u8 {
    unsafe {
        let ptr = gc_alloc(16);
        let slot = ptr as *mut i64;
        *slot = 1; // tag = err
        *slot.add(1) = msg as i64;
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn result_is_ok(r: *const u8) -> i64 {
    if r.is_null() {
        return 0;
    }
    unsafe {
        let slot = r as *const i64;
        if *slot == 0 {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn result_is_err(r: *const u8) -> i64 {
    if r.is_null() {
        return 0;
    }
    unsafe {
        let slot = r as *const i64;
        if *slot == 1 {
            1
        } else {
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn result_unwrap(r: *const u8) -> i64 {
    if r.is_null() {
        return 0;
    }
    unsafe {
        let slot = r as *const i64;
        *slot.add(1)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn result_unwrap_err(r: *const u8) -> *mut u8 {
    if r.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let slot = r as *const i64;
        *slot.add(1) as *mut u8
    }
}

// ---------------------------------------------------------------------------
// Optional
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn optional_some(value: i64) -> *mut u8 {
    unsafe {
        let ptr = gc_alloc(16);
        let slot = ptr as *mut i64;
        *slot = 1; // tag = some
        *slot.add(1) = value;
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn optional_none() -> *mut u8 {
    unsafe {
        let ptr = gc_alloc(16);
        let slot = ptr as *mut i64;
        *slot = 0; // tag = none
        *slot.add(1) = 0;
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn optional_is_some(o: *const u8) -> i64 {
    if o.is_null() {
        return 0;
    }
    unsafe {
        let slot = o as *const i64;
        if *slot == 1 {
            1
        } else {
            0
        }
    }
}
