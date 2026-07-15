use analyzer::{PerfInfo, RefuelUpdate};
use arm64::pmu::{self, PMU};
use embedded_io::Write;

use crate::{
    intruder::{self, INTRUDER_BREAK},
    perf::CounterValueExt,
    systick::SysTick,
};

pub struct NativeRunner {
    run_idx: usize,
}

impl<'log> NativeRunner {
    pub fn new() -> Self {
        Self { run_idx: 0 }
    }

    pub fn run(&mut self, mut writer: impl Write) {
        PMU::enable();

        PMU::setup_counter(0, pmu::Event::INST_RETIRED);
        PMU::setup_counter(1, pmu::Event::CHAIN);

        // PMU::setup_counter(2, pmu::Event::MEM_ACCESS);

        PMU::setup_counter(2, pmu::Event::L1D_CACHE);
        PMU::setup_counter(3, pmu::Event::L1D_CACHE_REFILL);

        // PMU::setup_counter(4, pmu::Event::L2D_CACHE);
        // PMU::setup_counter(4, pmu::Event::L2D_CACHE_WB);
        PMU::setup_counter(4, pmu::Event::L2D_CACHE_REFILL);

        PMU::reset();
        PMU::start();

        let last = SysTick::get_time_us();
        wasm_payload::kernel::run::<512, 512, 512, 512>();

        let current = SysTick::get_time_us();
        PMU::stop();

        let dt = current - last;

        let perf_info = PerfInfo {
            cycles: PMU::get_cycle_counter().ok(),

            instr: PMU::get_counter(0).chain(PMU::get_counter(1)).ok(),
            l1d_access: PMU::get_counter(2).ok(),
            l1d_refill: PMU::get_counter(3).ok(),
            l2d_refill: PMU::get_counter(4).ok(),
        };

        let update = RefuelUpdate {
            timestamp: current,
            fuel: None,
            refuel_idx: 0,
            run_idx: self.run_idx,
            intruder_break: INTRUDER_BREAK.load(core::sync::atomic::Ordering::Acquire),
            intruder_set_mask: unsafe { intruder::SET_MASK },
            dt,
            df: None,
            acc_t: dt,
            acc_f: None,
            perf_info: Some(perf_info),
        };

        let buf = &mut [0u8; 1024];
        let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
        writer.write(&buf[..n]);

        self.run_idx += 1;
    }
}
