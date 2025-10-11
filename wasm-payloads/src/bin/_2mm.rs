#![no_std]

use alloc_cat::{ALLOCATOR, AllocCat};

#[global_allocator]
pub static GLOBAL_ALLOCATOR: &AllocCat = &ALLOCATOR;

use polybench_rs::linear_algebra::kernels::_2mm::bench;

fn main() {
    let _ = bench::<64, 64, 64, 64>(); // 64 * 64 * 8 = 32KiB (*4) ~10M fuels
}
