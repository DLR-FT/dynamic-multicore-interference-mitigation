use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Irq {
    Freeze(usize),
    Unfreeze(usize),
}

pub trait Ipc: Copy + Serialize + for<'a> Deserialize<'a> + Send + Sync {
    fn irq(&self) -> Option<Irq>;
}
