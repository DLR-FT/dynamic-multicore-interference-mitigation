use core::fmt::Write;

use arm64::perf::PerfMonitor;
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

    let df = 1000 * 100;

    instance.set_fuel(Some(df));

    PerfMonitor::enable_cycle_counter();

    PerfMonitor::reset_cycle_counter();
    let mut last_time = PerfMonitor::get_cycle_counter();

    PerfMonitor::start_cycle_counter();
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
                PerfMonitor::stop_cycle_counter();
                let curr_time = PerfMonitor::get_cycle_counter();
                let dt = curr_time - last_time;

                UartWriter
                    .write_fmt(format_args!(
                        "dt/df: {} cycles / {}instr refuel ....\r\n",
                        dt, df
                    ))
                    .unwrap();

                res.set_fuel(Some(df));

                PerfMonitor::reset_cycle_counter();
                last_time = PerfMonitor::get_cycle_counter();

                PerfMonitor::start_cycle_counter();
                state = res.resume().unwrap();
            }
            wasm::InvocationState::Canceled => {
                break;
            }
        };
    }

    return Ok(());
}
