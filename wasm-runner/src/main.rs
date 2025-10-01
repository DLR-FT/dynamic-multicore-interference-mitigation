use std::{
    alloc::System,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;

use clap::Parser;
use ipmpsc::{Sender, SharedRingBuffer};
use wasm::*;
use wasm_runner_serde::*;

const WASM_BYTES: &[u8] = include_bytes!("../../2mm.wasm");

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long)]
    buf: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let buf = SharedRingBuffer::open(&args.buf)?;
    let sender = Sender::new(buf);

    loop {
        run_wasm(&sender)?;
    }
}

fn run_wasm(sender: &Sender) -> Result<()> {
    let validation_info = match validate(&WASM_BYTES) {
        Ok(table) => table,
        Err(_err) => {
            return Err(anyhow::anyhow!("wasm error"));
        }
    };

    let mut instance = match RuntimeInstance::new(&validation_info) {
        Ok(instance) => instance,
        Err(_err) => {
            return Err(anyhow::anyhow!("wasm error"));
        }
    };

    let df = 1000 * 1000;

    instance.set_fuel(Some(df));
    let mut last = Instant::now();
    let mut i = 0u32;

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
                let current = Instant::now();
                let dt = current - last;

                let x = WasmMeasurement {
                    timestamp_unix: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    i,
                    dt,
                    df,
                };

                match sender.send_timeout(&x, Duration::from_millis(25)) {
                    Err(_) => println!(
                        "----------------------------------------------------ipc error----------------------------------------------------"
                    ),
                    Ok(false) => println!(
                        "----------------------------------------------------ipc timeout--------------------------------------------------"
                    ),
                    Ok(true) => {}
                };

                res.set_fuel(Some(df));
                i = i + 1;
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
