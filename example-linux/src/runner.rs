use std::process::ExitStatus;

use anyhow::{Result, anyhow};
use futures::future::{join, join_all};
use hwloc::CpuSet;
use log::trace;
use procfs::process::Process;
use tokio::process::{Child, Command};

use crate::{cgroup::CGroup, command_ext::CommandExt};

#[derive(Debug)]
pub struct Runner {
    cgroup: CGroup,
    main: Option<Child>,
    intruders: Vec<Child>,
}

impl Runner {
    pub fn new() -> Result<Self> {
        let proc = Process::myself()?;
        let mut c = CGroup::get_current(&proc)?;

        let cgroup = c.create_child("foo123")?;

        Ok(Self {
            cgroup,
            main: None,
            intruders: vec![],
        })
    }

    pub fn start_main(&mut self, cmd: String, args: Vec<String>, cpu_core: u32) -> Result<()> {
        let main = Command::new(&cmd)
            .args(&args)
            .cpu_core(CpuSet::from(cpu_core))
            .spawn()?;

        self.main.replace(main);

        Ok(())
    }

    pub fn start_intruder(&mut self, cmd: String, args: Vec<String>, cpu_core: u32) -> Result<()> {
        let child = Command::new(&cmd)
            .args(&args)
            .cpu_core(CpuSet::from(cpu_core))
            .cgroup(&mut self.cgroup)
            .spawn()?;

        self.intruders.push(child);

        Ok(())
    }

    pub async fn wait(&mut self) -> Result<ExitStatus> {
        trace!("wait .......");
        let Some(child) = self.main.as_mut() else {
            return Err(anyhow!("no main task started"));
        };

        let res = child.wait().await;
        self.main.take();

        join_all((&mut self.intruders).into_iter().map(|child| child.kill())).await;

        trace!(".............. wait done.");

        res.map_err(|err| anyhow!(err))
    }

    pub async fn kill(&mut self) -> Result<()> {
        let Some(child) = self.main.as_mut() else {
            return Err(anyhow!("no main task started dsgghsdhs"));
        };

        let x = child.kill();
        let y = join_all((&mut self.intruders).into_iter().map(|child| child.kill()));

        let (x, _) = join(x, y).await;

        self.main.take();

        x.map_err(|err| anyhow!(err))
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        trace!("-------------------- drop ---------------------");

        if let Some(child) = self.main.as_mut() {
            child.start_kill().unwrap();
        }

        for child in (&mut self.intruders).into_iter() {
            child.start_kill().unwrap();
        }

        self.cgroup.remove().unwrap();
    }
}
