use std::{fs, path::*, time::*, usize};

use anyhow::{Result, anyhow};
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
    fuel: Vec<u32>,

    #[arg(long)]
    wctpf: Vec<u64>,

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

    let mut fuel: Vec<Option<u32>> = args.fuel.into_iter().map(Some).collect();
    if fuel.is_empty() {
        fuel.push(None);
    }

    let mut wctpf: Vec<Option<u64>> = args.wctpf.into_iter().map(Some).collect();
    if wctpf.is_empty() {
        wctpf.push(None);
    }

    for (i, fuel) in fuel.iter().enumerate() {
        for (j, wctpf) in wctpf.iter().enumerate() {
            for k in 0..args.count.unwrap_or(1) {
                let log = WasmRunLogger::new(sender.as_ref(), *fuel, *wctpf, i, j, k);
                run_wasm(&wasm_bytes, *fuel, *wctpf, log)?;
            }
        }
    }

    Ok(())
}

fn run_wasm<'sender>(
    wasm_bytes: &[u8],
    fuel: Option<u32>,
    wctpf: Option<u64>,
    logger: WasmRunLogger<'sender>,
) -> Result<()> {
    let validation_info = validate(wasm_bytes).map_err(|e| anyhow!(e))?;

    let mut store = Store::new(());

    fn wasm_panic_handler(
        _: &mut (),
        _values: Vec<Value>,
    ) -> Result<Vec<Value>, HaltExecutionError> {
        println!("Wasm binary panic!");
        Ok(Vec::new())
    }

    let wasm_panic = store.func_alloc_typed::<(), ()>(wasm_panic_handler);

    let module = store
        .module_instantiate(&validation_info, vec![ExternVal::Func(wasm_panic)], fuel)
        .map_err(|e| anyhow!(e))?
        .module_addr;

    let wasm_main = store
        .instance_export(module, "main")
        .map_err(|e| anyhow!(e))?
        .as_func()
        .ok_or(anyhow!(
            "Wasm module \"{}\" does not export func \"main\"",
            module
        ))?;

    let resumable_ref = store
        .create_resumable(wasm_main, vec![], fuel)
        .map_err(|e| anyhow!(e))?;

    let mitigator = Mitigator::new(wctpf, wctpf.map(|x| x / 10).unwrap_or(0));

    let mut fuel_cycle = 0;
    let mut acc_t = Duration::ZERO;
    let mut acc_f = Some(0);

    logger.log(
        Instant::now(),
        fuel_cycle,
        Duration::ZERO,
        Some(0),
        acc_t,
        acc_f,
        wctpf
            .map(|wctpf| {
                if wctpf == 0 {
                    Irq::Freeze(1)
                } else {
                    Irq::Unfreeze(1)
                }
            })
            .or(Some(Irq::Unfreeze(1))),
    )?;

    fuel_cycle = fuel_cycle + 1;
    let mut last = Instant::now();
    let mut state = store.resume(resumable_ref).map_err(|e| anyhow!(e))?;

    loop {
        let current = Instant::now();
        let dt = current - last;

        match state {
            resumable::RunState::Resumable {
                mut resumable_ref, ..
            } => {
                let df = store
                    .access_fuel_mut(&mut resumable_ref, |f| *f)
                    .map_err(|e| anyhow!(e))?
                    .zip(fuel)
                    .map(|(a, b)| b - a);

                acc_t = acc_t + dt;
                acc_f = acc_f.zip(df).map(|(a, b)| a + b);

                let irq = mitigator.mitigate(acc_t, acc_f);

                logger.log(current, fuel_cycle, dt, df, acc_t, acc_f, irq)?;

                store
                    .access_fuel_mut(&mut resumable_ref, |f| {
                        *f = fuel;
                    })
                    .unwrap();

                fuel_cycle = fuel_cycle + 1;
                last = Instant::now();
                state = store.resume(resumable_ref).map_err(|e| anyhow!(e))?;
                continue;
            }

            resumable::RunState::Finished {
                maybe_remaining_fuel,
                ..
            } => {
                let df = maybe_remaining_fuel.zip(fuel).map(|(a, b)| b - a);

                acc_t = acc_t + dt;
                acc_f = acc_f.zip(df).map(|(a, b)| a + b);

                let irq = mitigator.mitigate(acc_t, acc_f);

                logger.log(current, fuel_cycle, dt, df, acc_t, acc_f, irq)?;

                break;
            }
        }
    }

    return Ok(());
}

struct Mitigator {
    wctpf: Option<u64>,
    wctpf_hyst: u64,
    last_irq: Option<Irq>,
}

impl Mitigator {
    fn new(wctpf: Option<u64>, wctpf_hyst: u64) -> Self {
        Self {
            wctpf,
            wctpf_hyst,
            last_irq: Some(Irq::Unfreeze(1)),
        }
    }

    fn mitigate(&self, acc_t: Duration, acc_f: Option<u32>) -> Option<Irq> {
        let avgtpf = acc_f.map(|acc_f| (acc_t.as_nanos() * 1000) as u64 / acc_f as u64);

        self.wctpf
            .zip(avgtpf)
            .map(|(wctpf, avgtpf)| {
                if avgtpf > wctpf {
                    Some(Irq::Freeze(1))
                } else if avgtpf < (wctpf - self.wctpf_hyst) {
                    Some(Irq::Unfreeze(1))
                } else {
                    self.last_irq
                }
            })
            .flatten()
    }
}

#[derive(Clone, Copy)]
struct WasmRunLogger<'sender> {
    sender: Option<&'sender Sender>,
    timestamp_epoch: Instant,
    fuel: Option<u32>,
    wctpf: Option<u64>,
    fuel_index: usize,
    wctpf_index: usize,
    run_index: usize,
}

impl<'sender> WasmRunLogger<'sender> {
    fn new(
        sender: Option<&'sender Sender>,
        fuel: Option<u32>,
        wctpf: Option<u64>,
        fuel_index: usize,
        wctpf_index: usize,
        run_index: usize,
    ) -> Self {
        Self {
            sender,
            timestamp_epoch: Instant::now(),
            fuel,
            wctpf,
            fuel_index,
            wctpf_index,
            run_index,
        }
    }

    fn log(
        self,
        timestamp: Instant,
        fuel_cycle: usize,
        dt: Duration,
        df: Option<u32>,
        acc_t: Duration,
        acc_f: Option<u32>,
        irq: Option<Irq>,
    ) -> Result<()> {
        let timestamp = timestamp - self.timestamp_epoch;
        let log = WasmRunnerIpc {
            timestamp,
            fuel: self.fuel,
            wctpf: self.wctpf,
            i: self.fuel_index,
            j: self.wctpf_index,
            k: self.run_index,
            l: fuel_cycle,
            dt,
            df,
            acc_t,
            acc_f,
            irq,
        };

        if let Some(sender) = self.sender {
            sender.send(&log)?;
        } else {
            println!("{:?}", log);
        }

        Ok(())
    }
}
