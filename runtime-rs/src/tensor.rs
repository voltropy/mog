//! Tensor (n-dimensional array of f32) operations for the Mog runtime.

use std::io::Write;
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

/// ObjectKind::OBJ_TENSOR = 7
const OBJ_TENSOR: i32 = 7;
/// ObjectKind::OBJ_ARRAY = 1
const OBJ_ARRAY: i32 = 1;

// ---------------------------------------------------------------------------
// MogTensor struct layout (matches codegen expectations — 40 bytes)
//
//   offset  0: ndim   (i64)
//   offset  8: shape  (*mut i64)
//   offset 16: strides (*mut i64)
//   offset 24: data   (*mut f32)
//   offset 32: dtype  (i64)
// ---------------------------------------------------------------------------

#[repr(C)]
struct MogTensor {
    ndim: i64,
    shape: *mut i64,
    strides: *mut i64,
    data: *mut f32,
    dtype: i64,
}

/// Array layout (for tensor_shape return value)
#[repr(C)]
struct Array {
    element_size: u64,
    dimension_count: u64,
    dimensions: *mut u64,
    strides: *mut u64,
    data: *mut u8,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute total number of elements from shape.
unsafe fn compute_numel(ndim: i64, shape: *const i64) -> i64 {
    let mut size: i64 = 1;
    for i in 0..ndim as usize {
        size *= unsafe { *shape.add(i) };
    }
    size
}

/// Get a tensor reference from an opaque pointer.
unsafe fn as_tensor(ptr: *const u8) -> &'static MogTensor {
    unsafe { &*(ptr as *const MogTensor) }
}

/// Get a mutable tensor reference from an opaque pointer.
#[allow(clippy::mut_from_ref)]
unsafe fn as_tensor_mut(ptr: *mut u8) -> &'static mut MogTensor {
    unsafe { &mut *(ptr as *mut MogTensor) }
}

// ===========================================================================
// Creation
// ===========================================================================

/// Create a new zero-initialized tensor with the given shape and dtype.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_create(ndim: i64, shape: *const i64, dtype: i64) -> *mut u8 {
    unsafe {
        let mut t = gc_alloc_kind(std::mem::size_of::<MogTensor>(), OBJ_TENSOR) as *mut MogTensor;
        if t.is_null() {
            return ptr::null_mut();
        }
        let tensor_root: *mut *mut u8 = std::ptr::addr_of_mut!(t).cast();
        gc_add_root(tensor_root);

        (*t).ndim = ndim;
        (*t).dtype = dtype;

        // Allocate and fill shape
        let shape_buf = gc_alloc((ndim as usize) * std::mem::size_of::<i64>()) as *mut i64;
        if shape_buf.is_null() {
            gc_remove_root(tensor_root);
            return ptr::null_mut();
        }
        let strides_buf = gc_alloc((ndim as usize) * std::mem::size_of::<i64>()) as *mut i64;
        if strides_buf.is_null() {
            gc_remove_root(tensor_root);
            return ptr::null_mut();
        }

        let mut size: i64 = 1;
        // Compute row-major strides from the right
        for i in (0..ndim as usize).rev() {
            let dim = *shape.add(i);
            *shape_buf.add(i) = dim;
            *strides_buf.add(i) = size;
            size *= dim;
        }

        (*t).shape = shape_buf;
        (*t).strides = strides_buf;

        // Allocate zero-initialized data
        let data = gc_alloc((size as usize) * std::mem::size_of::<f32>()) as *mut f32;
        if data.is_null() {
            gc_remove_root(tensor_root);
            return ptr::null_mut();
        }
        std::ptr::write_bytes(data, 0, size as usize);
        (*t).data = data;

        gc_remove_root(tensor_root);
        t as *mut u8
    }
}

/// Create a tensor and copy data from the provided pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_create_with_data(
    ndim: i64,
    shape: *const i64,
    data: *const f32,
    dtype: i64,
) -> *mut u8 {
    unsafe {
        let ptr = tensor_create(ndim, shape, dtype);
        let t = as_tensor(ptr);
        let numel = compute_numel(ndim, shape) as usize;
        std::ptr::copy_nonoverlapping(data, t.data, numel);
        ptr
    }
}

// ===========================================================================
// Accessors
// ===========================================================================

/// Print a tensor to stdout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_print(t: *const u8) {
    unsafe {
        let t = as_tensor(t);
        let numel = compute_numel(t.ndim, t.shape);
        let mut out = std::io::stdout().lock();

        let _ = write!(out, "tensor(");

        if t.ndim == 1 {
            let _ = write!(out, "[");
            for i in 0..numel {
                if i > 0 {
                    let _ = write!(out, ", ");
                }
                let _ = write!(out, "{:.4}", *t.data.add(i as usize));
            }
            let _ = write!(out, "]");
        } else if t.ndim == 2 {
            let rows = *t.shape;
            let cols = *t.shape.add(1);
            let _ = write!(out, "[");
            for i in 0..rows {
                if i > 0 {
                    let _ = write!(out, ", ");
                }
                let _ = write!(out, "[");
                for j in 0..cols {
                    if j > 0 {
                        let _ = write!(out, ", ");
                    }
                    let _ = write!(out, "{:.4}", *t.data.add((i * cols + j) as usize));
                }
                let _ = write!(out, "]");
            }
            let _ = write!(out, "]");
        } else {
            // Generic: just print flat data for higher dimensions
            let _ = write!(out, "[");
            for i in 0..numel {
                if i > 0 {
                    let _ = write!(out, ", ");
                }
                let _ = write!(out, "{:.4}", *t.data.add(i as usize));
            }
            let _ = write!(out, "]");
        }

        let _ = write!(out, ", shape=[");
        for i in 0..t.ndim {
            if i > 0 {
                let _ = write!(out, ", ");
            }
            let _ = write!(out, "{}", *t.shape.add(i as usize));
        }
        let _ = writeln!(out, "])");
        let _ = out.flush();
    }
}

/// Return an Array containing the shape dimensions (as u64 values).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_shape(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        let ndim = t.ndim as usize;

        let mut arr = gc_alloc_kind(std::mem::size_of::<Array>(), OBJ_ARRAY) as *mut Array;
        if arr.is_null() {
            return ptr::null_mut();
        }
        let arr_root: *mut *mut u8 = std::ptr::addr_of_mut!(arr).cast();
        gc_add_root(arr_root);

        (*arr).element_size = 8; // i64 / u64
        (*arr).dimension_count = 1;

        let dims = gc_alloc(std::mem::size_of::<u64>()) as *mut u64;
        if dims.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        *dims = ndim as u64;
        (*arr).dimensions = dims;

        let strides = gc_alloc(std::mem::size_of::<u64>()) as *mut u64;
        if strides.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        *strides = 1;
        (*arr).strides = strides;

        let data = gc_alloc(ndim * std::mem::size_of::<u64>()) as *mut u64;
        if data.is_null() {
            gc_remove_root(arr_root);
            return ptr::null_mut();
        }
        for i in 0..ndim {
            *data.add(i) = *t.shape.add(i) as u64;
        }
        (*arr).data = data as *mut u8;

        gc_remove_root(arr_root);
        arr as *mut u8
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_ndim(t: *const u8) -> i64 {
    unsafe { as_tensor(t).ndim }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_dtype(t: *const u8) -> i64 {
    unsafe { as_tensor(t).dtype }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_numel(t: *const u8) -> i64 {
    unsafe {
        let t = as_tensor(t);
        compute_numel(t.ndim, t.shape)
    }
}

/// Get element at flat index; returns f64 for codegen convenience.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_get_f32(t: *const u8, index: i64) -> f64 {
    unsafe {
        let t = as_tensor(t);
        *t.data.add(index as usize) as f64
    }
}

/// Set element at flat index; value comes in as f64.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_set_f32(t: *mut u8, index: i64, value: f64) {
    unsafe {
        let t = as_tensor_mut(t);
        *t.data.add(index as usize) = value as f32;
    }
}

// ===========================================================================
// Element-wise arithmetic
// ===========================================================================

/// Helper: create a new tensor with the same shape as `src`, zero-initialized.
unsafe fn tensor_like(src: &MogTensor) -> *mut u8 {
    unsafe { tensor_create(src.ndim, src.shape, src.dtype) }
}

/// Helper: get numel from a tensor ref.
unsafe fn t_numel(t: &MogTensor) -> usize {
    unsafe { compute_numel(t.ndim, t.shape) as usize }
}

macro_rules! tensor_binop {
    ($name:ident, $op:tt) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(a: *const u8, b: *const u8) -> *mut u8 {
            unsafe {
                let a = as_tensor(a);
                let b = as_tensor(b);
                let ptr = tensor_like(a);
                let r = as_tensor(ptr);
                let n = t_numel(a);
                for i in 0..n {
                    *r.data.add(i) = *a.data.add(i) $op *b.data.add(i);
                }
                ptr
            }
        }
    };
}

tensor_binop!(tensor_add, +);
tensor_binop!(tensor_sub, -);
tensor_binop!(tensor_mul, *);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_neg(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        let ptr = tensor_like(t);
        let r = as_tensor(ptr);
        let n = t_numel(t);
        for i in 0..n {
            *r.data.add(i) = -*t.data.add(i);
        }
        ptr
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_sum(t: *const u8) -> f64 {
    unsafe {
        let t = as_tensor(t);
        let n = t_numel(t);
        let mut sum: f64 = 0.0;
        for i in 0..n {
            sum += *t.data.add(i) as f64;
        }
        sum
    }
}

// ===========================================================================
// Matrix operations
// ===========================================================================

/// Matrix multiply: a (M×K) * b (K×N) -> result (M×N).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_matmul(a: *const u8, b: *const u8) -> *mut u8 {
    unsafe {
        let a = as_tensor(a);
        let b = as_tensor(b);
        let m = *a.shape;
        let k = *a.shape.add(1);
        let n = *b.shape.add(1);

        let result_shape: [i64; 2] = [m, n];
        let ptr = tensor_create(2, result_shape.as_ptr(), 0);
        let r = as_tensor(ptr);

        for i in 0..m as usize {
            for j in 0..n as usize {
                let mut sum: f32 = 0.0;
                for kk in 0..k as usize {
                    sum += *a.data.add(i * k as usize + kk) * *b.data.add(kk * n as usize + j);
                }
                *r.data.add(i * n as usize + j) = sum;
            }
        }

        ptr
    }
}

/// Transpose the last two dimensions of a tensor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_transpose(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        if t.ndim < 2 {
            // Return the tensor as-is for < 2D
            return t as *const MogTensor as *mut u8;
        }

        let ndim = t.ndim as usize;

        // Build new shape: swap last two dims
        let new_shape_buf = gc_alloc(ndim * std::mem::size_of::<i64>()) as *mut i64;
        for i in 0..ndim.saturating_sub(2) {
            *new_shape_buf.add(i) = *t.shape.add(i);
        }
        *new_shape_buf.add(ndim - 2) = *t.shape.add(ndim - 1);
        *new_shape_buf.add(ndim - 1) = *t.shape.add(ndim - 2);

        let ptr = tensor_create(t.ndim, new_shape_buf, t.dtype);
        let r = as_tensor(ptr);

        if t.ndim == 2 {
            let rows = *t.shape as usize;
            let cols = *t.shape.add(1) as usize;
            for i in 0..rows {
                for j in 0..cols {
                    *r.data.add(j * rows + i) = *t.data.add(i * cols + j);
                }
            }
        } else {
            // Batched transpose
            let mut batch: usize = 1;
            for i in 0..ndim - 2 {
                batch *= *t.shape.add(i) as usize;
            }
            let rows = *t.shape.add(ndim - 2) as usize;
            let cols = *t.shape.add(ndim - 1) as usize;
            for b in 0..batch {
                for i in 0..rows {
                    for j in 0..cols {
                        *r.data.add(b * cols * rows + j * rows + i) =
                            *t.data.add(b * rows * cols + i * cols + j);
                    }
                }
            }
        }

        ptr
    }
}

// ===========================================================================
// Factory functions
// ===========================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_zeros(ndim: i64, shape: *const i64) -> *mut u8 {
    // tensor_create already zero-initializes
    unsafe { tensor_create(ndim, shape, 0) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_ones(ndim: i64, shape: *const i64) -> *mut u8 {
    unsafe {
        let ptr = tensor_create(ndim, shape, 0);
        let t = as_tensor(ptr);
        let n = t_numel(t);
        for i in 0..n {
            *t.data.add(i) = 1.0f32;
        }
        ptr
    }
}

/// Create a tensor filled with random values from a standard normal
/// distribution (Box-Muller transform).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_randn(ndim: i64, shape: *const i64) -> *mut u8 {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SEEDED: AtomicBool = AtomicBool::new(false);

    unsafe {
        if !SEEDED.swap(true, Ordering::Relaxed) {
            libc::srand(libc::time(std::ptr::null_mut()) as libc::c_uint);
        }

        let ptr = tensor_create(ndim, shape, 0);
        let t = as_tensor(ptr);
        let n = t_numel(t);
        let pi: f32 = std::f32::consts::PI;

        let mut i = 0usize;
        while i < n {
            let u1 = (libc::rand() as f32 + 1.0) / (libc::RAND_MAX as f32 + 1.0);
            let u2 = (libc::rand() as f32 + 1.0) / (libc::RAND_MAX as f32 + 1.0);
            let mag = (-2.0f32 * u1.ln()).sqrt();
            *t.data.add(i) = mag * (2.0 * pi * u2).cos();
            if i + 1 < n {
                *t.data.add(i + 1) = mag * (2.0 * pi * u2).sin();
            }
            i += 2;
        }

        ptr
    }
}

// ===========================================================================
// Activation functions
// ===========================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_relu(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        let ptr = tensor_like(t);
        let r = as_tensor(ptr);
        let n = t_numel(t);
        for i in 0..n {
            let v = *t.data.add(i);
            *r.data.add(i) = if v > 0.0 { v } else { 0.0 };
        }
        ptr
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_sigmoid(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        let ptr = tensor_like(t);
        let r = as_tensor(ptr);
        let n = t_numel(t);
        for i in 0..n {
            let v = *t.data.add(i);
            *r.data.add(i) = 1.0 / (1.0 + (-v).exp());
        }
        ptr
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_tanh_act(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        let ptr = tensor_like(t);
        let r = as_tensor(ptr);
        let n = t_numel(t);
        for i in 0..n {
            *r.data.add(i) = (*t.data.add(i)).tanh();
        }
        ptr
    }
}

/// Softmax over the last dimension.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tensor_softmax(t: *const u8) -> *mut u8 {
    unsafe {
        let t = as_tensor(t);
        let ptr = tensor_like(t);
        let r = as_tensor(ptr);
        let numel = t_numel(t);

        if t.ndim == 1 {
            // 1D: softmax over the entire vector
            let mut max_val = *t.data;
            for i in 1..numel {
                let v = *t.data.add(i);
                if v > max_val {
                    max_val = v;
                }
            }
            let mut sum: f32 = 0.0;
            for i in 0..numel {
                let e = (*t.data.add(i) - max_val).exp();
                *r.data.add(i) = e;
                sum += e;
            }
            for i in 0..numel {
                *r.data.add(i) /= sum;
            }
        } else if t.ndim == 2 {
            let rows = *t.shape as usize;
            let cols = *t.shape.add(1) as usize;
            // Softmax along last dim (each row independently)
            for i in 0..rows {
                let mut max_val = *t.data.add(i * cols);
                for j in 1..cols {
                    let v = *t.data.add(i * cols + j);
                    if v > max_val {
                        max_val = v;
                    }
                }
                let mut sum: f32 = 0.0;
                for j in 0..cols {
                    let e = (*t.data.add(i * cols + j) - max_val).exp();
                    *r.data.add(i * cols + j) = e;
                    sum += e;
                }
                for j in 0..cols {
                    *r.data.add(i * cols + j) /= sum;
                }
            }
        } else {
            // General nD: softmax over last dimension
            let inner = *t.shape.add(t.ndim as usize - 1) as usize;
            let outer = numel / inner;
            for o in 0..outer {
                let mut max_val = *t.data.add(o * inner);
                for i in 1..inner {
                    let v = *t.data.add(o * inner + i);
                    if v > max_val {
                        max_val = v;
                    }
                }
                let mut sum: f32 = 0.0;
                for i in 0..inner {
                    let e = (*t.data.add(o * inner + i) - max_val).exp();
                    *r.data.add(o * inner + i) = e;
                    sum += e;
                }
                for i in 0..inner {
                    *r.data.add(o * inner + i) /= sum;
                }
            }
        }

        ptr
    }
}
