use analyzer::RefuelUpdate;
use embedded_io::Write;
use wasm_payload::kernel::Kernel2MM;

use crate::{
    intruder::{self, INTRUDER_BREAK},
    perfmon::PerfMon,
    systick::SysTick,
};

pub struct NativeRunner {
    kernel: Kernel2MM,
    run_idx: usize,
}

impl<'log> NativeRunner {
    pub fn new() -> Self {
        Self {
            kernel: Kernel2MM::new(),
            run_idx: 0,
        }
    }

    pub fn run(&mut self, mut writer: impl Write) {
        PerfMon::start();

        let last = SysTick::get_time_us();
        self.kernel.run();

        let current = SysTick::get_time_us();
        let perf = PerfMon::stop();

        let dt = current - last;

        let update = RefuelUpdate {
            timestamp: current,
            fuel: None,
            refuel_idx: 0,
            run_idx: self.run_idx,
            intruder_break: INTRUDER_BREAK.load(core::sync::atomic::Ordering::Acquire),
            intruder_set_mask: unsafe { intruder::SET_MASK },
            dt,
            df: None,
            acc_t: dt,
            acc_f: None,
            perf_info: Some(perf),
        };

        let buf = &mut [0u8; 1024];
        let n = serde_json_core::to_slice(&update, &mut buf[..]).unwrap();
        let _ = writer.write(&buf[..n]);

        self.run_idx += 1;
    }
}
