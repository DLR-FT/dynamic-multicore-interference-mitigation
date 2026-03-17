use core::fmt::Write;

use alloc::vec::Vec;

use arm64::pmu::PMU;
use wasm::{ExternVal, HaltExecutionError, Value, resumable};

use crate::{systick::SysTick, uart::UartWriter};

pub fn run_wasm(wasm_bytes: &[u8]) {
    let fuel = Some(10000);

    let validation_info = wasm::validate(wasm_bytes).unwrap();

    let mut store = wasm::Store::new(());

    fn wasm_panic_handler(
        _: &mut (),
        _values: Vec<Value>,
    ) -> Result<Vec<Value>, HaltExecutionError> {
        Err(HaltExecutionError)
    }

    let wasm_panic = store.func_alloc_typed::<(), ()>(wasm_panic_handler);

    let module = store
        .module_instantiate(
            &validation_info,
            alloc::vec![ExternVal::Func(wasm_panic)],
            fuel,
        )
        .unwrap()
        .module_addr;

    let wasm_main = store
        .instance_export(module, "main")
        .unwrap()
        .as_func()
        .unwrap();

    let resumable_ref = store
        .create_resumable(wasm_main, alloc::vec![], fuel)
        .unwrap();

    let mut fuel_cycle = 0;
    let mut last = SysTick::get_time_us();
    let mut state = store.resume(resumable_ref).unwrap();

    loop {
        let current = SysTick::get_time_us();
        let dt = current - last;

        match state {
            resumable::RunState::Resumable {
                mut resumable_ref, ..
            } => {
                let df = store
                    .access_fuel_mut(&mut resumable_ref, |f| *f)
                    .unwrap()
                    .zip(fuel)
                    .map(|(a, b)| b - a);

                UartWriter
                    .write_fmt(format_args!(
                        "refuel {}, df = {:?}, dt = {:?}\n",
                        fuel_cycle, df, dt
                    ))
                    .unwrap();

                store
                    .access_fuel_mut(&mut resumable_ref, |f| {
                        *f = fuel;
                    })
                    .unwrap();

                fuel_cycle = fuel_cycle + 1;
                last = SysTick::get_time_us();
                state = store.resume(resumable_ref).unwrap();
                continue;
            }

            resumable::RunState::Finished {
                maybe_remaining_fuel,
                ..
            } => {
                let df = maybe_remaining_fuel.zip(fuel).map(|(a, b)| b - a);

                UartWriter
                    .write_fmt(format_args!(
                        "refuel {}, df = {:?}, dt = {:?}\n",
                        fuel_cycle, df, dt
                    ))
                    .unwrap();
                break;
            }
        }
    }
}
