use std::{fs, path::*, time::*, usize};

use anyhow::Result;
use clap::Parser;
use ipmpsc::*;
use simple_moving_average::*;
use wasm::*;

use wasm_runner::WasmRunnerIpc;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long)]
    wasm: PathBuf,

    #[arg(long)]
    fuel: Vec<usize>,

    #[arg(long)]
    count: Option<usize>,

    #[arg(long)]
    ipc: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let wasm_bytes = fs::read(args.wasm)?;

    let sender = args
        .ipc
        .map(|path| SharedRingBuffer::open(path.to_str().unwrap()))
        .transpose()?
        .map(Sender::new);

    let mut fuel: Vec<usize> = args.fuel;
    if fuel.is_empty() {
        fuel.push(1000);
    }

    let mut i = 0;
    for fuel in fuel.iter() {
        for j in 0..args.count.unwrap_or(1) {
            run_wasm(&wasm_bytes, &sender, *fuel, i, j)?;
            i = i + 1;
        }
    }

    Ok(())
}

fn run_wasm(
    wasm_bytes: &[u8],
    sender: &Option<Sender>,
    fuel: usize,
    i: usize,
    j: usize,
) -> Result<()> {
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

    instance.set_fuel(Some(fuel));
    let mut last = Instant::now();
    let mut k = 0;

    let mut state = instance
        .invoke_resumable(
            &instance
                .get_function_by_name(&instance.modules[0].name, "main")
                .unwrap(),
            (0u32, 0u32),
        )
        .unwrap();

    let mut ma = SingleSumSMA::<u64, u64, 3>::new();

    let mut res: Option<i32> = None;
    loop {
        match state {
            wasm::InvocationState::Finished(ret) => {
                let current = Instant::now();
                let dt = (current - last).as_nanos() as u64;
                let df = fuel - instance.get_fuel().unwrap();

                ma.add_sample(dt * 1000 / df as u64);
                let ma_tpf = ma.get_average();

                let x = WasmRunnerIpc {
                    timestamp_unix: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    fuel,
                    i,
                    j,
                    k,
                    dt,
                    df,
                    ma_tpf,
                    irq: None,
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
                let dt = (current - last).as_nanos() as u64;
                let df = fuel - res.get_fuel().unwrap();

                ma.add_sample(dt * 1000 / df as u64);
                let ma_tpf = ma.get_average();

                let x = WasmRunnerIpc {
                    timestamp_unix: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    fuel,
                    i,
                    j,
                    k,
                    dt,
                    ma_tpf,
                    df,
                    irq: None,
                };

                match sender {
                    Some(sender) => sender.send(&x)?,
                    None => {
                        println!("{:?}", x);
                    }
                }

                res.set_fuel(Some(df));
                k = k + 1;
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
