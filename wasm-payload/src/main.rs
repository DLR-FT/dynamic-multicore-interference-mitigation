#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;

mod allocator;
mod kernel;

use allocator::*;

pub const BUF_LEN: usize = 0x0010_0000;
pub static BUF: &[u8] = &[0u8; BUF_LEN];

#[unsafe(no_mangle)]
pub fn main() {
    unsafe { ALLOCATOR.lock().init(&BUF) };

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
