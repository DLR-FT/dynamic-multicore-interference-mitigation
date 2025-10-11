use anyhow::{Ok, Result};
use futures::future::join_all;
use hwloc::CpuSet;
use log::trace;
use procfs::process::Process;
use tokio::{io, join, process::Command, select, spawn, sync::watch};
use tokio_util::sync::CancellationToken;

use crate::{cgroup::CGroup, command_ext::CommandExt};

#[derive(Debug)]
pub struct Runner {
    main_cmd: Command,

    intruder_cgroup: CGroup,
    intruder_cmds: Vec<Command>,
}

impl Runner {
    pub fn new(cmd: String, args: Vec<String>, cpu_core: u32) -> Result<Self> {
        let proc = Process::myself()?;
        let mut c = CGroup::get_current(&proc)?;

        let intruder_cgroup = c.create_child("foo123", true)?;

        let mut cmd = Command::new(&cmd);
        cmd.args(&args)
            .cpu_core(CpuSet::from(cpu_core))?
            .process_group(0)
            .kill_on_drop(true);

        Ok(Self {
            main_cmd: cmd,
            intruder_cgroup,
            intruder_cmds: vec![],
        })
    }

    pub fn add_intruder_cmd(
        &mut self,
        cmd: String,
        args: Vec<String>,
        cpu_core: u32,
    ) -> Result<()> {
        let mut cmd = Command::new(&cmd);
        cmd.args(&args)
            .cgroup(&mut self.intruder_cgroup)?
            .cpu_core(CpuSet::from(cpu_core))?
            .kill_on_drop(true);

        self.intruder_cmds.push(cmd);

        Ok(())
    }

    pub async fn run<F: Future, T>(
        mut self,
        cancel: CancellationToken,
        extra: T,
        f: impl Fn(watch::Sender<bool>, T) -> F,
    ) -> Result<()> {
        let mut main_task = self.main_cmd.spawn()?;
        let mut intruder_tasks = self
            .intruder_cmds
            .into_iter()
            .map(|mut cmd| cmd.spawn())
            .collect::<io::Result<Vec<_>>>()?;

        let mut intruders = self.intruder_cgroup;
        let (tx, mut rx) = watch::channel(true);

        select! {
            _ = main_task.wait() => {},
            _ = f(tx, extra) => {},
            _ = spawn(async move {
                loop {
                    if rx.changed().await.is_err() {break;}
                    match *rx.borrow_and_update() {
                        true => { trace!("received freeze interrupt"); intruders.freeze()? },
                        false => { trace!("received unfreeze interrupt"); intruders.unfreeze()? }
                    };
                }

                Ok(())
            }) => {},
            _ = cancel.cancelled() => { trace!("runner cancelled") },
        };

        trace!("killing intruder tasks");
        let kill_intruders = intruder_tasks.iter_mut().map(|child| child.kill());

        trace!("killing main task");
        join!(main_task.kill(), join_all(kill_intruders)).0?;

        Ok(())
    }
}
