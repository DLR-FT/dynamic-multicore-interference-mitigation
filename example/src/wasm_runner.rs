use alloc::vec::{self, Vec};

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
    run_state: Option<RunState>,
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
            run_state: None,
        }
    }

    pub fn run(&mut self, mut writer: impl Write) {
        let run_state = match self.run_state.take() {
            Some(RunState::Resumable { resumable, .. }) => unsafe {
                self.store.resume_wasm(resumable).unwrap()
            },
            _ => unsafe {
                self.store
                    .invoke(self.main_addr, Vec::new(), self.fuel_amount)
                    .unwrap()
            },
        };

        match &run_state {
            RunState::Resumable { resumable, .. } => {}
            RunState::Finished {
                maybe_remaining_fuel,
                ..
            } => {}
            RunState::HostCalled { .. } => {
                panic!("wasm panic!")
            }
        }

        self.run_state.replace(run_state);
    }
}
