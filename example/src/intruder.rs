use core::arch::asm;
use core::ptr::write_volatile;
use core::sync::atomic::AtomicBool;
use core::usize;
use core::{cell::RefCell, mem::MaybeUninit};

use arm_gic::gicv2::{SgiTarget, SgiTargetListFilter};
use arm_gic::{IntId, InterruptGroup};

use arm64::{
    EntryInfo,
    arbitrary_int::{u2, u3},
    cache::*,
    mmu::*,
    secondary_entry,
};

use spin::mutex::SpinMutex;

use crate::Excps;

use crate::{DEVICE_ATTRS, NORMAL_ATTRS, plat::GIC_DRIVER, spin_utils::SpinMutexExt};

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

const CACHE_SIZE_BITS: usize = 20;
const CACHE_LINE_BITS: usize = 6;
const CACHE_WAYS_BITS: usize = 4;
const CACHE_SET_BITS: usize = CACHE_SIZE_BITS - (CACHE_LINE_BITS + CACHE_WAYS_BITS);
const CACHE_TAG_BITS: usize = usize::BITS as usize - (CACHE_SET_BITS + CACHE_LINE_BITS);

pub static mut SET_MASK: usize = 0x3FF;
static mut CACHE_BUF: [CacheBuf; 3] = [CacheBuf::uninit(); 3];

const INTRUDER_BREAK_INTR: IntId = IntId::sgi(3);
pub static INTRUDER_BREAK: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
#[repr(align(0x0010_0000))]
struct CacheBuf([MaybeUninit<u8>; 1 << CACHE_SIZE_BITS]);

impl CacheBuf {
    pub const fn uninit() -> Self {
        Self([MaybeUninit::uninit(); 1 << CACHE_SIZE_BITS])
    }
}

pub fn intruder_break() {
    INTRUDER_BREAK.store(true, core::sync::atomic::Ordering::Release);
    GIC_DRIVER.lock_irq(|gic| {
        let mut gic = gic.borrow_mut();

        gic.send_sgi(
            INTRUDER_BREAK_INTR,
            SgiTarget::List {
                target_list_filter: SgiTargetListFilter::CPUTargetList,
                target_list: 0b1110,
            },
        );
    });
}

pub fn intruder_cont() {
    INTRUDER_BREAK.store(false, core::sync::atomic::Ordering::Release);
}

#[secondary_entry(exceptions = Excps)]
fn intruder_main(info: EntryInfo) -> ! {
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

    const CACHE_SIZE_BITS: usize = 20;
    const CACHE_LINE_BITS: usize = 6;
    const CACHE_WAYS_BITS: usize = 4;
    const CACHE_SET_BITS: usize = CACHE_SIZE_BITS - (CACHE_LINE_BITS + CACHE_WAYS_BITS);
    const CACHE_TAG_BITS: usize = usize::BITS as usize - (CACHE_SET_BITS + CACHE_LINE_BITS);

    const TAG_MASK: usize = usize::MAX << (CACHE_SET_BITS + CACHE_LINE_BITS);

    unsafe {
        let buf = &mut CACHE_BUF[info.cpu_idx].0;

        let mut i = 0;
        loop {
            i = ((i + 1) * 1000003) % (1 << CACHE_SIZE_BITS);

            let mut addr = &mut buf[i] as *mut MaybeUninit<u8>;
            addr = addr.map_addr(|x| x & (TAG_MASK | (SET_MASK << CACHE_LINE_BITS)));

            let mut x: u64 = 0xDEADC0DE;
            asm!("mrs {x}, CNTPCT_EL0", x = lateout(reg) x);
            write_volatile(addr as *mut u8, x as u8);
        }
    }
}
