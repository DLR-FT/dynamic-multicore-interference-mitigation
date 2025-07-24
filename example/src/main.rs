#![no_std]
#![no_main]

use core::fmt::Write as FmtWrite;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;

use arm64::cache::{DCache, ICache};
use arm64::psci::Psci;
use arm64::{Entry, EntryInfo, critical_section, entry};
use arm64::{smccc::*, start};

use embedded_alloc::LlffHeap as Heap;

mod excps;
mod plat;
mod uart;
mod wasm;

use excps::*;
use plat::*;
use uart::*;
use wasm::*;

const HEAP_SIZE: usize = 2 * 1024 * 1024 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[entry(exceptions = Excps)]
unsafe fn main(info: EntryInfo) -> ! {
    ICache::enable();
    DCache::enable();

    {
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
    }

    critical_section::with(|cs| {
        UART_DRIVER.borrow_ref_mut(cs).init();
    });

    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\n", info.cpu_idx))
        .unwrap();

    Psci::cpu_on_64::<Smccc<SMC>>(
        1,
        (start::<SecEntryImpl, Excps> as *const fn() -> !) as u64,
        0,
    )
    .unwrap();

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}

struct SecEntryImpl;

impl Entry for SecEntryImpl {
    unsafe extern "C" fn entry(info: EntryInfo) -> ! {
        unsafe { sec_main(info) }
    }
}

unsafe fn sec_main(info: EntryInfo) -> ! {
    const WASM_BYTES: &[u8] = include_bytes!("../3mm.wasm");

    ICache::enable();
    DCache::enable();

    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\n", info.cpu_idx))
        .unwrap();

    UartWriter.write_str("running wasm ...").unwrap();

    run_wasm(WASM_BYTES).unwrap();

    UartWriter.write_str("done.").unwrap();

    // Psci::cpu_off::<Smccc<SMC>>().unwrap();

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    UartWriter
        .write_fmt(format_args!("{}", _info.message()))
        .unwrap();
    loop {}
}
