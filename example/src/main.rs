#![no_std]
#![no_main]

use core::fmt::Write as FmtWrite;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;

use arm64::cache::{DCache, ICache};
use arm64::smccc::*;
use arm64::{Entry, EntryInfo, critical_section, entry};
use arm64::{psci::*, start};

use embedded_alloc::LlffHeap as Heap;

mod excps;
mod plat;
mod systick;
mod uart;
mod wasm;

use excps::*;
use plat::*;
use uart::*;
use wasm::*;

use crate::systick::SysTick;

const HEAP_SIZE: usize = 1 * 1024 * 1024 * 1024; // 1GiB heap

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[entry(exceptions = Excps)]
unsafe fn main(info: EntryInfo) -> ! {
    // ICache::enable();
    // DCache::enable();

    {
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
    }

    critical_section::with(|cs| {
        UART_DRIVER.borrow_ref_mut(cs).init();
    });

    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\r\n", info.cpu_idx))
        .unwrap();

    Psci::cpu_on_64::<Smccc<SMC>>(
        1,
        (start::<IntruderEntryImpl, Excps> as *const fn() -> !) as u64,
        0,
    )
    .unwrap();

    Psci::cpu_on_64::<Smccc<SMC>>(
        2,
        (start::<IntruderEntryImpl, Excps> as *const fn() -> !) as u64,
        0,
    )
    .unwrap();

    Psci::cpu_on_64::<Smccc<SMC>>(
        3,
        (start::<IntruderEntryImpl, Excps> as *const fn() -> !) as u64,
        0,
    )
    .unwrap();

    SysTick::wait_us(1000000);

    UartWriter.write_str("running wasm ...").unwrap();

    const WASM_BYTES: &[u8] = include_bytes!("../2mm.wasm");
    run_wasm(WASM_BYTES).unwrap();

    UartWriter.write_str("done.").unwrap();

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}

struct IntruderEntryImpl;

impl Entry for IntruderEntryImpl {
    unsafe extern "C" fn entry(info: EntryInfo) -> ! {
        unsafe { intruder_main(info) }
    }
}

unsafe fn intruder_main(info: EntryInfo) -> ! {
    // ICache::enable();
    // DCache::enable();

    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\r\n", info.cpu_idx))
        .unwrap();

    unsafe {
        core::arch::asm!(
            "ldr x0, ={x}",
            // "mov x0, #0x"
            // "2: ldr x1, [x0]",
            // "str x1, [x0]",
            "2: nop",
            "nop",
            "b 2b",
            x = sym HEAP
        )
    }

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    UartWriter
        .write_fmt(format_args!("{}", _info.message()))
        .unwrap();

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}
