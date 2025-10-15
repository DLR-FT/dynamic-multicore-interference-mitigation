use serde::{Deserialize, Serialize};

use ipc_serde::{Ipc, Irq};

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct WasmRunnerIpc {
    pub timestamp_unix: u128,
    pub fuel: usize,
    pub i: usize,
    pub j: usize,
    pub k: usize,
    pub dt: u64,
    pub df: usize,
    // pub ma_tpf: u64,
    pub irq: Option<Irq>,
}

impl Ipc for WasmRunnerIpc {
    fn irq(&self) -> Option<Irq> {
        self.irq
    }
}
