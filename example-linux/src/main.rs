use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::*;
use clap::Parser;
use hwloc::*;
use ipmpsc::SharedRingBuffer;
use itertools::Itertools;
use log::*;
use nix::sys::statfs::*;
use procfs::process::*;
use serde::*;
use tokio::{process::*, time, *};
use tokio_util::sync::*;
use walkdir::WalkDir;
use wasm_runner_serde::*;

#[derive(Parser, Debug, Clone)]
struct Args {
    #[arg(long)]
    config: PathBuf,

    #[arg(long)]
    out: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    main: ChildCmd,
    cmds: HashMap<String, ChildCmd>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChildCmd {
    cmd: String,
    args: Vec<String>,
    cpu_core: u32,
}

#[main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::init();
    let cancellation = CancellationToken::new();

    let args = Args::parse();
    trace!("{:?}", args);

    let config = fs::read_to_string(args.config)?;
    let config: Config = toml::from_str(&config)?;
    trace!("{:?}", config);

    let proc = Process::myself()?;
    let mut c = CGroup::get_current(&proc)?;

    info!("PID: {},  CGroup: {:?}", proc.pid(), c.path);

    let mut child = c.create_child("foo123")?;

    let (buf_path, buf) = SharedRingBuffer::create_temp(4 * 1024)?;
    let recv = ipmpsc::Receiver::new(buf);

    let out_file = File::create_new(args.out)?;
    let mut out_writer = BufWriter::new(out_file);

    info!("Create main task");
    let mut main_task = Command::new(&config.main.cmd)
        .arg("--buf")
        .arg(buf_path)
        .args(&config.main.args)
        .cpu_core(CpuSet::from(config.main.cpu_core))
        .spawn()?;

    let spawn_task = |child_cmd: (&String, &ChildCmd)| {
        trace!("Spawn task: {}", child_cmd.0);
        Command::new(&child_cmd.1.cmd)
            .args(&child_cmd.1.args)
            .cgroup(&mut child)
            .cpu_core(CpuSet::from(child_cmd.1.cpu_core))
            .spawn()
    };

    let _tasks: Vec<Child> = config
        .cmds
        .iter()
        .map(spawn_task)
        .collect::<io::Result<Vec<Child>>>()?;

    trace!("CGroup pids: {:?}", child.get_pids()?);

    let mut child1 = child.clone();
    let mut child2 = child.clone();
    select! {
        _ = signal::ctrl_c() => {
            info!("Ctrl-C ....");
        },
         _ = tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_millis(5000)).await;
                child1.unfreeze()?;

                // let f = child1.is_frozen()?;
                // match f {
                //     true => child1.unfreeze()?,
                //     false => child1.freeze()?,
                // }
            }

            Ok(())
        }) => {},

        _ = tokio::spawn(async move {
            loop {
                for _ in 0..999 {
                    let x: Option<WasmMeasurement> = recv.try_recv()?;
                    match x {
                        Some(m) => {

                            if m.dt > Duration::from_micros(1000) {
                                child2.freeze()?;
                            }

                            // info!("{:?}", m);
                            out_writer.write_fmt(format_args!("{}, {}, {}, {}\n", m.timestamp_unix.as_nanos(), m.i, m.dt.as_nanos(), m.df))?;
                            continue
                        },
                        None => {break}
                    }
                }

                out_writer.flush()?;
                time::sleep(Duration::from_millis(1)).await;
            }

            Ok(())
        }) => {}
    }

    trace!("Cancel");
    cancellation.cancel();
    // task1.await??;s

    trace!("Kill main task");
    main_task.kill().await?;

    // w.flush()?;

    info!("Remove child group ...");
    child.remove()?;

    Ok(())
}

#[derive(Clone)]
pub struct CGroup {
    path: PathBuf,
}

impl CGroup {
    fn get_current(proc: &Process) -> Result<Self> {
        let mount = proc
            .mountinfo()?
            .into_iter()
            .find(|mount| mount.fs_type == "cgroup2")
            .with_context(|| format!("CGroup mount-point not found for proc: {}", proc.pid()))
            .map(|x| x.mount_point)?;

        let cgroup = proc
            .cgroups()?
            .into_iter()
            .find(|c| c.hierarchy == 0)
            .with_context(|| format!("CGroup not found for PID: {}", proc.pid()))?;

        let path = mount.join(cgroup.pathname.strip_prefix('/').unwrap());

        trace!("Get CGroup: {:?}", path);
        Ok(Self { path })
    }

    fn create_child(&mut self, name: &str) -> Result<Self> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let path = self.path.join(name);

        trace!("Create child CGroup: {:?}", path);
        fs::create_dir(&path)?;

        Ok(Self { path })
    }

    fn exists(&self) -> bool {
        let std::result::Result::Ok(stat) = statfs(&self.path) else {
            return false;
        };

        stat.filesystem_type() == CGROUP2_SUPER_MAGIC
    }

    fn is_populated(&self) -> Result<bool> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let events_path = self.path.join("cgroup.events");
        let events = fs::read_to_string(events_path)?;
        Ok(events.contains("populated 1\n"))
    }

    fn is_frozen(&self) -> Result<bool> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let freeze_path = self.path.join("cgroup.freeze");
        ensure!(
            freeze_path.exists(),
            format!(
                "cgroup.freeze file not found, cannot freeze CGroup: {:?}",
                self.path
            )
        );

        Ok(fs::read(&freeze_path)? == b"1\n")
    }

    fn get_pids(&self) -> Result<Vec<i32>> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let procs_path = self.path.join("cgroup.procs");
        ensure!(
            procs_path.exists(),
            format!(
                "cgroup.procs file not found, cannot get PIDs from CGroup: {:?}",
                self.path
            )
        );

        trace!("Reading PIDs from CGroup: {:?}", self.path);
        Ok(fs::read_to_string(procs_path)?
            .lines()
            .map(|line| line.parse().unwrap())
            .collect())
    }

    fn mv_proc(&mut self, proc: &Process) -> Result<()> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let procs_path = self.path.join("cgroup.procs");
        ensure!(
            procs_path.exists(),
            format!(
                "cgroup.procs file not found, cannot move process {} into CGroup: {:?}",
                proc.pid(),
                self.path
            )
        );

        trace!("Move proc {} to CGroup: {:?}", proc.pid(), self.path);
        fs::write(procs_path, proc.pid().to_string())?;

        Ok(())
    }

    fn freeze(&mut self) -> Result<()> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let freeze_path = self.path.join("cgroup.freeze");
        ensure!(
            freeze_path.exists(),
            format!(
                "cgroup.freeze file not found, cannot freeze CGroup: {:?}",
                self.path
            )
        );

        trace!("Freeze CGroup: {:?}", self.path);
        fs::write(freeze_path, "1")?;

        Ok(())
    }

    fn unfreeze(&mut self) -> Result<()> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let freeze_path = self.path.join("cgroup.freeze");
        ensure!(
            freeze_path.exists(),
            format!(
                "cgroup.freeze file not found, cannot freeze CGroup: {:?}",
                self.path
            )
        );

        trace!("Unfreeze CGroup: {:?}", self.path);
        fs::write(freeze_path, "0")?;

        Ok(())
    }

    fn kill(&mut self) -> Result<()> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let kill_path = self.path.join("cgroup.kill");
        ensure!(
            kill_path.exists(),
            format!(
                "cgroup.kill file not found, cannot kill CGroup: {:?}",
                self.path
            )
        );

        trace!("Kill CGroup: {:?}", self.path);
        fs::write(kill_path, "1")?;

        let start = Instant::now();
        let timeout = Duration::from_secs(1);
        while start.elapsed() < timeout {
            if !self.is_populated()? {
                return Ok(());
            }
        }

        bail!("Timeout while killing CGroup: {:?}", self.path)
    }

    fn remove(mut self) -> Result<()> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        self.kill()?;

        trace!("Remove CGroup: {:?}", self.path);
        for d in WalkDir::new(&self.path)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_dir())
            .sorted_by(|a, b| a.depth().cmp(&b.depth()).reverse())
        {
            fs::remove_dir(d.path())?;
        }

        Ok(())
    }
}

trait CommandExt {
    fn cgroup(&mut self, cgroup: &mut CGroup) -> &mut Self;
    fn cpu_core(&mut self, cpuset: CpuSet) -> &mut Self;
}

impl CommandExt for Command {
    fn cgroup(&mut self, cgroup: &mut CGroup) -> &mut Self {
        unsafe {
            let mut cgroup = cgroup.clone();
            self.pre_exec(move || {
                let proc = Process::myself().unwrap();
                cgroup.mv_proc(&proc).unwrap();

                io::Result::Ok(())
            })
        }
    }

    fn cpu_core(&mut self, cpuset: CpuSet) -> &mut Self {
        let proc = Process::myself().unwrap();
        let mut topo = Topology::new();

        trace!("Binding proc: {} to CPU core: {:?}", proc.pid(), cpuset);
        topo.set_cpubind(cpuset, CPUBIND_PROCESS).unwrap();

        trace!(
            "Proc: {} bound to CPU core: {:?}",
            proc.pid(),
            topo.get_cpubind(CPUBIND_PROCESS)
        );

        self
    }
}
