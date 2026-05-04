use alloc::vec::Vec;

use analyzer::{PMUInfo, RefuelUpdate};
use arm64::pmu::{self, PMU};
use wasm::{ExternVal, HaltExecutionError, Value, resumable};

use crate::{CounterValueExt, plat::UART_DRIVER, systick::SysTick, uart_ext::BufWrite};

pub struct WasmRunner<'wasm> {
    pub fuel_amount: Option<u32>,

    run_idx: usize,

    wasm_bytes: &'wasm [u8],
}

impl<'wasm, 'log> WasmRunner<'wasm> {
    pub fn new(wasm_bytes: &'wasm [u8], fuel_amount: Option<u32>) -> Self {
        Self {
            fuel_amount,
            run_idx: 0,

            wasm_bytes,
        }
    }

    pub fn run(&mut self, intruder_state: usize) {
        let validation_info = wasm::validate(self.wasm_bytes).unwrap();
        let mut store = wasm::Store::new(());

        fn wasm_panic_handler(
            _: &mut (),
            _values: Vec<Value>,
        ) -> Result<Vec<Value>, HaltExecutionError> {
            Err(HaltExecutionError)
        }

        let wasm_panic = store.func_alloc_typed::<(), ()>(wasm_panic_handler);

        let main = store
            .module_instantiate(
                &validation_info,
                alloc::vec![ExternVal::Func(wasm_panic)],
                self.fuel_amount,
            )
            .unwrap()
            .module_addr;

        let wasm_main = store
            .instance_export(main, "main")
            .unwrap()
            .as_func()
            .unwrap();

        let resumable_ref = store
            .create_resumable(wasm_main, alloc::vec![], self.fuel_amount)
            .unwrap();

        PMU::enable();

        PMU::setup_counter(0, pmu::Event::INST_RETIRED);
        PMU::setup_counter(1, pmu::Event::CHAIN);

        // PMU::setup_counter(1, pmu::Event::L1D_CACHE);
        // PMU::setup_counter(2, pmu::Event::L1D_CACHE_WB);
        // PMU::setup_counter(3, pmu::Event::L1D_CACHE_REFILL);

        PMU::setup_counter(2, pmu::Event::L2D_CACHE);
        PMU::setup_counter(3, pmu::Event::L2D_CACHE_WB);
        PMU::setup_counter(4, pmu::Event::L2D_CACHE_REFILL);

        let mut refuel_idx = 0;
        let mut acc_t = 0;
        let mut acc_f = Some(0);

        PMU::reset();
        PMU::start();

        let mut last = SysTick::get_time_us();
        let mut state = store.resume(resumable_ref).unwrap();

        loop {
            let current = SysTick::get_time_us();
            PMU::stop();

            let dt = current - last;

            let pmu_info = PMUInfo {
                cycles: PMU::get_cycle_counter().ok(),

                instr: PMU::get_counter(0).chain(PMU::get_counter(1)).ok(),

                l1d_access: None,
                l1d_wb: None,
                l1d_refill: None,

                l2d_access: PMU::get_counter(2).ok(),
                l2d_wb: PMU::get_counter(3).ok(),
                l2d_refill: PMU::get_counter(4).ok(),
            };

            match state {
                resumable::RunState::Resumable {
                    mut resumable_ref, ..
                } => {
                    let df = store
                        .access_fuel_mut(&mut resumable_ref, |f| *f)
                        .unwrap()
                        .zip(self.fuel_amount)
                        .map(|(a, b)| b - a);

                    acc_t = acc_t + dt;
                    acc_f = acc_f.zip(df).map(|(a, b)| a + b);

                    let update = RefuelUpdate {
                        timestamp: current,
                        fuel: self.fuel_amount,
                        run_idx: self.run_idx,
                        refuel_idx,
                        intruder_state,
                        dt,
                        df,
                        acc_t,
                        acc_f,
                        pmu_info: Some(pmu_info),
                    };

                    let buf = &mut [0u8; 1024];
                    let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
                    buf[n] = '\n' as u8;
                    UART_DRIVER.write_bytes(&buf[..n + 1]);

                    store
                        .access_fuel_mut(&mut resumable_ref, |f| {
                            *f = self.fuel_amount;
                        })
                        .unwrap();

                    refuel_idx = refuel_idx + 1;
                    PMU::reset();
                    PMU::start();
                    last = SysTick::get_time_us();
                    state = store.resume(resumable_ref).unwrap();
                    continue;
                }

                resumable::RunState::Finished {
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
                        intruder_state,
                        dt,
                        df,
                        acc_t,
                        acc_f,
                        pmu_info: Some(pmu_info),
                    };

                    let buf = &mut [0u8; 1024];
                    let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
                    buf[n] = '\n' as u8;
                    UART_DRIVER.write_bytes(&buf[..n + 1]);

                    break;
                }
            }
        }

        self.run_idx += 1;
    }
}
