use analyzer::{PMUInfo, RefuelUpdate};
use arm64::pmu::{self, PMU};

use crate::{CounterValueExt, plat::UART_DRIVER, systick::SysTick, uart_ext::BufWrite};

pub struct NativeRunner {
    run_idx: usize,
}

impl<'log> NativeRunner {
    pub fn new() -> Self {
        Self { run_idx: 0 }
    }

    pub fn run(&mut self, intruder_state: usize) {
        PMU::enable();

        // PMU::setup_counter(0, pmu::Event::INST_RETIRED);
        // PMU::setup_counter(1, pmu::Event::CHAIN);

        PMU::setup_counter(0, pmu::Event::L1D_CACHE);
        PMU::setup_counter(1, pmu::Event::L1D_CACHE_WB);
        PMU::setup_counter(2, pmu::Event::L1D_CACHE_REFILL);

        PMU::setup_counter(3, pmu::Event::L2D_CACHE);
        PMU::setup_counter(4, pmu::Event::L2D_CACHE_WB);
        PMU::setup_counter(5, pmu::Event::L2D_CACHE_REFILL);

        PMU::reset();
        PMU::start();

        let last = SysTick::get_time_us();
        wasm_payload::kernel::run::<256, 256, 256, 256>();

        let current = SysTick::get_time_us();
        PMU::stop();

        let dt = current - last;

        let pmu_info = PMUInfo {
            cycles: PMU::get_cycle_counter().ok(),

            instr: None, //PMU::get_counter(0).chain(PMU::get_counter(1)).ok(),

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
            refuel_idx: 0,
            run_idx: self.run_idx,
            intruder_state,
            dt,
            df: None,
            acc_t: dt,
            acc_f: None,
            pmu_info: Some(pmu_info),
        };

        let buf = &mut [0u8; 1024];
        let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
        buf[n] = '\n' as u8;
        UART_DRIVER.write_bytes(&buf[..n + 1]);

        self.run_idx += 1;
    }
}
