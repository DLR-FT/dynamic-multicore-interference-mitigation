use std::fs;
use std::{collections::*, io::*, path::*};

use anyhow::{Ok, Result};
use clap::Parser;
use log::*;
use serde::*;
use tokio::{sync::mpsc, *};

mod cgroup;
mod command_ext;
mod runner;

use runner::*;
use tokio_util::sync::CancellationToken;

use wasm_runner::WasmRunnerIpc;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    primary: (usize, usize),
    processes: HashMap<String, ChildProcess>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChildProcess {
    id: (usize, usize),
    cmd: String,
    args: Vec<String>,
    cpu_core: u32,
    ipc_arg: Option<String>,
}

#[main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    trace!("args = {:?}", args);

    let config = fs::read_to_string(args.config)?;
    let config: Config = toml::from_str(&config)?;
    trace!("config = {:?}", config);

    let mut runner = Runner::new()?;

    for (name, proc) in config.processes {
        info!("add process: {}", name);
        runner.add_process(proc.id, proc.cmd, proc.args, proc.cpu_core, proc.ipc_arg)?;
    }

    let cancel = CancellationToken::new();
    let run_cancel = cancel.clone();

    let mut out_writer = args
        .out
        .map(|out_path| {
            let file = fs::File::create(out_path)?;
            Ok(BufWriter::new(file))
        })
        .transpose()?;

    let (tx, mut rx) = mpsc::channel(4 * 1024);

    let run = runner.run::<WasmRunnerIpc>(config.primary, tx, run_cancel);

    let out_task = spawn(async move {
        loop {
            let Some(x) = rx.recv().await else {
                break;
            };

            let x = serde_json::to_string(&x)?;

            match &mut out_writer {
                Some(writer) => writer.write_fmt(format_args!("{}\n", x))?,
                None => println!("{}", x),
            }
        }

        Ok(())
    });

    pin!(run);

    select! {
        _ = out_task => {
            Ok(())
        },
        res = &mut run =>{
            info!("finished.");
            res
        },
        _ =  signal::ctrl_c() => {
            info!("ctrl-c");
            cancel.cancel();
            run.await
        },
    }
}
