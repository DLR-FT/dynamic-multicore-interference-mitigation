use std::{collections::HashMap, fmt::Debug, path::PathBuf, time::Duration};

use anyhow::{Context, Ok, Result};
use futures::future::join_all;
use hwloc::CpuSet;
use ipc_serde::{Ipc, Irq};
use ipmpsc::{Receiver, SharedRingBuffer};
use procfs::process::Process;
use serde::Deserialize;
use tokio::{
    io,
    process::{Child, Command},
    select, spawn,
    sync::{mpsc, oneshot, watch},
    task::yield_now,
    time::sleep,
};
use tokio_util::sync::CancellationToken;

use crate::{cgroup::CGroup, command_ext::CommandExt};

pub struct Runner {
    root_cgroup: CGroup,

    cmds: HashMap<(usize, usize), Command>,
    cgroups: HashMap<usize, CGroup>,

    ipc_buf: SharedRingBuffer,
    ipc_path: PathBuf,
}

impl Runner {
    pub fn new() -> Result<Self> {
        let proc = Process::myself()?;
        let root_cgroup = CGroup::get_current(&proc)?;

        let (ipc_path, ipc_buf) = SharedRingBuffer::create_temp(4 * 1024)?;

        Ok(Self {
            root_cgroup,
            cmds: HashMap::new(),
            cgroups: HashMap::new(),
            ipc_buf,
            ipc_path: ipc_path.into(),
        })
    }

    pub fn add_process(
        &mut self,
        id: (usize, usize),
        cmd: String,
        args: Vec<String>,
        cpu_core: u32,
        ipc_arg: Option<String>,
    ) -> Result<()> {
        let cgroup = if let Some(cgroup) = self.cgroups.get_mut(&id.0) {
            cgroup
        } else {
            let cgroup = self
                .root_cgroup
                .create_child(&format!("runner{}", id.0), true)?;

            self.cgroups.insert(id.0, cgroup);
            self.cgroups.get_mut(&id.0).unwrap()
        };

        let mut cmd = Command::new(&cmd);
        cmd.args(args)
            .cgroup(cgroup)?
            .cpu_core(CpuSet::from(cpu_core))?
            .kill_on_drop(true);

        if let Some(ipc_arg) = ipc_arg {
            cmd.args([ipc_arg, self.ipc_path.to_str().unwrap().to_owned()]);
        }

        self.cmds.insert(id, cmd);

        Ok(())
    }

    pub async fn run<T: Ipc + Debug + 'static>(
        mut self,
        primary_id: (usize, usize),
        tx: mpsc::Sender<T>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let recv = Receiver::new(self.ipc_buf);

        let mut tasks = self
            .cmds
            .iter_mut()
            .map(|(id, cmd)| cmd.spawn().map(|x| (*id, x)))
            .collect::<io::Result<HashMap<(usize, usize), Child>>>()?;

        let bar = CancellationToken::new();
        let baz = bar.clone();
        let foo = spawn(async move {
            loop {
                let x: Option<T> = recv.try_recv()?;
                let Some(x) = x else {
                    if baz.is_cancelled() {
                        break;
                    }

                    yield_now().await;
                    continue;
                };

                match x.irq() {
                    Some(Irq::Freeze(id)) => {
                        self.cgroups
                            .get_mut(&id)
                            .map(|cgroup| {
                                if !cgroup.is_frozen()? {
                                    cgroup.freeze()
                                } else {
                                    Ok(())
                                }
                            })
                            .transpose()?;
                    }
                    Some(Irq::Unfreeze(id)) => {
                        self.cgroups
                            .get_mut(&id)
                            .map(|cgroup| {
                                if cgroup.is_frozen()? {
                                    cgroup.unfreeze()
                                } else {
                                    Ok(())
                                }
                            })
                            .transpose()?;
                    }
                    None => {}
                }

                tx.send(x).await?;
                // println!("{:?}", x)
            }

            Ok(())
        });

        select! {
            _ = cancel.cancelled() => {},
            _ = tasks.get_mut(&primary_id).with_context(|| format!("id {:?} does not exist", primary_id))?.wait() => {},
        }

        let _ = join_all(tasks.iter_mut().map(|(_, task)| task.kill()))
            .await
            .into_iter()
            .collect::<io::Result<Vec<_>>>()?;

        bar.cancel();
        _ = foo.await;

        Ok(())
    }
}
