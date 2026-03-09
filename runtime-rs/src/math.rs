//! Math builtins and vector/matrix operations for the Mog runtime.
use std::ptr;

// ---------------------------------------------------------------------------
// GC externs
// ---------------------------------------------------------------------------
unsafe extern "C" {
    fn gc_alloc(size: usize) -> *mut u8;
    fn gc_alloc_kind(size: usize, kind: i32) -> *mut u8;
    fn gc_add_root(slot: *mut *mut u8);
    fn gc_remove_root(slot: *mut *mut u8);
}

/// ObjectKind::OBJ_ARRAY = 1
const OBJ_ARRAY: i32 = 1;

// ---------------------------------------------------------------------------
// Array struct layout (matches the C runtime)
//
//   offset  0: element_size    (u64)
//   offset  8: dimension_count (u64)
//   offset 16: dimensions ptr  (*mut u64)
//   offset 24: strides ptr     (*mut u64)
//   offset 32: data ptr        (*mut u8)
//
// Total header: 40 bytes
// ---------------------------------------------------------------------------

#[repr(C)]
struct Array {
    element_size: u64,
    dimension_count: u64,
    dimensions: *mut u64,
    strides: *mut u64,
    data: *mut u8,
}

/// Allocate a new 1-D Array of f64 (element_size = 8) with `len` elements.
unsafe fn array_alloc_1d(len: u64) -> *mut Array {
    let mut arr = unsafe { gc_alloc_kind(std::mem::size_of::<Array>(), OBJ_ARRAY) } as *mut Array;
    if arr.is_null() {
        return ptr::null_mut();
    }
    let arr_root: *mut *mut u8 = std::ptr::addr_of_mut!(arr).cast();
    unsafe {
        gc_add_root(arr_root);

        (*arr).element_size = 8;
        (*arr).dimension_count = 1;

        let dims = gc_alloc(std::mem::size_of::<u64>()) as *mut u64;
        if dims.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        *dims = len;
        (*arr).dimensions = dims;

        let strides = gc_alloc(std::mem::size_of::<u64>()) as *mut u64;
        if strides.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        *strides = 1;
        (*arr).strides = strides;

        let data = gc_alloc((len as usize) * 8);
        if data.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        (*arr).data = data;

        gc_remove_root(arr_root);
        arr
    }
}

/// Allocate a new 2-D Array of f64 with `rows x cols`.
unsafe fn array_alloc_2d(rows: u64, cols: u64) -> *mut Array {
    let mut arr = unsafe { gc_alloc_kind(std::mem::size_of::<Array>(), OBJ_ARRAY) } as *mut Array;
    if arr.is_null() {
        return ptr::null_mut();
    }
    let arr_root: *mut *mut u8 = std::ptr::addr_of_mut!(arr).cast();
    unsafe {
        gc_add_root(arr_root);

        (*arr).element_size = 8;
        (*arr).dimension_count = 2;

        let dims = gc_alloc(2 * std::mem::size_of::<u64>()) as *mut u64;
        if dims.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        *dims = rows;
        *dims.add(1) = cols;
        (*arr).dimensions = dims;

        let strides = gc_alloc(2 * std::mem::size_of::<u64>()) as *mut u64;
        if strides.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        *strides = cols;
        *strides.add(1) = 1;
        (*arr).strides = strides;

        let data = gc_alloc((rows as usize) * (cols as usize) * 8);
        if data.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        (*arr).data = data;

        gc_remove_root(arr_root);
        arr
    }
}

/// Get pointer to f64 data of an Array.
unsafe fn array_data(a: *const Array) -> *const f64 {
    unsafe { (*a).data as *const f64 }
}

/// Get mutable pointer to f64 data of an Array.
unsafe fn array_data_mut(a: *mut Array) -> *mut f64 {
    unsafe { (*a).data as *mut f64 }
}

/// Total number of elements in an array.
unsafe fn array_len(a: *const Array) -> u64 {
    let mut total: u64 = 1;
    unsafe {
        for i in 0..(*a).dimension_count {
            total *= *(*a).dimensions.add(i as usize);
        }
    }
    total
}

// ===========================================================================
// Scalar math builtins
// ===========================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_sqrt(x: f64) -> f64 {
    x.sqrt()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_sin(x: f64) -> f64 {
    x.sin()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_cos(x: f64) -> f64 {
    x.cos()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_tan(x: f64) -> f64 {
    x.tan()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_asin(x: f64) -> f64 {
    x.asin()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_acos(x: f64) -> f64 {
    x.acos()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_atan2(y: f64, x: f64) -> f64 {
    y.atan2(x)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_exp(x: f64) -> f64 {
    x.exp()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_log(x: f64) -> f64 {
    x.ln()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_log2(x: f64) -> f64 {
    x.log2()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_floor(x: f64) -> f64 {
    x.floor()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_ceil(x: f64) -> f64 {
    x.ceil()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_round(x: f64) -> f64 {
    x.round()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_abs(x: f64) -> f64 {
    x.abs()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_min(a: f64, b: f64) -> f64 {
    a.min(b)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mog_max(a: f64, b: f64) -> f64 {
    a.max(b)
}

// ===========================================================================
// Element-wise vector operations (operate on Array structs)
//
// Each function takes two Array pointers, produces a new GC-allocated Array
// of the same shape with the element-wise result.
// ===========================================================================

macro_rules! vector_binop {
    ($name:ident, $op:tt) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(a: *const u8, b: *const u8) -> *mut u8 {
            let a = a as *const Array;
            let b = b as *const Array;
            let len = unsafe { array_len(a) };
            let result = unsafe { array_alloc_1d(len) };
            let ad = unsafe { array_data(a) };
            let bd = unsafe { array_data(b) };
            let rd = unsafe { array_data_mut(result) };
            for i in 0..len as usize {
                unsafe { *rd.add(i) = *ad.add(i) $op *bd.add(i) };
            }
            result as *mut u8
        }
    };
}

vector_binop!(vector_add, +);
vector_binop!(vector_sub, -);
vector_binop!(vector_mul, *);
vector_binop!(vector_div, /);

/// Dot product of two 1-D arrays. Returns the scalar result as f64.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vector_dot(a: *const u8, b: *const u8) -> f64 {
    let a = a as *const Array;
    let b = b as *const Array;
    let len_a = unsafe { array_len(a) };
    let len_b = unsafe { array_len(b) };
    let len = len_a.min(len_b) as usize;
    let ad = unsafe { array_data(a) };
    let bd = unsafe { array_data(b) };
    let mut sum: f64 = 0.0;
    for i in 0..len {
        unsafe { sum += *ad.add(i) * *bd.add(i) };
    }
    sum
}

// ===========================================================================
// Vector-scalar operations
// ===========================================================================

macro_rules! vector_scalar_op {
    ($name:ident, $op:tt) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(a: *const u8, scalar: f64) -> *mut u8 {
            let a = a as *const Array;
            let len = unsafe { array_len(a) };
            let result = unsafe { array_alloc_1d(len) };
            let ad = unsafe { array_data(a) };
            let rd = unsafe { array_data_mut(result) };
            for i in 0..len as usize {
                unsafe { *rd.add(i) = *ad.add(i) $op scalar };
            }
            result as *mut u8
        }
    };
}

vector_scalar_op!(vector_scalar_add, +);
vector_scalar_op!(vector_scalar_sub, -);
vector_scalar_op!(vector_scalar_mul, *);
vector_scalar_op!(vector_scalar_div, /);

// ===========================================================================
// Matrix multiplication (operates on 2-D Array structs)
// ===========================================================================

/// Matrix multiply: a (M×K) * b (K×N) -> result (M×N).
/// Both arrays must be 2-D with element_size = 8 (f64).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn matrix_multiply(a: *const u8, b: *const u8) -> *mut u8 {
    let a = a as *const Array;
    let b = b as *const Array;

    if a.is_null() || b.is_null() {
        return std::ptr::null_mut();
    }

    unsafe {
        let a_rows = *(*a).dimensions;
        let a_cols = if (*a).dimension_count > 1 {
            *(*a).dimensions.add(1)
        } else {
            1
        };
        let b_cols = if (*b).dimension_count > 1 {
            *(*b).dimensions.add(1)
        } else {
            1
        };

        let result = array_alloc_2d(a_rows, b_cols);
        let ad = array_data(a);
        let bd = array_data(b);
        let rd = array_data_mut(result);

        for i in 0..a_rows as usize {
            for j in 0..b_cols as usize {
                let mut sum: f64 = 0.0;
                for k in 0..a_cols as usize {
                    sum += *ad.add(i * a_cols as usize + k) * *bd.add(k * b_cols as usize + j);
                }
                *rd.add(i * b_cols as usize + j) = sum;
            }
        }

        result as *mut u8
    }
}
