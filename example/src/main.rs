#![no_std]
#![no_main]
#![feature(slice_from_ptr_range)]

use core::arch::asm;
use core::cell::RefCell;
use core::fmt::Write as FmtWrite;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::ptr::addr_of;
use core::slice;

extern crate alloc;

use arm64::arbitrary_int::*;
use arm64::cache::*;
use arm64::mmu::*;
use arm64::psci::*;
use arm64::smccc::*;
use arm64::*;

use simple_alloc::SimpleAlloc;
use spin::mutex::SpinMutex;

mod excps;
mod intruder;
mod plat;
mod systick;
mod uart;
mod wasm;

use excps::*;
use plat::*;
use uart::*;

use crate::intruder::IntruderEntryImpl;
use crate::systick::SysTick;
use crate::wasm::run_wasm;

#[global_allocator]
pub static ALLOCATOR: SimpleAlloc = SimpleAlloc::new();

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
unsafe fn main(info: EntryInfo) -> ! {
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
                l0.map_table(0x0000_0000, l1.base_addr() as u64, TableAttrs::DEFAULT);
                l1.map_block(0x0000_0000, 0x0000_0000, NORMAL_ATTRS);
                l1.map_block(0x4000_0000, 0x4000_0000, NORMAL_ATTRS);
                l1.map_block(0xC000_0000, 0xC000_0000, DEVICE_ATTRS);
            }
        }

        MMU::enable_el2(l0.base_addr() as u64);

        ICache::enable();
        DCache::enable();
    }

    DCache::op_all(CacheOp::CleanInvalidate);

    UART_DRIVER.lock().borrow_mut().init();

    write!(UartWriter, "Hello World\n").unwrap();

    write!(UartWriter, "cpu_idx = {} ...\n", info.cpu_idx).unwrap();

    start_core::<IntruderEntryImpl>(1);
    start_core::<IntruderEntryImpl>(2);
    start_core::<IntruderEntryImpl>(3);

    SysTick::wait_us(1000000);

    loop {
        unsafe extern "C" {
            static mut __heap_start: MaybeUninit<u8>;
            static mut __heap_end: MaybeUninit<u8>;
        }

        let heap_start = addr_of!(__heap_start);
        let heap_end = addr_of!(__heap_end);

        let heap_buf = unsafe { slice::from_ptr_range(heap_start..heap_end) };

        unsafe { ALLOCATOR.init(heap_buf) };

        const WASM_BYTES: &[u8] =
            include_bytes!("../../target/wasm32-unknown-unknown/release/wasm-payload.wasm");
        run_wasm(WASM_BYTES);
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
    let _x = write!(UartWriter, "PANIC: {}", info.message());

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}
