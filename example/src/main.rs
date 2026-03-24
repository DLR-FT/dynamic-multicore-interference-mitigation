#![no_std]
#![no_main]
#![feature(slice_from_ptr_range)]

use core::cell::RefCell;
use core::fmt::Write as FmtWrite;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::ptr::addr_of;
use core::slice;
use core::u32;

extern crate alloc;

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
use crate::wasm::WasmRunner;

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

    start_core::<IntruderEntryImpl>(1);
    start_core::<IntruderEntryImpl>(2);
    start_core::<IntruderEntryImpl>(3);

    SysTick::wait_us(1000000);

    // const WASM_BYTES: &[u8] =
    //     include_bytes!("../../target/wasm32-unknown-unknown/release/wasm-payload.wasm");

    // let mut wasm_runner = WasmRunner::new(WASM_BYTES, Some(u32::MAX));

    PMU::enable();

    PMU::setup_counter(0, pmu::Event::L1D_CACHE);
    PMU::setup_counter(1, pmu::Event::L1D_CACHE_WB);
    PMU::setup_counter(2, pmu::Event::L1D_CACHE_REFILL);

    PMU::setup_counter(3, pmu::Event::L2D_CACHE);
    PMU::setup_counter(4, pmu::Event::L2D_CACHE_WB);
    PMU::setup_counter(5, pmu::Event::L2D_CACHE_REFILL);

    let mut run_idx = 0;

    loop {
        unsafe extern "C" {
            static mut __heap_start: MaybeUninit<u8>;
            static mut __heap_end: MaybeUninit<u8>;
        }

        let heap_start = addr_of!(__heap_start);
        let heap_end = addr_of!(__heap_end);

        let heap_buf = unsafe { slice::from_ptr_range(heap_start..heap_end) };
        unsafe { ALLOCATOR.init(heap_buf) };

        // wasm_runner.run(0);

        PMU::reset();
        PMU::start();
        let last = SysTick::get_time_us();

        wasm_payload::kernel::run::<128, 128, 128, 128>();

        let current = SysTick::get_time_us();
        PMU::stop();

        let dt = current - last;

        let pmu_info = PMUInfo {
            l1d_access: PMU::get_counter(0).ok(),
            l1d_wb: PMU::get_counter(1).ok(),
            l1d_refill: PMU::get_counter(2).ok(),

            l2d_access: PMU::get_counter(3).ok(),
            l2d_wb: PMU::get_counter(4).ok(),
            l2d_refill: PMU::get_counter(5).ok(),
        };

        let update = RefuelUpdate {
            timestamp: current,
            fuel: None,
            run_idx,
            refuel_idx: 0,
            intruder_state: 0,
            dt,
            df: None,
            acc_t: dt,
            acc_f: None,
            pmu_info: Some(pmu_info),
        };

        let buf = &mut [0u8; 1024];
        let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
        buf[n] = '\n' as u8;
        UartWriter::write_bytes(&buf[..n + 1]).unwrap();

        run_idx += 1;
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

trait CounterValueExt {
    type T;
    fn ok(self) -> Option<Self::T>;
}

impl<T> CounterValueExt for CounterValue<T> {
    type T = T;

    fn ok(self) -> Option<Self::T> {
        match self {
            CounterValue::Ok(x) => Some(x),
            CounterValue::Overflowed(_) => None,
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

// {"timestamp":128500696,"fuel":4294967295,"run_idx":0,"refuel_idx":0,"intruder_state":0,"dt":45849435,"df":503997900,"acc_t":45849435,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":35255318,"l1d_refill":35255318,"l2d_access":70882775,"l2d_wb":52340,"l2d_refill":86891}}
// {"timestamp":174780242,"fuel":4294967295,"run_idx":1,"refuel_idx":0,"intruder_state":0,"dt":45849726,"df":503997900,"acc_t":45849726,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":35248407,"l1d_refill":35248407,"l2d_access":70908073,"l2d_wb":52480,"l2d_refill":86712}}
// {"timestamp":221057566,"fuel":4294967295,"run_idx":2,"refuel_idx":0,"intruder_state":0,"dt":45847636,"df":503997900,"acc_t":45847636,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":35235837,"l1d_refill":35235837,"l2d_access":70876138,"l2d_wb":52777,"l2d_refill":86851}}
// {"timestamp":267336902,"fuel":4294967295,"run_idx":3,"refuel_idx":0,"intruder_state":0,"dt":45849616,"df":503997900,"acc_t":45849616,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":35266809,"l1d_refill":35266809,"l2d_access":70901393,"l2d_wb":52747,"l2d_refill":86803}}
// {"timestamp":313616068,"fuel":4294967295,"run_idx":4,"refuel_idx":0,"intruder_state":0,"dt":45849498,"df":503997900,"acc_t":45849498,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":35245283,"l1d_refill":35245283,"l2d_access":70858643,"l2d_wb":53171,"l2d_refill":87046}}

// {"timestamp":158741374,"fuel":4294967295,"run_idx":0,"refuel_idx":0,"intruder_state":0,"dt":53264828,"df":503997900,"acc_t":53264828,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":34847581,"l1d_refill":34847581,"l2d_access":69695947,"l2d_wb":23130525,"l2d_refill":25851472}}
// {"timestamp":212898625,"fuel":4294967295,"run_idx":1,"refuel_idx":0,"intruder_state":0,"dt":53468569,"df":503997900,"acc_t":53468569,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":34844237,"l1d_refill":34844237,"l2d_access":84980968,"l2d_wb":23297014,"l2d_refill":26148936}}
// {"timestamp":266827793,"fuel":4294967295,"run_idx":2,"refuel_idx":0,"intruder_state":0,"dt":53265403,"df":503997900,"acc_t":53265403,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":34856928,"l1d_refill":34856928,"l2d_access":69714521,"l2d_wb":23475223,"l2d_refill":26461863}}
// {"timestamp":320977010,"fuel":4294967295,"run_idx":3,"refuel_idx":0,"intruder_state":0,"dt":53480443,"df":503997900,"acc_t":53480443,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":34854617,"l1d_refill":34854617,"l2d_access":86569853,"l2d_wb":23390317,"l2d_refill":26317122}}
// {"timestamp":374945830,"fuel":4294967295,"run_idx":4,"refuel_idx":0,"intruder_state":0,"dt":53280820,"df":503997900,"acc_t":53280820,"acc_f":503997900,"pmu_info":{"l1d_access":null,"l1d_wb":34850977,"l1d_refill":34850977,"l2d_access":69702607,"l2d_wb":23407706,"l2d_refill":26375214}}

// ................

// {"timestamp":77096293,"fuel":4294967295,"run_idx":0,"refuel_idx":0,"intruder_state":0,"dt":5986470,"df":66237388,"acc_t":5986470,"acc_f":66237388,"pmu_info":{"l1d_access":3286565291,"l1d_wb":4350082,"l1d_refill":4350082,"l2d_access":8794958,"l2d_wb":5968,"l2d_refill":10996}}
// {"timestamp":83512215,"fuel":4294967295,"run_idx":1,"refuel_idx":0,"intruder_state":0,"dt":5986475,"df":66237388,"acc_t":5986475,"acc_f":66237388,"pmu_info":{"l1d_access":3286565308,"l1d_wb":4354363,"l1d_refill":4354363,"l2d_access":8821823,"l2d_wb":5689,"l2d_refill":10823}}
// {"timestamp":89928355,"fuel":4294967295,"run_idx":2,"refuel_idx":0,"intruder_state":0,"dt":5986527,"df":66237388,"acc_t":5986527,"acc_f":66237388,"pmu_info":{"l1d_access":3286565279,"l1d_wb":4352524,"l1d_refill":4352524,"l2d_access":8828175,"l2d_wb":5865,"l2d_refill":10943}}
// {"timestamp":96344452,"fuel":4294967295,"run_idx":3,"refuel_idx":0,"intruder_state":0,"dt":5986456,"df":66237388,"acc_t":5986456,"acc_f":66237388,"pmu_info":{"l1d_access":3286565286,"l1d_wb":4351211,"l1d_refill":4351211,"l2d_access":8815665,"l2d_wb":5858,"l2d_refill":10910}}
// {"timestamp":102760466,"fuel":4294967295,"run_idx":4,"refuel_idx":0,"intruder_state":0,"dt":5986480,"df":66237388,"acc_t":5986480,"acc_f":66237388,"pmu_info":{"l1d_access":3286565239,"l1d_wb":4355080,"l1d_refill":4355080,"l2d_access":8801965,"l2d_wb":5639,"l2d_refill":10822}}
// {"timestamp":109176593,"fuel":4294967295,"run_idx":5,"refuel_idx":0,"intruder_state":0,"dt":5986478,"df":66237388,"acc_t":5986478,"acc_f":66237388,"pmu_info":{"l1d_access":3286565267,"l1d_wb":4354297,"l1d_refill":4354297,"l2d_access":8821271,"l2d_wb":5803,"l2d_refill":10879}}

// {"timestamp":82383320,"fuel":4294967295,"run_idx":0,"refuel_idx":0,"intruder_state":0,"dt":6155337,"df":66237388,"acc_t":6155337,"acc_f":66237388,"pmu_info":{"l1d_access":3288274946,"l1d_wb":4369026,"l1d_refill":4369026,"l2d_access":8738785,"l2d_wb":3089196,"l2d_refill":506804}}
// {"timestamp":89207518,"fuel":4294967295,"run_idx":1,"refuel_idx":0,"intruder_state":0,"dt":6153908,"df":66237388,"acc_t":6153908,"acc_f":66237388,"pmu_info":{"l1d_access":3288274993,"l1d_wb":4364492,"l1d_refill":4364492,"l2d_access":8902302,"l2d_wb":3103345,"l2d_refill":507676}}
// {"timestamp":96017729,"fuel":4294967295,"run_idx":2,"refuel_idx":0,"intruder_state":0,"dt":6146501,"df":66237388,"acc_t":6146501,"acc_f":66237388,"pmu_info":{"l1d_access":3288274857,"l1d_wb":4369398,"l1d_refill":4369398,"l2d_access":8911904,"l2d_wb":3140182,"l2d_refill":509008}}
// {"timestamp":102853245,"fuel":4294967295,"run_idx":3,"refuel_idx":0,"intruder_state":0,"dt":6173580,"df":66237388,"acc_t":6173580,"acc_f":66237388,"pmu_info":{"l1d_access":3288274976,"l1d_wb":4361812,"l1d_refill":4361812,"l2d_access":8896929,"l2d_wb":3022492,"l2d_refill":504551}}
// {"timestamp":109710997,"fuel":4294967295,"run_idx":4,"refuel_idx":0,"intruder_state":0,"dt":6169243,"df":66237388,"acc_t":6169243,"acc_f":66237388,"pmu_info":{"l1d_access":3288274917,"l1d_wb":4362388,"l1d_refill":4362388,"l2d_access":8898169,"l2d_wb":3046028,"l2d_refill":505377}}
// {"timestamp":116560433,"fuel":4294967295,"run_idx":5,"refuel_idx":0,"intruder_state":0,"dt":6159292,"df":66237388,"acc_t":6159292,"acc_f":66237388,"pmu_info":{"l1d_access":3288274960,"l1d_wb":4364468,"l1d_refill":4364468,"l2d_access":8902433,"l2d_wb":3082713,"l2d_refill":506993}}
// {"timestamp":123398909,"fuel":4294967295,"run_idx":6,"refuel_idx":0,"intruder_state":0,"dt":6167038,"df":66237388,"acc_t":6167038,"acc_f":66237388,"pmu_info":{"l1d_access":3288274860,"l1d_wb":4363079,"l1d_refill":4363079,"l2d_access":8726899,"l2d_wb":3042436,"l2d_refill":504746}}

// .................

// {"timestamp":753417377,"fuel":null,"run_idx":187,"refuel_idx":0,"intruder_state":0,"dt":46381,"df":null,"acc_t":46381,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3728522,"l1d_refill":3728524,"l2d_access":7457048,"l2d_wb":0,"l2d_refill":0}}
// {"timestamp":753487022,"fuel":null,"run_idx":188,"refuel_idx":0,"intruder_state":0,"dt":47753,"df":null,"acc_t":47753,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3728051,"l1d_refill":3728053,"l2d_access":7456106,"l2d_wb":0,"l2d_refill":0}}
// {"timestamp":753556655,"fuel":null,"run_idx":189,"refuel_idx":0,"intruder_state":0,"dt":47742,"df":null,"acc_t":47742,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3726408,"l1d_refill":3726410,"l2d_access":7452820,"l2d_wb":0,"l2d_refill":0}}
// {"timestamp":753626319,"fuel":null,"run_idx":190,"refuel_idx":0,"intruder_state":0,"dt":47771,"df":null,"acc_t":47771,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3729624,"l1d_refill":3729626,"l2d_access":7459252,"l2d_wb":0,"l2d_refill":0}}
// {"timestamp":753694575,"fuel":null,"run_idx":191,"refuel_idx":0,"intruder_state":0,"dt":46365,"df":null,"acc_t":46365,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3726398,"l1d_refill":3726400,"l2d_access":7452800,"l2d_wb":0,"l2d_refill":0}}
// {"timestamp":753762825,"fuel":null,"run_idx":192,"refuel_idx":0,"intruder_state":0,"dt":46360,"df":null,"acc_t":46360,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3725776,"l1d_refill":3725778,"l2d_access":7451556,"l2d_wb":0,"l2d_refill":0}}
// {"timestamp":753831082,"fuel":null,"run_idx":193,"refuel_idx":0,"intruder_state":0,"dt":46370,"df":null,"acc_t":46370,"acc_f":null,"pmu_info":{"l1d_access":8962107,"l1d_wb":3727237,"l1d_refill":3727239,"l2d_access":7454478,"l2d_wb":0,"l2d_refill":0}}

// {"timestamp":39022144,"fuel":null,"run_idx":242,"refuel_idx":0,"intruder_state":0,"dt":57774,"df":null,"acc_t":57774,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3727899,"l1d_refill":3727900,"l2d_access":7455800,"l2d_wb":200854,"l2d_refill":19941}}
// {"timestamp":39100248,"fuel":null,"run_idx":243,"refuel_idx":0,"intruder_state":0,"dt":55504,"df":null,"acc_t":55504,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3727003,"l1d_refill":3727004,"l2d_access":7454008,"l2d_wb":199975,"l2d_refill":19984}}
// {"timestamp":39178925,"fuel":null,"run_idx":244,"refuel_idx":0,"intruder_state":0,"dt":56083,"df":null,"acc_t":56083,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3725820,"l1d_refill":3725821,"l2d_access":7451642,"l2d_wb":200040,"l2d_refill":20000}}
// {"timestamp":39258536,"fuel":null,"run_idx":245,"refuel_idx":0,"intruder_state":0,"dt":57016,"df":null,"acc_t":57016,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3727468,"l1d_refill":3727469,"l2d_access":7454938,"l2d_wb":200338,"l2d_refill":19978}}
// {"timestamp":39342914,"fuel":null,"run_idx":246,"refuel_idx":0,"intruder_state":0,"dt":61785,"df":null,"acc_t":61785,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3726161,"l1d_refill":3726162,"l2d_access":7452324,"l2d_wb":200727,"l2d_refill":20028}}
// {"timestamp":39424358,"fuel":null,"run_idx":247,"refuel_idx":0,"intruder_state":0,"dt":58848,"df":null,"acc_t":58848,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3726162,"l1d_refill":3726163,"l2d_access":7452326,"l2d_wb":201276,"l2d_refill":19970}}
// {"timestamp":39507301,"fuel":null,"run_idx":248,"refuel_idx":0,"intruder_state":0,"dt":60345,"df":null,"acc_t":60345,"acc_f":null,"pmu_info":{"l1d_access":8962119,"l1d_wb":3729292,"l1d_refill":3729293,"l2d_access":7458586,"l2d_wb":201348,"l2d_refill":19994}}
