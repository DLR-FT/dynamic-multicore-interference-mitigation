use alloc::vec::Vec;

use analyzer::RefuelUpdate;
use dlr_wasm_interpreter::{
    ExternVal, FuncAddr, FuncType, ResultType, RunState, Store, WasmResumable,
};
use embedded_io::Write;

use crate::{
    intruder::{self, INTRUDER_BREAK},
    perfmon::PerfMon,
    systick::SysTick,
};

pub struct WasmRunner<'wasm> {
    pub fuel_amount: Option<u64>,

    run_idx: usize,
    refuel_idx: usize,

    acc_t: u64,
    acc_f: Option<u64>,

    store: Store<'wasm, ()>,
    main_addr: FuncAddr,
    resumeable: Option<WasmResumable>,
}

impl<'wasm, 'log> WasmRunner<'wasm> {
    pub fn new(wasm_bytes: &'wasm [u8], fuel_amount: Option<u64>) -> Self {
        let module = dlr_wasm_interpreter::decode_and_validate(wasm_bytes, &mut ()).unwrap();

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
                    &module,
                    alloc::vec![ExternVal::Func(func_addr)],
                    fuel_amount,
                )
                .unwrap()
                .module_addr
        };

        // let x = unsafe {
        //     store
        //         .instance_export(main, "alloc_buf")
        //         .unwrap()
        //         .as_global()
        //         .unwrap()
        // };

        // let Value::I32(y) = (unsafe { store.global_read(x) }) else {
        //     panic!("sdhsdhdfdf");
        // };

        // let mem = unsafe {
        //     store
        //         .instance_export(main, "memory")
        //         .unwrap()
        //         .as_mem()
        //         .unwrap()
        // };

        // let mem_buf = unsafe { store.mem_data_mut(mem) };

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
            refuel_idx: 0,

            acc_t: 0,
            acc_f: fuel_amount.map(|_| 0),

            store,
            main_addr,
            resumeable: None,
        }
    }

    pub fn run(&mut self, mut writer: impl Write) {
        PerfMon::start();
        let last = SysTick::get_time_us();

        let state = match self.resumeable.take() {
            Some(resumable) => unsafe { self.store.resume_wasm(resumable).unwrap() },
            _ => unsafe {
                self.store
                    .invoke(self.main_addr, Vec::new(), self.fuel_amount)
                    .unwrap()
            },
        };

        let current = SysTick::get_time_us();
        let perf = PerfMon::stop();
        let dt = current - last;
        let df;

        match state {
            RunState::Resumable { mut resumable, .. } => {
                df = resumable.fuel().zip(self.fuel_amount).map(|(a, b)| b - a);

                *resumable.fuel_mut() = self.fuel_amount;
                self.resumeable.replace(resumable);
            }
            RunState::Finished {
                maybe_remaining_fuel,
                ..
            } => {
                df = maybe_remaining_fuel
                    .zip(self.fuel_amount)
                    .map(|(a, b)| b - a);
            }
            RunState::HostCalled { .. } => panic!("Wasm panic!"),
        }

        self.acc_t += dt;
        self.acc_f = self.acc_f.zip(df).map(|(a, b)| a + b);

        let update = RefuelUpdate {
            timestamp: current,
            fuel: self.fuel_amount,
            run_idx: self.run_idx,
            refuel_idx: self.refuel_idx,
            intruder_break: INTRUDER_BREAK.load(core::sync::atomic::Ordering::Acquire),
            intruder_set_mask: unsafe { intruder::SET_MASK },
            dt,
            df,
            acc_t: self.acc_t,
            acc_f: self.acc_f,
            perf_info: Some(perf),
        };

        let buf = &mut [0u8; 1024];
        let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
        let _ = writer.write(&buf[..n]);

        if self.resumeable.is_some() {
            self.refuel_idx += 1;
        } else {
            self.refuel_idx = 0;
            self.run_idx += 1;

            self.acc_t = 0;
            self.acc_f = self.fuel_amount.map(|_| 0);
        }
    }
}
