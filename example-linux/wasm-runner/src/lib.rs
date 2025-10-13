use std::time::Duration;

use serde::{Deserialize, Serialize};

use ipc_serde::{Ipc, Irq};

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct WasmRunnerIpc {
    pub timestamp_unix: Duration,
    pub fuel: Option<usize>,
    pub i: usize,
    pub j: usize,
    pub k: usize,
    pub dt: Duration,
    pub df: Option<usize>,

    pub irq: Option<Irq>,
}

impl Ipc for WasmRunnerIpc {
    fn irq(&self) -> Option<Irq> {
        self.irq
    }
}
