use std::time::Instant;

use anyhow::Result;

use wasm::*;

const WASM_BYTES: &[u8] = include_bytes!("../../2mm.wasm");

fn main() -> Result<()> {
    println!("Hello, world!");

    loop {
        run_wasm()?;
    }
}

fn run_wasm() -> Result<()> {
    let validation_info = match validate(&WASM_BYTES) {
        Ok(table) => table,
        Err(_err) => {
            return Err(anyhow::anyhow!("dfhdfhdfdfh"));
        }
    };

    let mut instance = match RuntimeInstance::new(&validation_info) {
        Ok(instance) => instance,
        Err(_err) => {
            return Err(anyhow::anyhow!("dfhdfhdfdfh"));
        }
    };

    let df = 1000 * 1000;

    instance.set_fuel(Some(df));

    let mut last = Instant::now();

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
        let current = Instant::now();
        match state {
            wasm::InvocationState::Finished(ret) => {
                res.replace(ret);
                break;
            }
            wasm::InvocationState::OutOfFuel(mut res) => {
                let dt = (current - last).as_micros();

                println!("dt/df = {}/{}", dt, df);
                res.set_fuel(Some(df));

                last = Instant::now();
                state = res.resume().unwrap();
            }
            wasm::InvocationState::Canceled => {
                break;
            }
        };
    }

    return Ok(());
}
