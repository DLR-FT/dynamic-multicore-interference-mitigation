use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct WasmMeasurement {
    pub timestamp_unix: Duration,
    pub i: u32,
    pub dt: Duration,
    pub df: usize,
}
