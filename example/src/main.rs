#![no_std]
#![no_main]

use core::arch::asm;
use core::cell::RefCell;
use core::fmt::Write as FmtWrite;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;

use arm64::cache::*;
use arm64::critical_section::*;
use arm64::mmu::*;
use arm64::psci::*;
use arm64::smccc::*;
use arm64::*;

use embedded_alloc::LlffHeap as Heap;

mod excps;
mod plat;
mod systick;
mod uart;
mod wasm;

use excps::*;
use plat::*;
use uart::*;

use systick::SysTick;
use wasm::run_wasm;

const HEAP_SIZE: usize = 1 * 1024 * 1024 * 1024;

#[global_allocator]
static HEAP: Heap = Heap::empty();

static L0TABLE: Mutex<RefCell<TranslationTable<Level0>>> =
    Mutex::new(RefCell::new(TranslationTable::DEFAULT));

static L1TABLE: Mutex<RefCell<TranslationTable<Level1>>> =
    Mutex::new(RefCell::new(TranslationTable::DEFAULT));

const DEVICE_ATTRS: BlockAttrs = BlockAttrs::DEFAULT
    .with_mem_type(MemoryTyp::Device_nGnRnE)
    .with_shareability(Shareability::Non)
    .with_access(Access::PrivReadWrite)
    .with_security(SecurityDomain::NonSecure);

const NORMAL_ATTRS: BlockAttrs = BlockAttrs::DEFAULT
    .with_mem_type(MemoryTyp::Normal_Cacheable)
    .with_shareability(Shareability::Outer)
    .with_access(Access::PrivReadWrite)
    .with_security(SecurityDomain::NonSecure);

#[entry(exceptions = Excps)]
unsafe fn main(info: EntryInfo) -> ! {
    critical_section::with(|cs| {
        let mut l0 = L0TABLE.borrow_ref_mut(cs);
        let mut l1 = L1TABLE.borrow_ref_mut(cs);

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
        }

        MMU::enable_el2(l0.base_addr() as u64);

        ICache::enable();
        DCache::enable();
    });

    DCache::op_all(CacheOp::CleanInvalidate);

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

    // Psci::cpu_on_64::<Smccc<SMC>>(
    //     2,
    //     (start::<IntruderEntryImpl, Excps> as *const fn() -> !) as u64,
    //     0,
    // )
    // .unwrap();

    // Psci::cpu_on_64::<Smccc<SMC>>(
    //     3,
    //     (start::<IntruderEntryImpl, Excps> as *const fn() -> !) as u64,
    //     0,
    // )
    // .unwrap();

    SysTick::wait_us(1000 * 1000);

    UartWriter.write_str("running wasm ...").unwrap();

    const WASM_BYTES: &[u8] = include_bytes!("../2mm.wasm");
    run_wasm(WASM_BYTES).unwrap();

    UartWriter.write_str("done.").unwrap();

    loop {
        unsafe {
            // DCache::op_all(CacheOp::CleanInvalidate);
            asm!("2:", "wfe", "b 2b",)
        };
    }
}

struct IntruderEntryImpl;

impl Entry for IntruderEntryImpl {
    unsafe extern "C" fn entry(info: EntryInfo) -> ! {
        unsafe { intruder_main(info) }
    }
}

static FOO: [u8; 64 * 1024 * 16] = [0; 64 * 1024 * 16];

unsafe fn intruder_main(info: EntryInfo) -> ! {
    critical_section::with(|cs| {
        let l0 = L0TABLE.borrow_ref_mut(cs);

        MMU::enable_el2(l0.base_addr() as u64);

        ICache::enable();
        DCache::enable();
    });

    DCache::op_all(CacheOp::CleanInvalidate);

    UartWriter
        .write_fmt(format_args!("Hello World! cpu_idx = {}\r\n", info.cpu_idx))
        .unwrap();

    loop {
        unsafe {
            asm!(
                "ldr x9, ={foo}",
                "mov x10, #{len}",
                "add x10, x9, 10",
                "2:", // Start loop
                "cmp x9, x10",
                "b.hs 3f", // done
                "mrs x11, CNTPCT_EL0",
                "str x11, [x9], 0x8",
                "b 2b",
                "3:", // end
                foo = sym FOO,
                len = const FOO.len(),
            )
        };
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
