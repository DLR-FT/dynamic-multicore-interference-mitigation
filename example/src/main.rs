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

    // start_core::<IntruderEntryImpl>(1);
    // start_core::<IntruderEntryImpl>(2);
    // start_core::<IntruderEntryImpl>(3);

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

        // PMU::enable();

        // PMU::setup_counter(0, pmu::Event::L1D_CACHE);
        // PMU::setup_counter(1, pmu::Event::L1D_CACHE_WB);
        // PMU::setup_counter(2, pmu::Event::L1D_CACHE_REFILL);

        // PMU::setup_counter(3, pmu::Event::L2D_CACHE);
        // PMU::setup_counter(4, pmu::Event::L2D_CACHE_WB);
        // PMU::setup_counter(5, pmu::Event::L2D_CACHE_REFILL);

        // PMU::reset();
        // PMU::start();
        // let t1 = SysTick::get_time_us();
        // wasm_payload::kernel::run::<256, 256, 256, 256>();
        // let t2 = SysTick::get_time_us();
        // PMU::stop();

        // let dt = t2 - t1;
        // let l1d = PMU::get_counter(0);
        // let l1d_wb = PMU::get_counter(1);
        // let l1d_refill = PMU::get_counter(2);

        // let l2d = PMU::get_counter(3);
        // let l2d_wb = PMU::get_counter(4);
        // let l2d_refill = PMU::get_counter(5);

        // write!(UartWriter, "dt = {:?}, l1d = {:?}, l1d_wb = {:?}, l1d_refill = {:?}, l2d = {:?}, l2d_wb = {:?}, l2d_refill = {:?}\n", dt, l1d, l1d_wb, l1d_refill, l2d, l2d_wb, l2d_refill).unwrap();
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

// Hello World
// cpu_idx = 0 ...
// dt = 406307, l1d = Ok(69402684), l1d_wb = Ok(32969908), l1d_refill = Ok(32970399), l2d = Ok(65940347), l2d_wb = Ok(46532), l2d_refill = Ok(89757)
// dt = 406511, l1d = Ok(69402683), l1d_wb = Ok(32972827), l1d_refill = Ok(32972827), l2d = Ok(65945654), l2d_wb = Ok(49122), l2d_refill = Ok(84897)
// dt = 406500, l1d = Ok(69402683), l1d_wb = Ok(32972085), l1d_refill = Ok(32972085), l2d = Ok(65944170), l2d_wb = Ok(49204), l2d_refill = Ok(85057)
// dt = 396883, l1d = Ok(69402683), l1d_wb = Ok(32965972), l1d_refill = Ok(32965972), l2d = Ok(65931945), l2d_wb = Ok(49161), l2d_refill = Ok(84978)
// dt = 406470, l1d = Ok(69402683), l1d_wb = Ok(32970227), l1d_refill = Ok(32970227), l2d = Ok(65940454), l2d_wb = Ok(49192), l2d_refill = Ok(84625)
// dt = 396908, l1d = Ok(69402683), l1d_wb = Ok(32967807), l1d_refill = Ok(32967807), l2d = Ok(65935614), l2d_wb = Ok(49179), l2d_refill = Ok(85219)
// dt = 406459, l1d = Ok(69402683), l1d_wb = Ok(32971011), l1d_refill = Ok(32971011), l2d = Ok(65942022), l2d_wb = Ok(49203), l2d_refill = Ok(84982)
// dt = 406490, l1d = Ok(69402683), l1d_wb = Ok(32971022), l1d_refill = Ok(32971022), l2d = Ok(65942044), l2d_wb = Ok(49146), l2d_refill = Ok(85180)
// dt = 406524, l1d = Ok(69402683), l1d_wb = Ok(32975017), l1d_refill = Ok(32975017), l2d = Ok(65950034), l2d_wb = Ok(49157), l2d_refill = Ok(85053)
// dt = 396890, l1d = Ok(69402683), l1d_wb = Ok(32964189), l1d_refill = Ok(32964189), l2d = Ok(65928378), l2d_wb = Ok(49185), l2d_refill = Ok(85265)
// dt = 396925, l1d = Ok(69402683), l1d_wb = Ok(32967239), l1d_refill = Ok(32967239), l2d = Ok(65934478), l2d_wb = Ok(49194), l2d_refill = Ok(85387)
// dt = 406458, l1d = Ok(69402683), l1d_wb = Ok(32971656), l1d_refill = Ok(32971656), l2d = Ok(65943312), l2d_wb = Ok(49193), l2d_refill = Ok(84941)
// dt = 406451, l1d = Ok(69402683), l1d_wb = Ok(32971239), l1d_refill = Ok(32971239), l2d = Ok(65942478), l2d_wb = Ok(49177), l2d_refill = Ok(84718)
// dt = 396885, l1d = Ok(69402683), l1d_wb = Ok(32964781), l1d_refill = Ok(32964781), l2d = Ok(65929562), l2d_wb = Ok(49167), l2d_refill = Ok(85068)
// dt = 406488, l1d = Ok(69402683), l1d_wb = Ok(32970975), l1d_refill = Ok(32970975), l2d = Ok(65941950), l2d_wb = Ok(49198), l2d_refill = Ok(85479)
// dt = 406469, l1d = Ok(69402683), l1d_wb = Ok(32972385), l1d_refill = Ok(32972385), l2d = Ok(65944770), l2d_wb = Ok(49141), l2d_refill = Ok(85226)
// dt = 396869, l1d = Ok(69402683), l1d_wb = Ok(32964864), l1d_refill = Ok(32964864), l2d = Ok(65929728), l2d_wb = Ok(49205), l2d_refill = Ok(84955)
// dt = 396916, l1d = Ok(69402683), l1d_wb = Ok(32965898), l1d_refill = Ok(32965898), l2d = Ok(65931796), l2d_wb = Ok(49194), l2d_refill = Ok(85546)
// dt = 406470, l1d = Ok(69402683), l1d_wb = Ok(32972414), l1d_refill = Ok(32972414), l2d = Ok(65944828), l2d_wb = Ok(49175), l2d_refill = Ok(84797)
// dt = 406491, l1d = Ok(69402683), l1d_wb = Ok(32970511), l1d_refill = Ok(32970511), l2d = Ok(65941022), l2d_wb = Ok(49142), l2d_refill = Ok(84866)
// dt = 406464, l1d = Ok(69402683), l1d_wb = Ok(32972493), l1d_refill = Ok(32972493), l2d = Ok(65944986), l2d_wb = Ok(49226), l2d_refill = Ok(84954)
// dt = 406465, l1d = Ok(69402683), l1d_wb = Ok(32968942), l1d_refill = Ok(32968942), l2d = Ok(65937884), l2d_wb = Ok(49130), l2d_refill = Ok(84897)
// dt = 406467, l1d = Ok(69402683), l1d_wb = Ok(32970459), l1d_refill = Ok(32970459), l2d = Ok(65940918), l2d_wb = Ok(49198), l2d_refill = Ok(85144)
// dt = 406459, l1d = Ok(69402683), l1d_wb = Ok(32968676), l1d_refill = Ok(32968676), l2d = Ok(65937352), l2d_wb = Ok(49189), l2d_refill = Ok(85006)
// dt = 396906, l1d = Ok(69402683), l1d_wb = Ok(32967241), l1d_refill = Ok(32967241), l2d = Ok(65934482), l2d_wb = Ok(49164), l2d_refill = Ok(84981)
// dt = 406447, l1d = Ok(69402683), l1d_wb = Ok(32970670), l1d_refill = Ok(32970670), l2d = Ok(65941340), l2d_wb = Ok(49179), l2d_refill = Ok(84979)
// dt = 406464, l1d = Ok(69402683), l1d_wb = Ok(32970614), l1d_refill = Ok(32970614), l2d = Ok(65941228), l2d_wb = Ok(49157), l2d_refill = Ok(84958)
// dt = 406490, l1d = Ok(69402683), l1d_wb = Ok(32970140), l1d_refill = Ok(32970140), l2d = Ok(65940280), l2d_wb = Ok(49181), l2d_refill = Ok(85125)
// dt = 406494, l1d = Ok(69402683), l1d_wb = Ok(32971368), l1d_refill = Ok(32971368), l2d = Ok(65942736), l2d_wb = Ok(49213), l2d_refill = Ok(85091)

// Hello World
// cpu_idx = 0 ...
// cpu_idx = 1 ...
// dt = 991756, l1d = Ok(69402694), l1d_wb = Ok(32962194), l1d_refill = Ok(32962651), l2d = Ok(65924885), l2d_wb = Ok(4173860), l2d_refill = Ok(5845012)
// dt = 989283, l1d = Ok(69402694), l1d_wb = Ok(32965480), l1d_refill = Ok(32965482), l2d = Ok(65930964), l2d_wb = Ok(4168498), l2d_refill = Ok(5784461)
// dt = 988971, l1d = Ok(69402694), l1d_wb = Ok(32961825), l1d_refill = Ok(32961827), l2d = Ok(65923654), l2d_wb = Ok(4167213), l2d_refill = Ok(5780548)
// dt = 988491, l1d = Ok(69402694), l1d_wb = Ok(32963173), l1d_refill = Ok(32963175), l2d = Ok(65926350), l2d_wb = Ok(4166983), l2d_refill = Ok(5774505)
// dt = 988809, l1d = Ok(69402694), l1d_wb = Ok(32965378), l1d_refill = Ok(32965380), l2d = Ok(65930760), l2d_wb = Ok(4166765), l2d_refill = Ok(5776169)
// dt = 988551, l1d = Ok(69402694), l1d_wb = Ok(32962916), l1d_refill = Ok(32962918), l2d = Ok(65925836), l2d_wb = Ok(4166982), l2d_refill = Ok(5777006)
// dt = 989218, l1d = Ok(69402694), l1d_wb = Ok(32964226), l1d_refill = Ok(32964228), l2d = Ok(65928456), l2d_wb = Ok(4167287), l2d_refill = Ok(5785605)
// dt = 972204, l1d = Ok(69402694), l1d_wb = Ok(32961730), l1d_refill = Ok(32961732), l2d = Ok(65923464), l2d_wb = Ok(4141766), l2d_refill = Ok(5644201)
// dt = 988516, l1d = Ok(69402694), l1d_wb = Ok(32964089), l1d_refill = Ok(32964091), l2d = Ok(65928182), l2d_wb = Ok(4166003), l2d_refill = Ok(5777060)
// dt = 988605, l1d = Ok(69402694), l1d_wb = Ok(32964185), l1d_refill = Ok(32964187), l2d = Ok(65928374), l2d_wb = Ok(4166520), l2d_refill = Ok(5775337)
// dt = 991523, l1d = Ok(69402694), l1d_wb = Ok(32961770), l1d_refill = Ok(32961772), l2d = Ok(65923544), l2d_wb = Ok(4174009), l2d_refill = Ok(5842653)
// dt = 988868, l1d = Ok(69402694), l1d_wb = Ok(32961900), l1d_refill = Ok(32961902), l2d = Ok(65923804), l2d_wb = Ok(4165507), l2d_refill = Ok(5779115)
// dt = 991460, l1d = Ok(69402694), l1d_wb = Ok(32959939), l1d_refill = Ok(32959941), l2d = Ok(65919882), l2d_wb = Ok(4173576), l2d_refill = Ok(5843575)
// dt = 988523, l1d = Ok(69402694), l1d_wb = Ok(32963963), l1d_refill = Ok(32963965), l2d = Ok(65927930), l2d_wb = Ok(4166330), l2d_refill = Ok(5774359)
// dt = 988245, l1d = Ok(69402694), l1d_wb = Ok(32963997), l1d_refill = Ok(32963999), l2d = Ok(65927998), l2d_wb = Ok(4167081), l2d_refill = Ok(5771633)
// dt = 989000, l1d = Ok(69402694), l1d_wb = Ok(32966379), l1d_refill = Ok(32966381), l2d = Ok(65932762), l2d_wb = Ok(4168275), l2d_refill = Ok(5783658)
// dt = 988922, l1d = Ok(69402694), l1d_wb = Ok(32964619), l1d_refill = Ok(32964621), l2d = Ok(65929242), l2d_wb = Ok(4167741), l2d_refill = Ok(5780605)

// Hello World
// cpu_idx = 0 ...
// cpu_idx = 1 ...
// cpu_idx = 2 ...
// dt = 989644, l1d = Ok(69402694), l1d_wb = Ok(32959871), l1d_refill = Ok(32960328), l2d = Ok(65920241), l2d_wb = Ok(4467222), l2d_refill = Ok(4283643)
// dt = 992030, l1d = Ok(69402694), l1d_wb = Ok(32960908), l1d_refill = Ok(32960909), l2d = Ok(65921818), l2d_wb = Ok(4467067), l2d_refill = Ok(4266265)
// dt = 991981, l1d = Ok(69402694), l1d_wb = Ok(32962680), l1d_refill = Ok(32962681), l2d = Ok(65925361), l2d_wb = Ok(4466450), l2d_refill = Ok(4267327)
// dt = 991977, l1d = Ok(69402694), l1d_wb = Ok(32961734), l1d_refill = Ok(32961735), l2d = Ok(65923469), l2d_wb = Ok(4466626), l2d_refill = Ok(4267812)
// dt = 989574, l1d = Ok(69402694), l1d_wb = Ok(32961425), l1d_refill = Ok(32961426), l2d = Ok(65922851), l2d_wb = Ok(4467248), l2d_refill = Ok(4285259)
// dt = 991881, l1d = Ok(69402694), l1d_wb = Ok(32961909), l1d_refill = Ok(32961910), l2d = Ok(65923820), l2d_wb = Ok(4466743), l2d_refill = Ok(4267316)
// dt = 989202, l1d = Ok(69402694), l1d_wb = Ok(32961088), l1d_refill = Ok(32961089), l2d = Ok(65922177), l2d_wb = Ok(4467965), l2d_refill = Ok(4285663)
// dt = 991970, l1d = Ok(69402694), l1d_wb = Ok(32963633), l1d_refill = Ok(32963634), l2d = Ok(65927267), l2d_wb = Ok(4466362), l2d_refill = Ok(4266856)
// dt = 989322, l1d = Ok(69402694), l1d_wb = Ok(32960357), l1d_refill = Ok(32960358), l2d = Ok(65920715), l2d_wb = Ok(4467378), l2d_refill = Ok(4285118)
// dt = 992103, l1d = Ok(69402694), l1d_wb = Ok(32963461), l1d_refill = Ok(32963462), l2d = Ok(65926924), l2d_wb = Ok(4466230), l2d_refill = Ok(4265664)
// dt = 989830, l1d = Ok(69402694), l1d_wb = Ok(32959542), l1d_refill = Ok(32959543), l2d = Ok(65919086), l2d_wb = Ok(4468801), l2d_refill = Ok(4285546)
// dt = 992189, l1d = Ok(69402694), l1d_wb = Ok(32961522), l1d_refill = Ok(32961523), l2d = Ok(65923046), l2d_wb = Ok(4466582), l2d_refill = Ok(4268241)
// dt = 992022, l1d = Ok(69402694), l1d_wb = Ok(32961403), l1d_refill = Ok(32961404), l2d = Ok(65922808), l2d_wb = Ok(4466781), l2d_refill = Ok(4266343)
// dt = 992200, l1d = Ok(69402694), l1d_wb = Ok(32960525), l1d_refill = Ok(32960526), l2d = Ok(65921052), l2d_wb = Ok(4467045), l2d_refill = Ok(4267946)

// Hello World
// cpu_idx = 0 ...
// cpu_idx = 1 ...
// cpu_idx = 2 ...
// cpu_idx = 3 ...
// dt = 1158047, l1d = Ok(69402694), l1d_wb = Ok(32957639), l1d_refill = Ok(32958083), l2d = Ok(65915759), l2d_wb = Ok(4557613), l2d_refill = Ok(4331531)
// dt = 1179982, l1d = Ok(69402694), l1d_wb = Ok(32956942), l1d_refill = Ok(32956943), l2d = Ok(65913887), l2d_wb = Ok(4556632), l2d_refill = Ok(4335637)
// dt = 1173408, l1d = Ok(69402694), l1d_wb = Ok(32959508), l1d_refill = Ok(32959509), l2d = Ok(65919017), l2d_wb = Ok(4557617), l2d_refill = Ok(4338639)
// dt = 1176649, l1d = Ok(69402694), l1d_wb = Ok(32958417), l1d_refill = Ok(32958418), l2d = Ok(65916834), l2d_wb = Ok(4556590), l2d_refill = Ok(4337098)
// dt = 1143605, l1d = Ok(69402694), l1d_wb = Ok(32958028), l1d_refill = Ok(32958029), l2d = Ok(65916057), l2d_wb = Ok(4558377), l2d_refill = Ok(4338223)
// dt = 1110783, l1d = Ok(69402694), l1d_wb = Ok(32957857), l1d_refill = Ok(32957858), l2d = Ok(65915715), l2d_wb = Ok(4559968), l2d_refill = Ok(4326684)
// dt = 1109414, l1d = Ok(69402694), l1d_wb = Ok(32958673), l1d_refill = Ok(32958674), l2d = Ok(65917347), l2d_wb = Ok(4559132), l2d_refill = Ok(4332065)
// dt = 1156246, l1d = Ok(69402694), l1d_wb = Ok(32956469), l1d_refill = Ok(32956470), l2d = Ok(65912939), l2d_wb = Ok(4558608), l2d_refill = Ok(4337317)
// dt = 1117549, l1d = Ok(69402694), l1d_wb = Ok(32957793), l1d_refill = Ok(32957794), l2d = Ok(65915587), l2d_wb = Ok(4559310), l2d_refill = Ok(4328485)
// dt = 1112775, l1d = Ok(69402694), l1d_wb = Ok(32957270), l1d_refill = Ok(32957271), l2d = Ok(65914541), l2d_wb = Ok(4558881), l2d_refill = Ok(4324636)
// dt = 1110891, l1d = Ok(69402694), l1d_wb = Ok(32956830), l1d_refill = Ok(32956831), l2d = Ok(65913660), l2d_wb = Ok(4558374), l2d_refill = Ok(4325994)
// dt = 1108279, l1d = Ok(69402694), l1d_wb = Ok(32956653), l1d_refill = Ok(32956654), l2d = Ok(65913307), l2d_wb = Ok(4558234), l2d_refill = Ok(4332267)
// dt = 1114243, l1d = Ok(69402694), l1d_wb = Ok(32957268), l1d_refill = Ok(32957269), l2d = Ok(65914537), l2d_wb = Ok(4559144), l2d_refill = Ok(4325671)
// dt = 1118443, l1d = Ok(69402694), l1d_wb = Ok(32957597), l1d_refill = Ok(32957598), l2d = Ok(65915195), l2d_wb = Ok(4557553), l2d_refill = Ok(4331957)
// dt = 1133299, l1d = Ok(69402694), l1d_wb = Ok(32960546), l1d_refill = Ok(32960547), l2d = Ok(65921093), l2d_wb = Ok(4560525), l2d_refill = Ok(4332267)
// dt = 1165537, l1d = Ok(69402694), l1d_wb = Ok(32958075), l1d_refill = Ok(32958076), l2d = Ok(65916150), l2d_wb = Ok(4557535), l2d_refill = Ok(4337038)

// Hello World
// cpu_idx = 0 ...
//  .....
// refuel 17111, df = Some(10000), dt = 913
// refuel 17112, df = Some(10000), dt = 914
// refuel 17113, df = Some(10000), dt = 911
// refuel 17114, df = Some(10000), dt = 908
// refuel 17115, df = Some(10000), dt = 912
// refuel 17116, df = Some(10000), dt = 911
// refuel 17117, df = Some(10000), dt = 915
// refuel 17118, df = Some(10000), dt = 910
// refuel 17119, df = Some(10000), dt = 912
// refuel 17120, df = Some(10000), dt = 912
// refuel 17121, df = Some(10000), dt = 913
// refuel 17122, df = Some(10000), dt = 914
// refuel 17123, df = Some(10000), dt = 914
// refuel 17124, df = Some(10000), dt = 913
// refuel 17125, df = Some(10000), dt = 912
// refuel 17126, df = Some(10000), dt = 912
// refuel 17127, df = Some(10000), dt = 913
// refuel 17128, df = Some(10000), dt = 913
// refuel 17129, df = Some(10000), dt = 911
// refuel 17130, df = Some(10000), dt = 913
// refuel 17131, df = Some(10000), dt = 912
// refuel 17132, df = Some(10000), dt = 913
// refuel 17133, df = Some(10000), dt = 910
// refuel 17134, df = Some(10000), dt = 908
// refuel 17135, df = Some(10000), dt = 912
// refuel 17136, df = Some(10000), dt = 913
// refuel 17137, df = Some(10000), dt = 912
// refuel 17138, df = Some(10000), dt = 914
// refuel 17139, df = Some(10000), dt = 911
// refuel 17140, df = Some(10000), dt = 914
// refuel 17141, df = Some(10000), dt = 914

// Hello World
// cpu_idx = 0 ...
// cpu_idx = 1 ...
// .....
// refuel 11145, df = Some(10000), dt = 995
// refuel 11146, df = Some(10000), dt = 989
// refuel 11147, df = Some(10000), dt = 992
// refuel 11148, df = Some(10000), dt = 988
// refuel 11149, df = Some(10000), dt = 989
// refuel 11150, df = Some(10000), dt = 992
// refuel 11151, df = Some(10000), dt = 992
// refuel 11152, df = Some(10000), dt = 989
// refuel 11153, df = Some(10000), dt = 985
// refuel 11154, df = Some(10000), dt = 988
// refuel 11155, df = Some(10000), dt = 986
// refuel 11156, df = Some(10000), dt = 990
// refuel 11157, df = Some(10000), dt = 990
// refuel 11158, df = Some(10000), dt = 991
// refuel 11159, df = Some(10000), dt = 991
// refuel 11160, df = Some(10000), dt = 991
// refuel 11161, df = Some(10000), dt = 992
// refuel 11162, df = Some(10000), dt = 992
// refuel 11163, df = Some(10000), dt = 994
// refuel 11164, df = Some(10000), dt = 990
// refuel 11165, df = Some(10000), dt = 994
// refuel 11166, df = Some(10000), dt = 995
// refuel 11167, df = Some(10000), dt = 993
// refuel 11168, df = Some(10000), dt = 992
// refuel 11169, df = Some(10000), dt = 996
// refuel 11170, df = Some(10000), dt = 991
// refuel 11171, df = Some(10000), dt = 992
// refuel 11172, df = Some(10000), dt = 993
// refuel 11173, df = Some(10000), dt = 990
// refuel 11174, df = Some(10000), dt = 995
// refuel 11175, df = Some(10000), dt = 994
