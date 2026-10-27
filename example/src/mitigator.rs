use analyzer::RefuelUpdate;

use crate::intruder::{intruder_break, intruder_cont};

pub struct Mitigator {
    total_fuel: u64,
    deadline: u64,
    hyst: u64,
}

impl Mitigator {
    pub fn new(total_fuel: u64, deadline: u64, hyst: u64) -> Self {
        Self {
            total_fuel,
            deadline,
            hyst,
        }
    }

    pub fn mitigate(&self, update: &RefuelUpdate) {
        let (Some(_), Some(acc_f)) = (update.fuel, update.acc_f) else {
            return;
        };

        let avgtpf_ns = update.acc_t * 1000 / acc_f;
        let est_t = update.acc_t + ((avgtpf_ns * (self.total_fuel - acc_f)) / 1000);

        if est_t > (self.deadline - self.hyst) {
            intruder_break();
        } else {
            intruder_cont();
        }
    }
}
