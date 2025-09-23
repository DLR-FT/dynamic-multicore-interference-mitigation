use core::{arch::asm, fmt::Write};

use arm64::{Entry, EntryInfo, cache::*, critical_section, mmu::*};

use crate::{L0TABLE, systick::SysTick, uart::UartWriter};

pub struct IntruderEntryImpl;

impl Entry for IntruderEntryImpl {
    unsafe extern "C" fn entry(info: EntryInfo) -> ! {
        unsafe { intruder_main(info) }
    }
}

static FOO: [u8; 1024 * 1024] = [0; 1024 * 1024];

unsafe fn intruder_main(info: EntryInfo) -> ! {
    critical_section::with(|cs| {
        let l0 = L0TABLE.borrow_ref_mut(cs);

        MMU::enable_el2(l0.base_addr() as u64);

        ICache::enable();
        DCache::enable();
    });

    DCache::op_all(CacheOp::CleanInvalidate);

    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\n", info.cpu_idx))
        .unwrap();

    loop {
        let ptr = &FOO as *const _ as *mut [u8; FOO.len()];

        unsafe {
            asm!(
                "2:",                           // Start loop
                "cmp {i}, {end}",
                "b.hs 3f",                      // done
                "mrs {x}, CNTPCT_EL0",
                "str {x}, [{i}], 0x8",
                "b 2b",
                "3:",                           // end
                i = in(reg) ptr,
                end = in(reg) ptr.byte_offset(FOO.len() as isize),
                x = out(reg) _,
            )
        };

        SysTick::wait_us(1000 * 1000);
    }
}
