use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct WasmMeasurement {
    pub timestamp_unix: Duration,
    pub i: usize,
    pub j: usize,
    pub dt: Duration,
    pub df: Option<usize>,
}
