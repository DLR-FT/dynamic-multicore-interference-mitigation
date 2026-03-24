use core::{arch::asm, cell::RefCell, fmt::Write, mem::MaybeUninit};

use arm64::{
    Entry, EntryInfo,
    arbitrary_int::{u2, u3},
    cache::*,
    mmu::*,
    pmu::PMU,
};

use spin::mutex::SpinMutex;

use crate::uart::UartWriter;
use crate::{DEVICE_ATTRS, NORMAL_ATTRS};

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

const FOO_LEN: usize = 1024 * 1024;
static mut FOO: [CacheBuf; 3] = [CacheBuf::uninit(); 3];

#[derive(Clone, Copy)]
#[repr(align(0x0010_0000))]
struct CacheBuf([MaybeUninit<u8>; FOO_LEN]);

impl CacheBuf {
    pub const fn uninit() -> Self {
        Self([MaybeUninit::uninit(); FOO_LEN])
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

    PMU::enable();

    PMU::setup_counter(0, arm64::pmu::Event::L1D_CACHE);
    PMU::setup_counter(1, arm64::pmu::Event::L1D_CACHE_WB);
    PMU::setup_counter(2, arm64::pmu::Event::L1D_CACHE_REFILL);

    PMU::setup_counter(3, arm64::pmu::Event::L2D_CACHE);
    PMU::setup_counter(4, arm64::pmu::Event::L2D_CACHE_WB);
    PMU::setup_counter(5, arm64::pmu::Event::L2D_CACHE_REFILL);

    loop {
        let mut ptr =
            unsafe { &FOO[info.cpu_idx - 1].0 as *const _ as *mut [MaybeUninit<u8>; FOO_LEN] };
        let mut end = unsafe { ptr.byte_offset(FOO_LEN as isize) };

        PMU::reset();
        PMU::start();

        unsafe {
            asm!(
                "2:",                           // Start loop
                "cmp {i}, {end}",
                "b.hs 3f",                      // done
                "mrs {x:x}, CNTPCT_EL0",
                "str {x:x}, [{i}], #0x40",
                "b 2b",
                "3:",                           // end
                "nop",
                i = inout(reg) ptr,
                end = inout(reg) end,
                x = out(reg) _,
            )
        };

        PMU::stop();

        let l1d = PMU::get_counter(0);
        let l1d_wb = PMU::get_counter(1);
        let l1d_refill = PMU::get_counter(2);

        let l2d = PMU::get_counter(3);
        let l2d_wb = PMU::get_counter(4);
        let l2d_refill = PMU::get_counter(5);

        // if info.cpu_idx == 1 {
        //     write!(
        //     UartWriter,
        //     "cpu_idx = {},  l1d = {:?}, l1d_wb = {:?}, l1d_refill = {:?}, l2d = {:?}, l2d_wb = {:?}, l2d_refill = {:?}\n",
        //     info.cpu_idx, l1d, l1d_wb, l1d_refill, l2d, l2d_wb, l2d_refill
        // )
        // .unwrap();
        // }
    }
}
