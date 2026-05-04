#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct RefuelUpdate {
    pub timestamp: u64,
    pub fuel: Option<u32>,
    pub run_idx: usize,
    pub refuel_idx: usize,
    pub intruder_state: usize,
    pub dt: u64,
    pub df: Option<u32>,
    pub acc_t: u64,
    pub acc_f: Option<u32>,

    pub pmu_info: Option<PMUInfo>,
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct PMUInfo {
    pub cycles: Option<u64>,
    pub instr: Option<u64>,

    pub l1d_access: Option<u32>,
    pub l1d_wb: Option<u32>,
    pub l1d_refill: Option<u32>,
    pub l2d_access: Option<u32>,
    pub l2d_wb: Option<u32>,
    pub l2d_refill: Option<u32>,
}
