#![no_std]
#![no_main]
#![feature(once_cell_get_mut)]

extern crate alloc;

use core::{cell::OnceCell, mem::MaybeUninit, panic::PanicInfo};

use simple_alloc::SimpleAlloc;

use wasm_payload::kernel::Kernel2MM;

pub const ALLOC_BUF_LEN: usize = 0x0100_0000;
pub static ALLOC_BUF: &[MaybeUninit<u8>] = &[MaybeUninit::uninit(); ALLOC_BUF_LEN];

#[global_allocator]
pub static ALLOCATOR: SimpleAlloc = SimpleAlloc::new();
pub static mut ALLOC_INIT: bool = false;

pub static mut KERNEL: OnceCell<Kernel2MM> = OnceCell::new();

#[unsafe(no_mangle)]
pub fn main() {
    unsafe {
        if !ALLOC_INIT {
            ALLOCATOR.init(&ALLOC_BUF);
            ALLOC_INIT = true;
        }
    }

    #[allow(static_mut_refs)]
    let kernel = unsafe { KERNEL.get_mut_or_init(|| Kernel2MM::new()) };

    kernel.run();
}

unsafe extern "C" {
    pub fn host_panic();
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    unsafe { host_panic() };
    loop {}
}
