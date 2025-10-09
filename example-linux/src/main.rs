use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufWriter,
    path::PathBuf,
};

use anyhow::*;
use clap::Parser;
use ipmpsc::SharedRingBuffer;
use log::*;
use serde::*;
use tokio::*;

mod cgroup;
mod command_ext;
mod runner;

use runner::*;

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
    let config: Config = toml::from_str(&config)?;
    trace!("config = {:?}", config);

    // let mut out_writer = args
    //     .out
    //     .map(|path| File::create(path))
    //     .transpose()?
    //     .map(BufWriter::new);

    let mut runner = Runner::new()?;

    // let (buf_path, buf) = SharedRingBuffer::create_temp(4 * 1024)?;
    // let recv = ipmpsc::Receiver::new(buf);

    info!("start main");
    runner.start_main(config.main.cmd, config.main.args, config.main.cpu_core)?;

    for (name, cmd) in config.intruders {
        info!("start intruder: {}", name);
        runner.start_intruder(cmd.cmd, cmd.args, cmd.cpu_core)?;
    }

    select! {
        _ = signal::ctrl_c() => {
            info!("ctrl-c ....");

            runner.kill().await?;
        },

        res = runner.wait() => {
            info!("finished = {:?}", res)
        },

        // _ = tokio::spawn(async move {
        //     loop {
        //         for _ in 0..999 {
        //             let x: Option<WasmMeasurement> = recv.try_recv()?;
        //             match x {
        //                 Some(m) => {

        //                     let x = serde_json::to_string(&m)?;
        //                     match &mut out_writer {
        //                         Some(writer) => { writer.write_fmt(format_args!("{}\n", x))?;  },
        //                         None => { println!("{}", x) }
        //                     }

        //                     continue;
        //                 },
        //                 None => break,
        //             }
        //         }

        //         time::sleep(Duration::from_millis(1)).await;
        //     }

        //     #[allow(unreachable_code)]
        //     Ok(())
        // }) => {}
    }

    Ok(())
}
