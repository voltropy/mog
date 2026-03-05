// Mog language runtime — Rust implementation.
//
// This is a staticlib that compiled Mog programs link against.  Every public
// function is `extern "C"` + `#[no_mangle]` so the QBE-generated assembly can
// call it directly.

mod gc;
mod string_ops;
mod array;
mod map;
mod print;
mod math;
mod tensor;
mod convert;
mod result_opt;
mod vm;
mod async_rt;
mod posix;
mod plugin;
