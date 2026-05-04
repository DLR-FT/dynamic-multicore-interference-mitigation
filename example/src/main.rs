#![no_std]
#![no_main]
#![feature(slice_from_ptr_range)]

extern crate alloc;

use core::cell::RefCell;
use core::mem::MaybeUninit;
use core::ops::BitOr;
use core::ops::Shl;
use core::panic::PanicInfo;
use core::ptr::addr_of;
use core::slice;
use core::sync::atomic::AtomicUsize;

use analyzer::PMUInfo;
use analyzer::RefuelUpdate;
use arm64::arbitrary_int::*;
use arm64::cache::*;
use arm64::mmu::*;
use arm64::pmu::CounterValue;
use arm64::pmu::PMU;
use arm64::psci::*;
use arm64::smccc::*;
use arm64::*;

use arm_gic::IntId;
use arm_gic::gicv2::SgiTarget;
use arm_gic::gicv2::SgiTargetListFilter;

use spin::Once;
use spin::mutex::SpinMutex;

use log::error;
use log::info;
use log::set_logger;
use log::set_max_level;

use simple_alloc::SimpleAlloc;

mod excps;
mod intruder;
mod logger;
mod native_runner;
mod plat;
mod spin_utils;
mod systick;
mod uart_ext;
mod wasm_runner;

use excps::*;
use intruder::*;
use logger::*;
use native_runner::*;
use plat::*;
use spin_utils::*;
use systick::*;

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

static INTRUDER_STATE: AtomicUsize = AtomicUsize::new(0);

const INTRUDER_STOP_INTR: IntId = IntId::sgi(3);

#[entry(exceptions = Excps)]
unsafe fn main(_info: EntryInfo) -> ! {
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

    info!("Hello World!");

    start_core::<IntruderEntryImpl>(1);
    start_core::<IntruderEntryImpl>(2);
    start_core::<IntruderEntryImpl>(3);

    SysTick::wait_us(1000000);

    const WASM_BYTES: &[u8] =
        include_bytes!("../../target/wasm32-unknown-unknown/release/wasm-payload.wasm");

    let mut runner = NativeRunner::new();
    // let mut runner = WasmRunner::new(WASM_BYTES, Some(u32::MAX));

    loop {
        unsafe extern "C" {
            static mut __heap_start: MaybeUninit<u8>;
            static mut __heap_end: MaybeUninit<u8>;
        }

        let heap_start = addr_of!(__heap_start);
        let heap_end = addr_of!(__heap_end);

        let heap_buf = unsafe { slice::from_ptr_range(heap_start..heap_end) };
        unsafe { ALLOCATOR.init(heap_buf) };

        let intruder_state = INTRUDER_STATE.load(core::sync::atomic::Ordering::Acquire);
        runner.run(intruder_state);

        let mut state = INTRUDER_STATE.load(core::sync::atomic::Ordering::Acquire);
        if state < 3 {
            state += 1;
        } else {
            state = 0;
        }

        enable_intruders(state);
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

fn enable_intruders(state: usize) {
    let last_state = INTRUDER_STATE.load(core::sync::atomic::Ordering::Acquire);

    // info!("intruder state: {} -> {}", last_state, state);

    let mut targets = 0;
    for x in state..last_state {
        targets |= 1 << (x + 1);
    }

    INTRUDER_STATE.store(state, core::sync::atomic::Ordering::Release);
    GIC_DRIVER.lock_irq(|gic| {
        let mut gic = gic.borrow_mut();

        gic.send_sgi(
            INTRUDER_STOP_INTR,
            SgiTarget::List {
                target_list_filter: SgiTargetListFilter::CPUTargetList,
                target_list: targets,
            },
        );
    });
}

trait CounterValueExt {
    type T;
    fn ok(self) -> Option<Self::T>;
    fn chain<U>(self, upper: Self) -> CounterValue<U>
    where
        Self::T: Into<U>,
        U: Shl<usize, Output = U>,
        U: BitOr<Output = U>;
}

impl<T> CounterValueExt for CounterValue<T> {
    type T = T;

    fn ok(self) -> Option<Self::T> {
        match self {
            CounterValue::Ok(x) => Some(x),
            CounterValue::Overflowed(_) => None,
        }
    }

    fn chain<U>(self, upper: Self) -> CounterValue<U>
    where
        T: Into<U>,
        U: Shl<usize, Output = U>,
        U: BitOr<Output = U>,
    {
        let upper = match upper {
            CounterValue::Overflowed(cnt) => return CounterValue::Overflowed(cnt.into()),
            CounterValue::Ok(cnt) => cnt.into(),
        };

        let lower = match self {
            CounterValue::Overflowed(cnt) => cnt.into(),
            CounterValue::Ok(cnt) => cnt.into(),
        };

        CounterValue::Ok((upper << 32) | lower)
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("PANIC: {}", info);

    loop {
        unsafe { core::arch::asm!("nop") };
    }
}
