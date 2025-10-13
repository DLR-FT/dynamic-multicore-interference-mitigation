use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Irq {
    Freeze(usize),
    Unfreeze(usize),
}

pub trait Ipc: Serialize + for<'a> Deserialize<'a> {
    fn irq(&self) -> Option<Irq>;
}
