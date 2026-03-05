// ARM64 backend — complete port of QBE's arm64/{abi,isel,emit,targ}.c
//
// Sub-modules:
//   regs  — register constants, target definitions
//   abi   — ABI lowering (abi0 / abi1)
//   isel  — instruction selection
//   emit  — assembly emission

mod abi;
mod emit_asm;
mod isel_pass;
pub mod regs;

pub use abi::{abi0, abi1, argregs, retregs};
pub use emit_asm::emitfn;
pub use isel_pass::isel;
pub use regs::{T_ARM64, T_ARM64_APPLE};
