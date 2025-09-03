use core::fmt::Write;

use wasm::{RuntimeInstance, validate};

use crate::{systick::SysTick, uart::UartWriter};

pub fn run_wasm(wasm_bytes: &[u8]) -> Result<(), ()> {
    let validation_info = match validate(&wasm_bytes) {
        Ok(table) => table,
        Err(_err) => {
            return Err(());
        }
    };

    let mut instance = match RuntimeInstance::new(&validation_info) {
        Ok(instance) => instance,
        Err(_err) => {
            return Err(());
        }
    };

    let df = 100000;

    instance.set_fuel(Some(df));

    let mut last_time: u64 = SysTick::get_time_us();

    let mut state = instance
        .invoke_resumable(
            &instance
                .get_function_by_name(&instance.modules[0].name, "main")
                .unwrap(),
            (0u32, 0u32),
        )
        .unwrap();

    let mut res: Option<i32> = None;
    loop {
        match state {
            wasm::InvocationState::Finished(ret) => {
                res.replace(ret);
                break;
            }
            wasm::InvocationState::OutOfFuel(mut res) => {
                let curr_time = SysTick::get_time_us();
                let dt = curr_time - last_time;

                UartWriter
                    .write_fmt(format_args!(
                        "dt/df: {}us / {}instr refuel ....\r\n",
                        dt, df
                    ))
                    .unwrap();

                last_time = SysTick::get_time_us();

                res.set_fuel(Some(df));
                state = res.resume().unwrap();
            }
            wasm::InvocationState::Canceled => {
                break;
            }
        };
    }

    return Ok(());
}
