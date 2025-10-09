use std::{
    fs,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
    usize,
};

use anyhow::Result;

use clap::Parser;
use ipmpsc::{Sender, SharedRingBuffer};
use wasm::*;
use wasm_runner_serde::*;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long)]
    wasm: PathBuf,

    #[arg(long)]
    fuel: Option<usize>,

    #[arg(long)]
    count: Option<usize>,

    #[arg(long)]
    buf: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let wasm_bytes = fs::read(args.wasm)?;

    let sender = args
        .buf
        .map(|path| SharedRingBuffer::open(path.to_str().unwrap()))
        .transpose()?
        .map(Sender::new);

    for i in 0..args.count.unwrap_or(usize::MAX) {
        run_wasm(&wasm_bytes, &sender, args.fuel, i)?
    }

    Ok(())
}

fn run_wasm(wasm_bytes: &[u8], sender: &Option<Sender>, df: Option<usize>, i: usize) -> Result<()> {
    let validation_info = match validate(wasm_bytes) {
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

    instance.set_fuel(df);
    let mut last = Instant::now();
    let mut j = 0;

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
                let current = Instant::now();
                let dt = current - last;
                let df = instance.get_fuel().zip(df).map(|a| a.1 - a.0);

                let x = WasmMeasurement {
                    timestamp_unix: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    i,
                    j,
                    dt,
                    df,
                };

                match sender {
                    Some(sender) => sender.send(&x)?,
                    None => {
                        println!("{:?}", x);
                    }
                }

                res.replace(ret);
                break;
            }
            wasm::InvocationState::OutOfFuel(mut res) => {
                let current = Instant::now();
                let dt = current - last;
                let df = res.get_fuel().zip(df).map(|a| a.1 - a.0);

                let x = WasmMeasurement {
                    timestamp_unix: SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
                    i,
                    j,
                    dt,
                    df,
                };

                match sender {
                    Some(sender) => sender.send(&x)?,
                    None => {
                        println!("{:?}", x);
                    }
                }

                // match sender.send_timeout(&x, Duration::from_millis(25)) {
                //     Err(_) => println!(
                //         "----------------------------------------------------ipc error----------------------------------------------------"
                //     ),
                //     Ok(false) => println!(
                //         "----------------------------------------------------ipc timeout--------------------------------------------------"
                //     ),
                //     Ok(true) => {}
                // };

                res.set_fuel(df);
                j = j + 1;
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
