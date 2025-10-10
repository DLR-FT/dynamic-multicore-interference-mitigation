use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::*;
use clap::Parser;
use ipmpsc::{Receiver, SharedRingBuffer};
use log::*;
use serde::*;
use tokio::{
    sync::watch::{self},
    task::yield_now,
    *,
};

mod cgroup;
mod command_ext;
mod runner;

use runner::*;
use tokio_util::sync::CancellationToken;

use wasm_runner_serde::WasmMeasurement;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    main: ChildCmd,
    intruders: HashMap<String, ChildCmd>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChildCmd {
    cmd: String,
    args: Vec<String>,
    cpu_core: u32,
}

#[main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    trace!("args = {:?}", args);

    let config = fs::read_to_string(args.config)?;
    let mut config: Config = toml::from_str(&config)?;
    trace!("config = {:?}", config);

    // let mut out_writer = args
    //     .out
    //     .map(|path| File::create(path))
    //     .transpose()?
    //     .map(BufWriter::new);

    let (buf_path, buf) = SharedRingBuffer::create_temp(4 * 1024)?;
    let recv = ipmpsc::Receiver::new(buf);

    config
        .main
        .args
        .append(&mut vec!["--buf".to_string(), buf_path]);

    let mut runner = Runner::new(config.main.cmd, config.main.args, config.main.cpu_core)?;

    for (name, cmd) in config.intruders {
        info!("start intruder: {}", name);
        runner.add_intruder_cmd(cmd.cmd, cmd.args, cmd.cpu_core)?;
    }

    let cancel = CancellationToken::new();
    let run_cancel = cancel.clone();

    let run = runner.run(run_cancel, recv, mitigate);

    pin!(run);

    select! {
        _ = &mut run =>{
            info!("finished.");
            Ok(())
        },
        _=  signal::ctrl_c() => {
            info!("ctrl-c");
            cancel.cancel();
            run.await?;

             Ok(())
        },
    }
}

async fn mitigate(intr: watch::Sender<bool>, recv: Receiver) -> Result<()> {
    loop {
        let x: Option<WasmMeasurement> = recv.try_recv()?;
        let Some(x) = x else {
            yield_now().await;
            continue;
        };

        println!("{:?}", x);
    }

    #[allow(unreachable_code)]
    Ok(())
}
