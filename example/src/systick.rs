use core::{arch::asm, time::Duration};

pub struct SysTick;

impl SysTick {
    pub fn get_cnt() -> u64 {
        arm64::sysreg_read!("CNTPCT_EL0")
    }

    pub fn get_freq() -> u32 {
        arm64::sysreg_read!("CNTFRQ_EL0")
    }

    pub fn get_time_us() -> u64 {
        Self::get_cnt() / (Self::get_freq() / 1000000) as u64
    }

    pub fn wait_us(us: u64) {
        let start = Self::get_time_us();
        let end = start + us;
        loop {
            if Self::get_time_us() > end {
                break;
            }

            for _ in 0..1000 {
                unsafe {
                    core::arch::asm!("nop");
                }
            }
        }
    }
}
