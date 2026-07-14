#![no_std]
#![no_main]
#![feature(slice_from_ptr_range)]

extern crate alloc;

use core::{cell::RefCell, mem::MaybeUninit, panic::PanicInfo, ptr::addr_of, slice};

use arm64::{
    Entry, EntryInfo,
    arbitrary_int::*,
    cache::{CacheOp, DCache, ICache},
    entry,
    mmu::{
        Access, BlockAttrs, Level0, Level1, MMU, MemoryTyp, SecurityDomain, Shareability,
        TableAttrs, TranslationTable,
    },
    psci::{NodeHwState, Psci},
    smccc::{SMC, Smccc},
    start,
};

use spin::{Once, mutex::SpinMutex};

use log::{error, info, set_logger, set_max_level};

use simple_alloc::SimpleAlloc;

mod excps;
mod intruder;
mod logger;
mod native_runner;
mod perf;
mod plat;
mod spin_utils;
mod stm;
mod systick;
mod uart;
mod wasm_runner;

use excps::*;
use intruder::*;
use logger::*;
use native_runner::*;
use perf::*;
use plat::*;
use spin_utils::*;
use stm::*;
use systick::*;

use crate::wasm_runner::WasmRunner;

#[global_allocator]
pub static ALLOCATOR: SimpleAlloc = SimpleAlloc::new();

pub static LOGGER: Once<Logger<'static, plat::uart::Driver>> = Once::new();

static CORE0_L0TABLE: SpinMutex<RefCell<TranslationTable<Level0>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

static CORE0_L1TABLE: SpinMutex<RefCell<TranslationTable<Level1>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

const DEVICE_ATTRS: BlockAttrs = BlockAttrs::DEFAULT
    .with_mem_type(MemoryTyp::Device_nGnRnE)
    .with_shareability(Shareability::Non)
    .with_access(Access::PrivReadWrite)
    .with_security(SecurityDomain::NonSecure);

const NORMAL_ATTRS: BlockAttrs = BlockAttrs::DEFAULT
    .with_mem_type(MemoryTyp::Normal_Cacheable)
    .with_shareability(Shareability::Inner)
    .with_access(Access::PrivReadWrite)
    .with_security(SecurityDomain::NonSecure);

#[entry(exceptions = Excps)]
fn main(_info: EntryInfo) -> ! {
    arm64::sys_regs::CPUACTLR_EL1.modify(|x| {
        x.with_L1RADIS(u2::new(0b11))
            .with_RADIS(u2::new(0b11))
            .with_DTAH(true)
            .with_L1PCTL(u3::new(0))
    });

    {
        let lock_l0 = CORE0_L0TABLE.lock();
        let mut l0 = lock_l0.borrow_mut();

        let lock_l1 = CORE0_L1TABLE.lock();
        let mut l1 = lock_l1.borrow_mut();

        match () {
            #[cfg(feature = "qemu")]
            () => {
                use arm64::mmu::TableAttrs;

                l0.map_table(0x0000_0000, l1.base_addr() as u64, TableAttrs::DEFAULT);
                l1.map_block(0x0000_0000, 0x0000_0000, DEVICE_ATTRS);
                l1.map_block(0x4000_0000, 0x4000_0000, NORMAL_ATTRS);
                l1.map_block(0x8000_0000, 0x8000_0000, NORMAL_ATTRS);
                l1.map_block(0xC000_0000, 0xC000_0000, NORMAL_ATTRS);
            }

            #[cfg(feature = "tebf0818")]
            () => {
                l0.map_table(0x0000_0000, l1.base_addr() as u64, TableAttrs::DEFAULT);
                l1.map_block(0x0000_0000, 0x0000_0000, NORMAL_ATTRS);
                l1.map_block(0x4000_0000, 0x4000_0000, NORMAL_ATTRS);
                l1.map_block(0xC000_0000, 0xC000_0000, DEVICE_ATTRS);
            }

            #[cfg(feature = "kr260")]
            () => {
                use arm64::mmu::TableAttrs;

                l0.map_table(0x0000_0000, l1.base_addr() as u64, TableAttrs::DEFAULT);
                l1.map_block(0x0000_0000, 0x0000_0000, NORMAL_ATTRS);
                l1.map_block(0x4000_0000, 0x4000_0000, NORMAL_ATTRS);
                l1.map_block(0xC000_0000, 0xC000_0000, DEVICE_ATTRS);
            }
        }

        MMU::enable_el2(l0.base_addr() as u64);

        ICache::enable();
        ICache::enable();
    }

    DCache::op_all(CacheOp::CleanInvalidate);

    GIC_DRIVER.lock_irq(|gic| {
        let mut gic = gic.borrow_mut();

        gic.setup();
        gic.set_priority_mask(0xff);
        gic.enable_group0(true);
    });

    UART_DRIVER.lock_irq(|uart| uart.borrow_mut().init());

    let logger = LOGGER.call_once(|| Logger::new(&UART_DRIVER));
    set_logger(logger).unwrap();
    set_max_level(log::LevelFilter::Info);

    let mut stm_writer = StmWriter::new(0, &STM_DRIVER);

    info!("Hello World!");

    start_core::<SecondaryEntryImpl>(1);
    // start_core::<SecondaryEntryImpl>(2);
    // start_core::<SecondaryEntryImpl>(3);

    SysTick::wait_us(1000000);

    const WASM_BYTES: &[u8] =
        include_bytes!("../../target/wasm32-unknown-unknown/release/wasm-payload.wasm");

    // let mut runner = NativeRunner::new();
    let mut runner = WasmRunner::new(WASM_BYTES, Some(u32::MAX));

    loop {
        unsafe extern "C" {
            static mut __heap_start: MaybeUninit<u8>;
            static mut __heap_end: MaybeUninit<u8>;
        }

        let heap_start = addr_of!(__heap_start);
        let heap_end = addr_of!(__heap_end);

        let heap_buf = unsafe { slice::from_ptr_range(heap_start..heap_end) };
        unsafe { ALLOCATOR.init(heap_buf) };

        runner.run(&mut stm_writer);

        unsafe {
            intruder::SET_MASK = if intruder::SET_MASK == 0x3FF {
                0x3FE
            } else if intruder::SET_MASK == 0x3FE {
                0x3FC
            } else if intruder::SET_MASK == 0x3FC {
                0x3F8
            } else if intruder::SET_MASK == 0x3F8 {
                0x3F0
            } else {
                0x3FF
            }
        };
    }
}

fn start_core<E: Entry>(core_id: u64) {
    Psci::cpu_on_64::<Smccc<SMC>>(core_id, (start::<E, Excps> as *const fn() -> !) as u64, 0)
        .unwrap();

    loop {
        let Ok(state) = Psci::node_hw_state_64::<Smccc<SMC>>(core_id, 0) else {
            break;
        };

        match state {
            NodeHwState::HwOn => break,
            _ => SysTick::wait_us(10000),
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {}", info);

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}
