// Array operations for the Mog runtime.
//
// Every function is `extern "C"` + `#[unsafe(no_mangle)]` so that QBE-generated code
// can call them directly.
//
// The Array struct layout (40 bytes) matches the C definition exactly:
//   offset 0:  element_size    (u64)
//   offset 8:  dimension_count (u64)
//   offset 16: dimensions      (*mut u64)
//   offset 24: strides         (*mut u64)
//   offset 32: data            (*mut u8)

use std::ptr;

const ARRAY_BOUNDS_ERROR_EXIT: i32 = 2;

fn abort_array_oob(index: i64, length: i64) -> ! {
    eprintln!("runtime error: array index out of bounds: index={index}, length={length}");
    std::process::exit(ARRAY_BOUNDS_ERROR_EXIT);
}

const OBJ_ARRAY: i32 = 1;
const OBJ_STRING: i32 = 5;

unsafe extern "C" {
    fn gc_alloc_kind(size: usize, kind: i32) -> *mut u8;
    fn gc_alloc(size: usize) -> *mut u8;
    fn gc_add_root(slot: *mut *mut u8);
    fn gc_remove_root(slot: *mut *mut u8);
    fn gc_get_threshold() -> usize;
    fn gc_set_threshold(t: usize);
}

// ---------------------------------------------------------------------------
// Array struct — mirrors the C `typedef struct Array`.
// ---------------------------------------------------------------------------

#[repr(C)]
struct Array {
    element_size: u64,
    dimension_count: u64,
    dimensions: *mut u64,
    strides: *mut u64,
    data: *mut u8,
}

/// Cast a raw pointer to an `&Array`.
#[inline]
unsafe fn as_arr(ptr: *const u8) -> &'static Array {
    unsafe { &*(ptr as *const Array) }
}

/// Cast a raw pointer to an `&mut Array`.
#[inline]
unsafe fn as_arr_mut(ptr: *mut u8) -> &'static mut Array {
    unsafe { &mut *(ptr as *mut Array) }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a u64 element from array data at `index`.
#[inline]
unsafe fn read_elem(arr: &Array, index: u64) -> u64 {
    unsafe {
        let p = arr.data.add((index * arr.element_size) as usize) as *const u64;
        ptr::read_unaligned(p)
    }
}

/// Write a u64 element into array data at `index`.
#[inline]
unsafe fn write_elem(arr: &Array, index: u64, value: u64) {
    unsafe {
        let p = arr.data.add((index * arr.element_size) as usize) as *mut u64;
        ptr::write_unaligned(p, value);
    }
}

/// Allocate a GC-managed string of `len` content bytes (+ NUL terminator).
unsafe fn alloc_string(len: usize) -> *mut u8 {
    unsafe { gc_alloc_kind(len + 1, OBJ_STRING) }
}

#[inline]
unsafe fn with_gc_disabled<R>(f: impl FnOnce() -> R) -> R {
    let threshold = gc_get_threshold();
    gc_set_threshold(usize::MAX);
    let result = f();
    gc_set_threshold(threshold);
    result
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn array_alloc(
    element_size: u64,
    dimension_count: u64,
    dimensions: *const u64,
) -> *mut u8 {
    unsafe {
        with_gc_disabled(|| {
            let mut arr_ptr = gc_alloc_kind(std::mem::size_of::<Array>(), OBJ_ARRAY);
            if arr_ptr.is_null() {
                return ptr::null_mut();
            }
            let arr_root = std::ptr::addr_of_mut!(arr_ptr);
            gc_add_root(arr_root);

            let dim_bytes = (std::mem::size_of::<u64>()) * dimension_count as usize;
            let array_dimensions = gc_alloc(dim_bytes) as *mut u64;
            if array_dimensions.is_null() {
                gc_remove_root(arr_root);
                return ptr::null_mut();
            }

            let array_strides = gc_alloc(dim_bytes) as *mut u64;
            if array_strides.is_null() {
                gc_remove_root(arr_root);
                return ptr::null_mut();
            }

            let arr = as_arr_mut(arr_ptr);

            arr.element_size = element_size;
            arr.dimension_count = dimension_count;
            arr.dimensions = array_dimensions;
            arr.strides = array_strides;

            ptr::copy_nonoverlapping(dimensions, arr.dimensions, dimension_count as usize);

            let mut total_elements: u64 = 1;
            for i in 0..dimension_count as usize {
                *arr.strides.add(i) = total_elements;
                total_elements *= *arr.dimensions.add(i);
            }

            arr.data = gc_alloc((element_size * total_elements) as usize);
            if arr.data.is_null() {
                gc_remove_root(arr_root);
                return ptr::null_mut();
            }

            gc_remove_root(arr_root);
            arr_ptr
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_push(arr: *mut u8, value: i64) {
    unsafe {
        with_gc_disabled(|| {
            let mut arr = arr;
            if arr.is_null() {
                crate::vm::mog_request_interrupt();
                return;
            }
            let arr_root = std::ptr::addr_of_mut!(arr);
            gc_add_root(arr_root);
            let arr = as_arr_mut(arr);
            if arr.dimension_count == 0 {
                gc_remove_root(arr_root);
                return;
            }

            let old_len = *arr.dimensions;
            let new_len = old_len + 1;
            let new_data = gc_alloc((arr.element_size * new_len) as usize);
            if new_data.is_null() {
                gc_remove_root(arr_root);
                crate::vm::mog_request_interrupt();
                return;
            }
            if old_len > 0 && !arr.data.is_null() {
                ptr::copy_nonoverlapping(arr.data, new_data, (arr.element_size * old_len) as usize);
            }

            let p = new_data.add((old_len * arr.element_size) as usize) as *mut u64;
            ptr::write_unaligned(p, value as u64);

            arr.data = new_data;
            *arr.dimensions = new_len;
            gc_remove_root(arr_root);
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_pop(arr: *mut u8) -> i64 {
    unsafe {
        let arr = as_arr_mut(arr);
        if arr.dimension_count == 0 || *arr.dimensions == 0 {
            return 0;
        }

        let old_len = *arr.dimensions;
        let new_len = old_len - 1;

        let value = read_elem(arr, new_len);
        *arr.dimensions = new_len;
        value as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_get(arr: *const u8, index: i64) -> i64 {
    unsafe {
        let arr = as_arr(arr);
        if arr.dimension_count == 0 {
            abort_array_oob(index, 0);
        }

        let len = arr.dimensions.read() as i64;
        if index < 0 || index as u64 >= arr.dimensions.read() {
            abort_array_oob(index, len);
        }

        read_elem(arr, index as u64) as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_set(arr: *mut u8, index: i64, value: i64) {
    unsafe {
        let arr = as_arr(arr);
        if arr.dimension_count == 0 {
            abort_array_oob(index, 0);
        }

        let len = arr.dimensions.read() as i64;
        if index < 0 || index as u64 >= arr.dimensions.read() {
            abort_array_oob(index, len);
        }

        write_elem(arr, index as u64, value as u64);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_length(arr: *const u8) -> i64 {
    unsafe {
        let arr = as_arr(arr);
        if arr.dimension_count == 0 {
            return 0;
        }
        *arr.dimensions as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_slice(arr: *const u8, start: i64, end: i64) -> *mut u8 {
    unsafe {
        with_gc_disabled(|| {
            let src = as_arr(arr);
            let start = start as u64;
            let mut slice_len = end as u64 - start;
            let src_len = *src.dimensions;
            if slice_len > src_len - start {
                slice_len = src_len - start;
            }

            let mut slice_ptr = gc_alloc_kind(std::mem::size_of::<Array>(), OBJ_ARRAY);
            if slice_ptr.is_null() {
                return ptr::null_mut();
            }
            let slice_root = std::ptr::addr_of_mut!(slice_ptr);
            gc_add_root(slice_root);

            let sl = as_arr_mut(slice_ptr);
            sl.element_size = src.element_size;
            sl.dimension_count = 1;
            sl.dimensions = gc_alloc(std::mem::size_of::<u64>()) as *mut u64;
            if sl.dimensions.is_null() {
                gc_remove_root(slice_root);
                return ptr::null_mut();
            }
            *sl.dimensions = slice_len;
            sl.strides = gc_alloc(std::mem::size_of::<u64>()) as *mut u64;
            if sl.strides.is_null() {
                gc_remove_root(slice_root);
                return ptr::null_mut();
            }
            *sl.strides = 1;
            sl.data = gc_alloc((src.element_size * slice_len) as usize);
            if sl.data.is_null() {
                gc_remove_root(slice_root);
                return ptr::null_mut();
            }

            for i in 0..slice_len {
                let val = read_elem(src, start + i);
                write_elem(sl, i, val);
            }

            gc_remove_root(slice_root);
            slice_ptr
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_concat(a: *const u8, b: *const u8) -> *mut u8 {
    unsafe {
        let arr_a = as_arr(a);
        let arr_b = as_arr(b);

        let len_a = if arr_a.dimension_count > 0 {
            *arr_a.dimensions
        } else {
            0
        };
        let len_b = if arr_b.dimension_count > 0 {
            *arr_b.dimensions
        } else {
            0
        };
        let total = len_a + len_b;
        let elem_size = arr_a.element_size;

        let dims: [u64; 1] = [total];
        let result = array_alloc(elem_size, 1, dims.as_ptr());
        let res = as_arr(result);

        if len_a > 0 {
            ptr::copy_nonoverlapping(arr_a.data, res.data, (elem_size * len_a) as usize);
        }
        if len_b > 0 {
            ptr::copy_nonoverlapping(
                arr_b.data,
                res.data.add((elem_size * len_a) as usize),
                (elem_size * len_b) as usize,
            );
        }

        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_contains(arr: *const u8, value: i64) -> i64 {
    unsafe {
        let arr = as_arr(arr);
        if arr.dimension_count == 0 {
            return 0;
        }
        let len = *arr.dimensions;
        let value = value as u64;
        for i in 0..len {
            if read_elem(arr, i) == value {
                return 1;
            }
        }
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_index_of(arr: *const u8, value: i64) -> i64 {
    unsafe {
        let arr = as_arr(arr);
        if arr.dimension_count == 0 {
            return -1;
        }
        let len = *arr.dimensions;
        let value = value as u64;
        for i in 0..len {
            if read_elem(arr, i) == value {
                return i as i64;
            }
        }
        -1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_reverse(arr: *mut u8) {
    unsafe {
        let arr = as_arr_mut(arr);
        if arr.dimension_count == 0 || *arr.dimensions <= 1 {
            return;
        }

        let len = *arr.dimensions;
        for i in 0..len / 2 {
            let j = len - 1 - i;
            let a = read_elem(arr, i);
            let b = read_elem(arr, j);
            write_elem(arr, i, b);
            write_elem(arr, j, a);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_sort(arr: *mut u8) {
    unsafe {
        let arr = as_arr_mut(arr);
        if arr.dimension_count == 0 || *arr.dimensions <= 1 {
            return;
        }

        let len = *arr.dimensions as usize;
        let elem_size = arr.element_size as usize;

        // Build a slice of i64 values, sort, write back.
        // This matches the C implementation which uses qsort with i64 comparison.
        if elem_size == 8 {
            let data = arr.data as *mut i64;
            let slice = std::slice::from_raw_parts_mut(data, len);
            slice.sort_unstable();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_sort_with_comparator(
    arr: *mut u8,
    closure_fn: *const u8,
    closure_env: *const u8,
) {
    unsafe {
        let arr = as_arr_mut(arr);
        if arr.dimension_count == 0 || *arr.dimensions <= 1 {
            return;
        }

        let len = *arr.dimensions as usize;

        // Closure calling convention: fn(env, a, b) -> i64
        let cmp: fn(i64, i64, i64) -> i64 = std::mem::transmute(closure_fn);
        let env = closure_env as i64;

        if arr.element_size == 8 {
            let data = arr.data as *mut i64;
            let slice = std::slice::from_raw_parts_mut(data, len);
            slice.sort_unstable_by(|a, b| {
                let result = cmp(env, *a, *b);
                if result < 0 {
                    std::cmp::Ordering::Less
                } else if result > 0 {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_fill(arr: *mut u8, value: i64, count: i64) {
    unsafe {
        let arr = as_arr_mut(arr);
        let count = count as u64;
        for i in 0..count {
            write_elem(arr, i, value as u64);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_join(arr: *const u8, delim: *const u8) -> *mut u8 {
    unsafe {
        let arr_s = as_arr(arr);
        if arr_s.dimension_count == 0 || *arr_s.dimensions == 0 {
            let result = alloc_string(0);
            *result = 0;
            return result;
        }

        let len = *arr_s.dimensions;
        let delim_bytes = if delim.is_null() {
            &[] as &[u8]
        } else {
            std::ffi::CStr::from_ptr(delim as *const i8).to_bytes()
        };

        // Each element is a pointer to a C string (i64-sized slot).
        // Collect string pointers and compute total length.
        let mut total_len: usize = 0;
        let mut parts: Vec<&[u8]> = Vec::with_capacity(len as usize);
        for i in 0..len {
            let str_ptr = read_elem(arr_s, i) as *const u8;
            let bytes = if str_ptr.is_null() {
                &[] as &[u8]
            } else {
                std::ffi::CStr::from_ptr(str_ptr as *const i8).to_bytes()
            };
            total_len += bytes.len();
            parts.push(bytes);
        }
        total_len += delim_bytes.len() * (len as usize - 1);

        let result = alloc_string(total_len);
        let mut pos: usize = 0;
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                ptr::copy_nonoverlapping(delim_bytes.as_ptr(), result.add(pos), delim_bytes.len());
                pos += delim_bytes.len();
            }
            ptr::copy_nonoverlapping(part.as_ptr(), result.add(pos), part.len());
            pos += part.len();
        }
        *result.add(total_len) = 0;
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_filter(
    arr: *const u8,
    closure_fn: *const u8,
    closure_env: *const u8,
) -> *mut u8 {
    unsafe {
        let src = as_arr(arr);
        if src.dimension_count == 0 {
            let dims: [u64; 1] = [0];
            return array_alloc(8, 1, dims.as_ptr());
        }

        let len = *src.dimensions;
        let elem_size = src.element_size;

        // Closure calling convention: fn(env, elem) -> i64 (truthy/falsy)
        let predicate: fn(i64, i64) -> i64 = std::mem::transmute(closure_fn);
        let env = closure_env as i64;

        // First pass: determine which elements to keep.
        let mut keep = Vec::with_capacity(len as usize);
        let mut keep_count: u64 = 0;
        for i in 0..len {
            let elem = read_elem(src, i) as i64;
            if predicate(env, elem) != 0 {
                keep.push(true);
                keep_count += 1;
            } else {
                keep.push(false);
            }
        }

        // Allocate result array.
        let dims: [u64; 1] = [keep_count];
        let result = array_alloc(elem_size, 1, dims.as_ptr());
        let res = as_arr(result);

        // Second pass: copy kept elements.
        let mut j: u64 = 0;
        for i in 0..len {
            if keep[i as usize] {
                let val = read_elem(src, i);
                write_elem(res, j, val);
                j += 1;
            }
        }

        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_map(
    arr: *const u8,
    closure_fn: *const u8,
    closure_env: *const u8,
) -> *mut u8 {
    unsafe {
        let src = as_arr(arr);
        if src.dimension_count == 0 {
            let dims: [u64; 1] = [0];
            return array_alloc(8, 1, dims.as_ptr());
        }

        let len = *src.dimensions;
        let elem_size = src.element_size;

        // Closure calling convention: fn(env, elem) -> i64
        let map_fn: fn(i64, i64) -> i64 = std::mem::transmute(closure_fn);
        let env = closure_env as i64;

        let dims: [u64; 1] = [len];
        let result = array_alloc(elem_size, 1, dims.as_ptr());
        let res = as_arr(result);

        for i in 0..len {
            let elem = read_elem(src, i) as i64;
            let mapped = map_fn(env, elem);
            write_elem(res, i, mapped as u64);
        }

        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_new_multidim(ndim: u64, dim0: u64, dim1: u64) -> *mut u8 {
    let dims: [u64; 2] = [dim0, dim1];
    // For ndim==1, only dim0 is meaningful; for ndim==2, both are used.
    array_alloc(8, ndim, dims.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn array_get_2d(arr: *const u8, row: i64, col: i64) -> i64 {
    unsafe {
        let arr = as_arr(arr);
        // strides[0] is the number of elements per row step,
        // strides[1] is the number of elements per column step.
        // In the C runtime, strides are computed as cumulative products:
        //   strides[0] = 1, strides[1] = dimensions[0]
        // So the linear index = row * strides[1] + col * strides[0]
        //                     = row * dimensions[0] + col
        // But actually the C code computes: strides[0]=1, strides[1]=dim[0]
        // and array_get/set use index*element_size. For 2D we use
        // row * cols + col as the linear index where cols = dimensions[1].
        //
        // Re-examining the C code:
        //   for i in 0..ndim: strides[i] = total; total *= dimensions[i]
        // So strides[0] = 1, strides[1] = dim[0].
        // The standard row-major index for (row, col) in a dim[0] x dim[1] array
        // where strides[0]=1, strides[1]=dim[0]:
        //   linear = row * strides[1] + col * strides[0] = row * dim[0] + col
        //
        // This is column-major. Let's just use: index = row * dim[1] + col
        // which is the conventional row-major layout when dims = [rows, cols]
        // Actually, let's match the C stride layout exactly:
        //   index = row * strides[1] + col * strides[0]
        let index = row as u64 * *arr.strides.add(1) + col as u64 * *arr.strides.add(0);
        read_elem(arr, index) as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_set_2d(arr: *mut u8, row: i64, col: i64, value: i64) {
    unsafe {
        let arr = as_arr(arr);
        let index = row as u64 * *arr.strides.add(1) + col as u64 * *arr.strides.add(0);
        write_elem(arr, index, value as u64);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn array_dimensions(arr: *const u8) -> *mut u8 {
    unsafe {
        let src = as_arr(arr);
        let ndim = src.dimension_count;

        // Return a 1-D array of dimension values.
        let dims: [u64; 1] = [ndim];
        let result = array_alloc(8, 1, dims.as_ptr());
        let res = as_arr(result);

        for i in 0..ndim {
            let dim_val = *src.dimensions.add(i as usize);
            write_elem(res, i, dim_val);
        }

        result
    }
}
