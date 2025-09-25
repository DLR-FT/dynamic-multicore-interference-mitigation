use anyhow::Result;
use clap::Parser;
use hwloc::*;
use tokio::main;
use tokio::process::Command;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg()]
    payload_cmd: String,
    payload_args: Vec<String>,
    payload_core: u32,
}

#[main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut payload = Command::new(args.payload_cmd)
        .args(args.payload_args)
        .spawn()?;

    payload.wait().await?;

    Ok(())
}
