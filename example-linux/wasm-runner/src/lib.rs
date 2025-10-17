use std::time::Duration;

use serde::{Deserialize, Serialize};

use ipc_serde::{Ipc, Irq};

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct WasmRunnerIpc {
    #[serde(with = "serde_nanos")]
    pub timestamp: Duration,
    pub fuel: Option<usize>,
    pub wctpf: Option<u64>,
    pub i: usize,
    pub j: usize,
    pub k: usize,
    pub l: usize,
    #[serde(with = "serde_nanos")]
    pub dt: Duration,
    pub df: Option<usize>,
    #[serde(with = "serde_nanos")]
    pub acc_t: Duration,
    pub acc_f: Option<usize>,
    pub irq: Option<Irq>,
}

impl Ipc for WasmRunnerIpc {
    fn irq(&self) -> Option<Irq> {
        self.irq
    }
}
