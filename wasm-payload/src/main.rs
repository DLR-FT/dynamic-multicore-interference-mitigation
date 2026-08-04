#![no_std]
#![no_main]

extern crate alloc;

use core::{cell::LazyCell, mem::MaybeUninit, panic::PanicInfo};

use simple_alloc::SimpleAlloc;

use wasm_payload::kernel::Kernel2MM;

pub const BUF_LEN: usize = 0x0100_0000;
pub static BUF: &[MaybeUninit<u8>] = &[MaybeUninit::uninit(); BUF_LEN];

#[global_allocator]
pub static ALLOCATOR: SimpleAlloc = SimpleAlloc::new();

pub static mut KERNEL: LazyCell<Kernel2MM> = LazyCell::new(|| Kernel2MM::new());

// #[unsafe(no_mangle)]
// pub fn init() {
//     unsafe { ALLOCATOR.init(&BUF) };

//     let mut kernel = Kernel2MM::new();
// }

#[unsafe(no_mangle)]
pub fn main() {
    unsafe { KERNEL.run() };
}

unsafe extern "C" {
    pub fn host_panic();
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    unsafe { host_panic() };
    loop {}
}
