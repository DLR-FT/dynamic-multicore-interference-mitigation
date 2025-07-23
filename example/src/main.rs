#![no_std]
#![no_main]

use core::fmt::Write as FmtWrite;
use core::panic::PanicInfo;

use arm64::cache::{DCache, ICache};
use arm64::psci::Psci;
use arm64::{Entry, EntryInfo, critical_section, entry};
use arm64::{smccc::*, start};

mod excps;
mod plat;
mod uart;

use excps::*;
use plat::*;
use uart::*;

#[entry(exceptions = Excps)]
unsafe fn main(info: EntryInfo) -> ! {
    ICache::enable();
    DCache::enable();

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
    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\n", info.cpu_idx))
        .unwrap();

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    Psci::system_off::<Smccc<SMC>>().unwrap();
    loop {}
}
