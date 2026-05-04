use core::{arch::asm, cell::RefCell, mem::MaybeUninit, ptr::write_volatile};

use arm_gic::{IntId, InterruptGroup};
use arm64::{
    Entry, EntryInfo,
    arbitrary_int::{u2, u3},
    cache::*,
    mmu::*,
    pmu::PMU,
};

use spin::mutex::SpinMutex;

use log::error;
use log::info;

use crate::{
    CounterValueExt, DEVICE_ATTRS, INTRUDER_STATE, NORMAL_ATTRS, plat::GIC_DRIVER,
    spin_utils::SpinMutexExt,
};

static CORE1_L0TABLE: SpinMutex<RefCell<TranslationTable<Level0>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

static CORE1_L1TABLE: SpinMutex<RefCell<TranslationTable<Level1>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

static CORE2_L0TABLE: SpinMutex<RefCell<TranslationTable<Level0>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

static CORE2_L1TABLE: SpinMutex<RefCell<TranslationTable<Level1>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

static CORE3_L0TABLE: SpinMutex<RefCell<TranslationTable<Level0>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

static CORE3_L1TABLE: SpinMutex<RefCell<TranslationTable<Level1>>> =
    SpinMutex::new(RefCell::new(TranslationTable::DEFAULT));

pub struct IntruderEntryImpl;

impl Entry for IntruderEntryImpl {
    unsafe extern "C" fn entry(info: EntryInfo) -> ! {
        unsafe {
            intruder_main(info);
            loop {}
        }
    }
}

const CACHE_LINE_LEN: usize = 64;
const CACHE_SIZE: usize = 1024 * 1024;
const CACHE_BUF_SIZE: usize = CACHE_SIZE;

static mut CACHE_BUF: [CacheBuf; 3] = [CacheBuf::uninit(); 3];

#[derive(Clone, Copy)]
#[repr(align(0x0010_0000))]
struct CacheBuf([MaybeUninit<u8>; CACHE_BUF_SIZE]);

impl CacheBuf {
    pub const fn uninit() -> Self {
        Self([MaybeUninit::uninit(); CACHE_BUF_SIZE])
    }
}

#[unsafe(no_mangle)]
unsafe fn intruder_main(info: EntryInfo) -> u8 {
    arm64::sys_regs::CPUACTLR_EL1.modify(|x| {
        x.with_L1RADIS(u2::new(0b11))
            .with_RADIS(u2::new(0b11))
            .with_DTAH(true)
            .with_L1PCTL(u3::new(0))
    });

    {
        let lock_l0 = match info.cpu_idx {
            1 => CORE1_L0TABLE.lock(),
            2 => CORE2_L0TABLE.lock(),
            3 => CORE3_L0TABLE.lock(),
            _ => panic!("sdghsdgsdhsdh"),
        };
        let mut l0 = lock_l0.borrow_mut();

        let lock_l1 = match info.cpu_idx {
            1 => CORE1_L1TABLE.lock(),
            2 => CORE2_L1TABLE.lock(),
            3 => CORE3_L1TABLE.lock(),
            _ => panic!("sdghsdgsdhsdh"),
        };
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
                l1.map_block(0xF000_0000, 0xF000_0000, DEVICE_ATTRS);
            }

            #[cfg(feature = "kr260")]
            () => {
                l0.map_table(0x0000_0000, l1.base_addr() as u64, TableAttrs::DEFAULT);
                l1.map_block(0x0000_0000, 0x0000_0000, NORMAL_ATTRS);
                l1.map_block(0x4000_0000, 0x4000_0000, NORMAL_ATTRS);
                l1.map_block(0xC000_0000, 0xC000_0000, DEVICE_ATTRS);
                l1.map_block(0xF000_0000, 0xF000_0000, DEVICE_ATTRS);
            }
        }

        MMU::enable_el2(l0.base_addr() as u64);

        ICache::enable();
        DCache::enable();
    }

    DCache::op_all(CacheOp::CleanInvalidate);

    let sgi_intid = IntId::sgi(3);
    GIC_DRIVER.lock_irq(|lock| {
        let mut gic = lock.borrow_mut();

        gic.setup();
        gic.set_priority_mask(0xff);
        gic.enable_group0(true);

        gic.set_group(sgi_intid, InterruptGroup::Group0);
        gic.set_interrupt_priority(sgi_intid, 0);
        gic.enable_interrupt(sgi_intid, true).unwrap();
        gic.set_trigger(sgi_intid, arm_gic::Trigger::Edge);
    });

    arm_gic::irq_enable();

    loop {
        let state = INTRUDER_STATE.load(core::sync::atomic::Ordering::Acquire);
        if state > info.cpu_idx {
            break;
        }
    }

    loop {
        let buf = unsafe { &mut CACHE_BUF[info.cpu_idx - 1].0 };

        const N: usize = 16;
        const STRIDE: usize = CACHE_BUF_SIZE / N;

        for i in (0..STRIDE).step_by(CACHE_LINE_LEN) {
            for j in (0..CACHE_BUF_SIZE).step_by(STRIDE) {
                let x: u64;
                unsafe { asm!("mrs {x}, CNTPCT_EL0", x = lateout(reg) x) }
                unsafe { write_volatile(&mut buf[i + j] as *const _ as *mut u8, x as u8) }
            }
        }
    }
}
