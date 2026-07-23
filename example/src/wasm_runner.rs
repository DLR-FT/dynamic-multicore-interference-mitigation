use alloc::vec::Vec;

use analyzer::{PerfInfo, RefuelUpdate};
use arm64::pmu::{self, PMU};
use dlr_wasm_interpreter::RunState;
use embedded_io::Write;

use crate::{
    CounterValueExt,
    intruder::{self, INTRUDER_BREAK},
    systick::SysTick,
};

pub struct WasmRunner<'wasm> {
    pub fuel_amount: Option<u64>,

    run_idx: usize,

    wasm_bytes: &'wasm [u8],
}

impl<'wasm, 'log> WasmRunner<'wasm> {
    pub fn new(wasm_bytes: &'wasm [u8], fuel_amount: Option<u64>) -> Self {
        Self {
            fuel_amount,
            run_idx: 0,

            wasm_bytes,
        }
    }

    pub fn run(&mut self, mut writer: impl Write) {
        let validation_info =
            dlr_wasm_interpreter::decode_and_validate(self.wasm_bytes, &mut ()).unwrap();
        let mut store = dlr_wasm_interpreter::Store::new(());

        let main = unsafe {
            store
                .module_instantiate(&validation_info, alloc::vec![], self.fuel_amount)
                .unwrap()
                .module_addr
        };

        let wasm_main = unsafe {
            store
                .instance_export(main, "main")
                .unwrap()
                .as_func()
                .unwrap()
        };

        PMU::enable();

        PMU::setup_counter(0, pmu::Event::INST_RETIRED);
        PMU::setup_counter(1, pmu::Event::CHAIN);

        // PMU::setup_counter(2, pmu::Event::MEM_ACCESS);

        PMU::setup_counter(2, pmu::Event::L1D_CACHE);
        PMU::setup_counter(3, pmu::Event::L1D_CACHE_REFILL);

        // PMU::setup_counter(4, pmu::Event::L2D_CACHE);
        // PMU::setup_counter(4, pmu::Event::L2D_CACHE_WB);
        PMU::setup_counter(4, pmu::Event::L2D_CACHE_REFILL);

        let mut refuel_idx = 0;
        let mut acc_t = 0;
        let mut acc_f = Some(0);

        PMU::reset();
        PMU::start();

        let mut last = SysTick::get_time_us();
        let mut state = unsafe {
            store
                .invoke(wasm_main, Vec::new(), self.fuel_amount)
                .unwrap()
        };

        loop {
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

            match state {
                RunState::Resumable { mut resumable, .. } => {
                    let df = resumable.fuel().zip(self.fuel_amount).map(|(a, b)| b - a);

                    acc_t = acc_t + dt;
                    acc_f = acc_f.zip(df).map(|(a, b)| a + b);

                    let update = RefuelUpdate {
                        timestamp: current,
                        fuel: self.fuel_amount,
                        run_idx: self.run_idx,
                        refuel_idx,
                        intruder_break: INTRUDER_BREAK.load(core::sync::atomic::Ordering::Acquire),
                        intruder_set_mask: unsafe { intruder::SET_MASK },
                        dt,
                        df,
                        acc_t,
                        acc_f,
                        perf_info: Some(perf_info),
                    };

                    let buf = &mut [0u8; 1024];
                    let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
                    writer.write(&buf[..n]);

                    *resumable.fuel_mut() = self.fuel_amount;

                    refuel_idx = refuel_idx + 1;
                    PMU::reset();
                    PMU::start();
                    last = SysTick::get_time_us();
                    state = unsafe { store.resume_wasm(resumable).unwrap() };
                    continue;
                }

                RunState::Finished {
                    maybe_remaining_fuel,
                    ..
                } => {
                    let df = maybe_remaining_fuel
                        .zip(self.fuel_amount)
                        .map(|(a, b)| b - a);

                    acc_t = acc_t + dt;
                    acc_f = acc_f.zip(df).map(|(a, b)| a + b);

                    let update = RefuelUpdate {
                        timestamp: current,
                        fuel: self.fuel_amount,
                        refuel_idx,
                        run_idx: self.run_idx,
                        intruder_break: INTRUDER_BREAK.load(core::sync::atomic::Ordering::Acquire),
                        intruder_set_mask: unsafe { intruder::SET_MASK },
                        dt,
                        df,
                        acc_t,
                        acc_f,
                        perf_info: Some(perf_info),
                    };

                    let buf = &mut [0u8; 1024];
                    let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
                    writer.write(&buf[..n]);

                    break;
                }

                RunState::HostCalled { .. } => {
                    panic!("Wasm panic")
                }
            }
        }

        self.run_idx += 1;
    }
}
