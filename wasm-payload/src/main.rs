#![no_std]
#![no_main]

extern crate alloc;

use core::{mem::MaybeUninit, panic::PanicInfo};

use simple_alloc::SimpleAlloc;

mod kernel;

pub const BUF_LEN: usize = 0x0010_0000;
pub static BUF: &[MaybeUninit<u8>] = &[MaybeUninit::uninit(); BUF_LEN];

#[global_allocator]
pub static ALLOCATOR: SimpleAlloc = SimpleAlloc::new();

#[unsafe(no_mangle)]
pub fn main() {
    unsafe { ALLOCATOR.init(&BUF) };

    kernel::run::<64, 64, 64, 64>();
}

unsafe extern "C" {
    pub fn host_panic();
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    unsafe { host_panic() };
    loop {}
}
