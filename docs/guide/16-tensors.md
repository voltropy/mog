# Chapter 16: Tensors — N-Dimensional Arrays

Mog provides tensors as a built-in data type — n-dimensional arrays with a fixed element dtype. They are the interchange format between Mog scripts and host-provided ML capabilities. You create tensors, read and write their elements, reshape them, and pass them to host functions. That's it.

Tensors in Mog are deliberately not a full tensor library. There is no built-in matmul, no convolution, no autograd. The language gives you the data container; the host gives you the compute. A script describes *what* data to prepare and *where* results go. The host decides *how* to execute the math — CPU, GPU, or remote accelerator.

## What Are Tensors?

A tensor is an n-dimensional array where every element has the same dtype. A 1D tensor is a vector. A 2D tensor is a matrix. A 3D tensor might be an image batch or a sequence of embeddings. The dimensionality is limited only by available memory.

Three properties define a tensor:

1. **Shape.** An array of integers describing the size of each dimension. A shape of `[3, 224, 224]` means 3 planes of 224×224 elements — 150,528 elements total.
2. **Dtype.** The element type. Mog supports `f16`, `bf16`, `f32`, and `f64`. The dtype is part of the tensor's type — `tensor<f32>` and `tensor<f16>` are different types.
3. **Data.** A flat, contiguous buffer of elements in row-major order.

Tensors are heap-allocated and garbage-collected like arrays and maps. They are passed by reference — assigning a tensor to a new variable does not copy the data.

```mog
a := tensor<f32>([3], [1.0, 2.0, 3.0]);
b := a;       // b and a point to the same data
b[0] = 99.0;  // a[0] is now 99.0 too
```

> **Note:** If you need an independent copy, use `.reshape()` with the same shape — it returns a new tensor with its own data buffer.

## Creating Tensors

### From a Literal

The most direct way to create a tensor is with an explicit shape and data array:

```mog
tensor<dtype>(shape, data)
```

The dtype must be specified — there is no inference. The shape is an array of integers. The data is a flat array of values in row-major order, and its length must equal the product of the shape dimensions.

```mog
// 1D tensor (vector) — 3 elements
v := tensor<f32>([3], [1.0, 2.0, 3.0]);

// 2D tensor (matrix) — 3 rows, 4 columns
matrix := tensor<f32>([3, 4], [
  1.0, 2.0,  3.0,  4.0,
  5.0, 6.0,  7.0,  8.0,
  9.0, 10.0, 11.0, 12.0,
]);

// 3D tensor — 2 matrices of 2x3
batch := tensor<f64>([2, 2, 3], [
  1.0, 2.0, 3.0,
  4.0, 5.0, 6.0,
  7.0, 8.0, 9.0,
  10.0, 11.0, 12.0,
]);

// Scalar (0D tensor) — shape is empty
scalar := tensor<f32>([], [42.0]);
```

If the data length doesn't match the shape, you get a runtime error:

```mog
// Runtime error: shape [2, 3] requires 6 elements, got 4
bad := tensor<f32>([2, 3], [1.0, 2.0, 3.0, 4.0]);
```

### Static Constructors

For common initialization patterns, tensor types provide static constructors:

```mog
// All zeros
zeros := tensor<f32>.zeros([3, 224, 224]);

// All ones
ones := tensor<f32>.ones([768]);

// Random normal distribution (mean 0, stddev 1)
random := tensor<f32>.randn([10, 10]);
```

These are the only built-in constructors. If you need other initialization patterns — linspace, arange, identity matrices — build them with a loop or request them from a host capability.

```mog
// Building an identity matrix manually
fn eye(n: int) -> tensor<f32> {
  t := tensor<f32>.zeros([n, n]);
  for i in 0..n {
    t[(i * n) + i] = 1.0;
  }
  return t;
}
```

### Supported Dtypes

| Dtype | Size | Description |
|---|---|---|
| `f16` | 2 bytes | IEEE 754 half-precision float |
| `bf16` | 2 bytes | Brain floating point (truncated f32 mantissa) |
| `f32` | 4 bytes | IEEE 754 single-precision float |
| `f64` | 8 bytes | IEEE 754 double-precision float |

The dtype is part of the type system. You cannot assign a `tensor<f32>` to a variable of type `tensor<f16>` — they are different types. There is no implicit conversion between tensor dtypes.

```mog
a := tensor<f32>.zeros([10]);
b := tensor<f16>.zeros([10]);
// a = b;  // Compile error: type mismatch
```

> **Tip:** Use `f32` as the default. Use `f16` or `bf16` when the host ML capability expects reduced-precision inputs — this is common for inference on GPUs. Use `f64` only when you need the extra precision, such as accumulating loss values over many steps.

## Tensor Properties

Every tensor exposes three read-only properties:

```mog
t := tensor<f32>([3, 224, 224], /* ... */);

t.shape   // [3, 224, 224] — array of ints
t.dtype   // "f32" — string
t.ndim    // 3 — int (same as t.shape.len)
```

These properties are useful for validating inputs, debugging shapes, and writing generic data-processing functions:

```mog
fn describe(t: tensor<f32>) {
  print("shape: {t.shape}, dtype: {t.dtype}, ndim: {t.ndim}");
  total := 1;
  for dim in t.shape {
    total = total * dim;
  }
  print("total elements: {total}");
}
```

| Property | Type | Description |
|---|---|---|
| `.shape` | `int[]` | Array of dimension sizes |
| `.dtype` | `string` | Element type name (`"f16"`, `"bf16"`, `"f32"`, `"f64"`) |
| `.ndim` | `int` | Number of dimensions (equal to `shape.len`) |

## Element Access

Tensor elements are accessed using flat indexing in row-major order. This is a single integer index into the underlying data buffer, not multi-dimensional indexing.

### Reading Elements

```mog
t := tensor<f32>([2, 3], [10.0, 20.0, 30.0, 40.0, 50.0, 60.0]);

// Flat index — row-major order
// Row 0: indices 0, 1, 2 → 10.0, 20.0, 30.0
// Row 1: indices 3, 4, 5 → 40.0, 50.0, 60.0
val := t[0];   // 10.0
val2 := t[4];  // 50.0
```

The returned value type depends on the tensor's dtype. For `tensor<f32>` and `tensor<f64>`, element access returns a `float`. For `tensor<f16>` and `tensor<bf16>`, the value is promoted to `float` on read.

### Writing Elements

```mog
t := tensor<f32>([3], [0.0, 0.0, 0.0]);
t[0] = 1.0;
t[1] = 2.0;
t[2] = 3.0;
// t is now [1.0, 2.0, 3.0]
```

Out-of-bounds access is a runtime error:

```mog
t := tensor<f32>([3], [1.0, 2.0, 3.0]);
// val := t[5];  // Runtime error: index 5 out of bounds for tensor with 3 elements
```

### Computing Flat Indices

For multi-dimensional tensors, you compute the flat index from coordinates manually. For a tensor with shape `[d0, d1, d2]`, the flat index of element `[i, j, k]` is `(i * d1 * d2) + (j * d2) + k`:

```mog
// 3x4 matrix
m := tensor<f32>([3, 4], [
  1.0, 2.0, 3.0, 4.0,
  5.0, 6.0, 7.0, 8.0,
  9.0, 10.0, 11.0, 12.0,
]);

// Element at row 1, column 2 → flat index = 1*4 + 2 = 6
val := m[(1 * 4) + 2];  // 7.0

// Helper function for 2D indexing
fn idx2d(shape: int[], row: int, col: int) -> int {
  return (row * shape[1]) + col;
}

val2 := m[idx2d(m.shape, 2, 3)];  // 12.0
```

## Shape Operations

Mog provides two built-in shape operations on tensors. Everything else — slicing, concatenation, padding — comes from host capabilities.

### Reshape

`.reshape(new_shape)` returns a new tensor with the same data but a different shape. The total number of elements must be the same:

```mog
t := tensor<f32>([2, 6], [
  1.0, 2.0, 3.0, 4.0, 5.0, 6.0,
  7.0, 8.0, 9.0, 10.0, 11.0, 12.0,
]);

// Reshape 2x6 → 3x4
reshaped := t.reshape([3, 4]);
print(reshaped.shape);  // [3, 4]

// Reshape to 1D
flat := t.reshape([12]);
print(flat.shape);  // [12]

// Reshape to higher dimensions
cube := t.reshape([2, 2, 3]);
print(cube.shape);  // [2, 2, 3]
```

Mismatched element counts cause a runtime error:

```mog
t := tensor<f32>([2, 3], [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
// bad := t.reshape([2, 4]);  // Runtime error: cannot reshape 6 elements into shape [2, 4] (8 elements)
```

### Transpose

`.transpose()` reverses the order of dimensions. For a 2D tensor (matrix), this swaps rows and columns:

```mog
m := tensor<f32>([2, 3], [
  1.0, 2.0, 3.0,
  4.0, 5.0, 6.0,
]);

mt := m.transpose();
print(mt.shape);  // [3, 2]
// mt data in row-major order: [1.0, 4.0, 2.0, 5.0, 3.0, 6.0]
```

For higher-dimensional tensors, `.transpose()` reverses all axes. A tensor with shape `[2, 3, 4]` becomes `[4, 3, 2]`:

```mog
t := tensor<f32>.zeros([2, 3, 4]);
tt := t.transpose();
print(tt.shape);  // [4, 3, 2]
```

For 1D tensors, transpose is a no-op — it returns a tensor with the same shape.

## Tensors and Host Capabilities

Tensors exist to be passed between Mog scripts and host ML capabilities. The language provides the data structure; the host provides the operations. This separation means the host can implement operations however it wants — BLAS on CPU, CUDA on GPU, or a remote inference service — without the script needing to change.

### Passing Tensors to Host Functions

A host capability that performs ML operations receives tensors as arguments and returns tensors as results:

```mog
requires ml;

async fn main() -> int {
  // Prepare input — e.g., a flattened 28x28 grayscale image
  input := tensor<f32>.randn([1, 784]);

  // Pass to host for inference
  output := await ml.forward(input)?;

  // Inspect the result
  print("output shape: {output.shape}");
  print("output dtype: {output.dtype}");

  return 0;
}
```

The `.mogdecl` for such a capability might look like:

```
capability ml {
  async fn forward(input: tensor<f32>) -> tensor<f32>
  async fn matmul(a: tensor<f32>, b: tensor<f32>) -> tensor<f32>
  async fn relu(input: tensor<f32>) -> tensor<f32>
  async fn softmax(input: tensor<f32>) -> tensor<f32>
  async fn loss_mse(predicted: tensor<f32>, target: tensor<f32>) -> tensor<f32>
}
```

### Composing Operations

Since host capability calls are regular function calls that take and return tensors, you compose them naturally:

```mog
requires ml;

async fn two_layer_forward(x: tensor<f32>, w1: tensor<f32>, w2: tensor<f32>) -> Result<tensor<f32>> {
  // Layer 1: matmul + relu
  h := await ml.matmul(x, w1)?;
  h = await ml.relu(h)?;

  // Layer 2: matmul + softmax
  out := await ml.matmul(h, w2)?;
  out = await ml.softmax(out)?;

  return ok(out);
}
```

The script reads like a neural network definition, but every heavy operation is executed by the host. The Mog script is orchestrating — not computing.

### Why This Design?

Three reasons:

1. **Performance.** Tensor math belongs on hardware accelerators. A Mog script running on CPU should not be doing matrix multiplication — the host routes these operations to the fastest available backend.
2. **Safety.** ML operations that allocate GPU memory, launch kernels, or talk to remote services are side effects. Keeping them in capabilities means the host controls resource usage.
3. **Portability.** The same Mog script runs unchanged whether the host uses CPU, CUDA, Metal, or a cloud inference API. The script never names a device.

## Practical Examples

### Creating a Data Batch

Preparing a batch of input tensors for inference — for example, 32 flattened 28×28 images:

```mog
requires fs;

async fn load_batch(paths: string[], batch_size: int) -> tensor<f32> {
  image_size := 784;  // 28 * 28
  batch := tensor<f32>.zeros([batch_size, image_size]);

  for i in 0..batch_size {
    data := await fs.read_file(paths[i])?;
    // Assume data is a raw float file — parse into tensor elements
    for j in 0..image_size {
      batch[(i * image_size) + j] = parse_float(data, j);
    }
  }

  return batch;
}
```

### Reading Model Output Elements

After inference, extracting results from the output tensor:

```mog
fn argmax(t: tensor<f32>) -> int {
  // Find the index of the largest element (1D tensor)
  best_idx := 0;
  best_val := t[0];

  total := t.shape[0];
  for i in 1..total {
    if t[i] > best_val {
      best_val = t[i];
      best_idx = i;
    }
  }

  return best_idx;
}

fn top_k(t: tensor<f32>, k: int) -> int[] {
  // Return indices of the k largest elements
  indices := []int{};
  used := map<int, bool>{};

  for round in 0..k {
    best_idx := -1;
    best_val := -1.0e30;
    total := t.shape[0];

    for i in 0..total {
      if !used[i] && (t[i] > best_val) {
        best_val = t[i];
        best_idx = i;
      }
    }

    indices = append(indices, best_idx);
    used[best_idx] = true;
  }

  return indices;
}
```

### Preparing Input Tensors

Normalizing raw data before passing to a model:

```mog
fn normalize(t: tensor<f32>) -> tensor<f32> {
  total := t.shape[0];

  // Compute mean
  sum := 0.0;
  for i in 0..total {
    sum = sum + t[i];
  }
  mean := sum / float(total);

  // Compute standard deviation
  sq_sum := 0.0;
  for i in 0..total {
    diff := t[i] - mean;
    sq_sum = sq_sum + (diff * diff);
  }
  std := sqrt(sq_sum / float(total));

  // Normalize
  result := tensor<f32>.zeros(t.shape);
  for i in 0..total {
    result[i] = (t[i] - mean) / std;
  }

  return result;
}
```

### End-to-End Inference Script

Putting it all together — load data, normalize, run inference, report results:

```mog
requires ml, fs;

async fn main() -> int {
  // Load raw input
  raw := tensor<f32>([1, 784], /* loaded from file */);

  // Normalize
  input := normalize(raw);
  print("input shape: {input.shape}, dtype: {input.dtype}");

  // Run inference
  logits := await ml.forward(input)?;
  probs := await ml.softmax(logits)?;

  // Find prediction
  prediction := argmax(probs);
  confidence := probs[prediction];

  print("predicted class: {prediction}");
  print("confidence: {confidence}");

  return 0;
}
```

## Summary

| Concept | Syntax / Example | Description |
|---|---|---|
| Create from literal | `tensor<f32>([2, 3], [1.0, ...])` | Shape + flat data in row-major order |
| Zeros | `tensor<f32>.zeros([3, 224, 224])` | All elements initialized to 0.0 |
| Ones | `tensor<f32>.ones([768])` | All elements initialized to 1.0 |
| Random normal | `tensor<f32>.randn([10, 10])` | Elements from N(0, 1) distribution |
| Shape | `t.shape` | `int[]` — dimension sizes |
| Dtype | `t.dtype` | `string` — `"f32"`, `"f16"`, etc. |
| Dimensions | `t.ndim` | `int` — number of dimensions |
| Read element | `t[i]` | Flat index, row-major order |
| Write element | `t[i] = 42.0` | Flat index assignment |
| Reshape | `t.reshape([3, 4])` | New tensor, same data, new shape |
| Transpose | `t.transpose()` | Reverses dimension order |
| Supported dtypes | `f16`, `bf16`, `f32`, `f64` | Dtype is part of the type — no implicit conversion |
| Host ML ops | `await ml.forward(t)?` | All compute goes through capabilities |

Tensors are data containers. They hold the numbers. The host provides the math. This separation keeps Mog scripts portable, safe, and small — a script that prepares tensors and calls host capabilities works the same whether the host runs on a laptop CPU or a cluster of GPUs.
