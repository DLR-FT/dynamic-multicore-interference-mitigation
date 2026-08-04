use alloc::vec::Vec;

use analyzer::RefuelUpdate;
use arm64::pmu::PMU;
use dlr_wasm_interpreter::{ExternVal, FuncAddr, FuncType, ResultType, RunState, Store};
use embedded_io::Write;

use crate::{
    intruder::{self, INTRUDER_BREAK},
    perfmon::PerfMon,
    systick::SysTick,
};

pub struct WasmRunner<'wasm> {
    pub fuel_amount: Option<u64>,

    run_idx: usize,

    store: Store<'wasm, ()>,
    main_addr: FuncAddr,
}

impl<'wasm, 'log> WasmRunner<'wasm> {
    pub fn new(wasm_bytes: &'wasm [u8], fuel_amount: Option<u64>) -> Self {
        let validation_info =
            dlr_wasm_interpreter::decode_and_validate(wasm_bytes, &mut ()).unwrap();

        let mut store = dlr_wasm_interpreter::Store::new(());

        let func_addr = store.func_alloc(
            FuncType {
                params: ResultType {
                    valtypes: Vec::new(),
                },
                returns: ResultType {
                    valtypes: Vec::new(),
                },
            },
            123,
        );

        let main = unsafe {
            store
                .module_instantiate(
                    &validation_info,
                    alloc::vec![ExternVal::Func(func_addr)],
                    fuel_amount,
                )
                .unwrap()
                .module_addr
        };

        let main_addr = unsafe {
            store
                .instance_export(main, "main")
                .unwrap()
                .as_func()
                .unwrap()
        };

        Self {
            fuel_amount,
            run_idx: 0,

            store,
            main_addr,
        }
    }

    pub fn run(&mut self, mut writer: impl Write) {
        let mut refuel_idx = 0;
        let mut acc_t = 0;
        let mut acc_f = Some(0);

        PerfMon::start();

        let mut last = SysTick::get_time_us();
        let mut state = unsafe {
            self.store
                .invoke(self.main_addr, Vec::new(), self.fuel_amount)
                .unwrap()
        };

        loop {
            let current = SysTick::get_time_us();
            let perf = PerfMon::stop();

            let dt = current - last;

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
                        perf_info: Some(perf),
                    };

                    let buf = &mut [0u8; 1024];
                    let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
                    let _ = writer.write(&buf[..n]);

                    *resumable.fuel_mut() = self.fuel_amount;

                    refuel_idx = refuel_idx + 1;
                    PMU::reset();
                    PMU::start();
                    last = SysTick::get_time_us();
                    state = unsafe { self.store.resume_wasm(resumable).unwrap() };
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
                        perf_info: Some(perf),
                    };

                    let buf = &mut [0u8; 1024];
                    let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
                    let _ = writer.write(&buf[..n]);

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
