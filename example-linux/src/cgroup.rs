use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use itertools::Itertools;
use log::trace;
use nix::{
    self,
    sys::statfs::{CGROUP2_SUPER_MAGIC, statfs},
};
use procfs::process::Process;

use anyhow::{Context, Result, bail, ensure};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct CGroup {
    pub(crate) path: PathBuf,
}

impl CGroup {
    pub fn get_current(proc: &Process) -> Result<Self> {
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

    pub fn create_child(&mut self, name: &str) -> Result<Self> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let path = self.path.join(name);

        trace!("Create child CGroup: {:?}", path);
        fs::create_dir(&path)?;

        Ok(Self { path })
    }

    pub fn exists(&self) -> bool {
        let std::result::Result::Ok(stat) = statfs(&self.path) else {
            return false;
        };

        stat.filesystem_type() == CGROUP2_SUPER_MAGIC
    }

    pub fn is_populated(&self) -> Result<bool> {
        ensure!(
            self.exists(),
            format!("CGroup does not exist: {:?}", self.path)
        );

        let events_path = self.path.join("cgroup.events");
        let events = fs::read_to_string(events_path)?;
        Ok(events.contains("populated 1\n"))
    }

    pub fn is_frozen(&self) -> Result<bool> {
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

    pub fn get_pids(&self) -> Result<Vec<i32>> {
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

    pub fn mv_proc(&mut self, proc: &Process) -> Result<()> {
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

    pub fn freeze(&mut self) -> Result<()> {
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

    pub fn unfreeze(&mut self) -> Result<()> {
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

    pub fn kill(&mut self) -> Result<()> {
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

    pub fn remove(&mut self) -> Result<()> {
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
