use std::{fs, path::*, time::*, usize};

use anyhow::Result;
use clap::Parser;
use ipc_serde::Irq;
use ipmpsc::*;
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
    wc_tpf: Option<u64>,

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

    for (i, fuel) in fuel.iter().enumerate() {
        for j in 0..args.count.unwrap_or(1) {
            run_wasm(&wasm_bytes, &sender, *fuel, i, j, args.wc_tpf)?;
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
    wc_tpf: Option<u64>,
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

    let mut total_time = 0u64;
    let mut total_fuel = 0usize;
    let mut last_irq = None;

    let mut state = instance
        .invoke_resumable(
            &instance
                .get_function_by_name(&instance.modules[0].name, "main")
                .unwrap(),
            (0u32, 0u32),
        )
        .unwrap();

    // let mut ma = SingleSumSMA::<u64, u64, 10>::new();

    let mut res: Option<i32> = None;
    loop {
        match state {
            wasm::InvocationState::Finished(ret) => {
                let current = Instant::now();
                let dt = (current - last).as_nanos() as u64;
                let df = fuel - instance.get_fuel().unwrap();

                total_time = total_time + dt;
                total_fuel = total_fuel + df;

                // ma.add_sample(dt * 1000 / df as u64);
                // let ma_tpf = ma.get_average();

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
                    avg_tpf: total_time * 1000 / total_fuel as u64,
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

                total_time = total_time + dt;
                total_fuel = total_fuel + df;

                let avg_tpf = total_time * 1000 / total_fuel as u64;

                let irq = wc_tpf.and_then(|wc| {
                    if avg_tpf > wc {
                        Some(Irq::Freeze(1))
                    } else if avg_tpf < (wc - 100) {
                        Some(Irq::Unfreeze(1))
                    } else {
                        last_irq
                    }
                });

                last_irq = irq;

                // ma.add_sample(dt * 1000 / df as u64);
                // let ma_tpf = ma.get_average();

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
                    avg_tpf,
                    df,
                    irq,
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
