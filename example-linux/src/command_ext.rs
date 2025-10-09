use hwloc::{CPUBIND_PROCESS, CpuSet, Topology};
use log::trace;
use procfs::process::Process;
use tokio::{io, process::Command};

use crate::cgroup::CGroup;

pub trait CommandExt {
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
