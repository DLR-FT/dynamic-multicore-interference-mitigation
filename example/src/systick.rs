use core::arch::asm;

use arm64::sys_regs::{CNTFRQ_EL0, CNTPCT_EL0};

pub struct SysTick;

impl SysTick {
    pub fn get_cnt() -> u64 {
        CNTPCT_EL0.read().CNT()
    }

    pub fn get_freq() -> u32 {
        CNTFRQ_EL0.read().FREQ()
    }

    pub fn get_time_us() -> u64 {
        Self::get_cnt() / (Self::get_freq() as u64 / 1000000)
    }

    pub fn wait_us(us: u64) {
        let ticks = us * (Self::get_freq() as u64 / 1000000);
        let end = Self::get_cnt() + ticks;
        loop {
            if Self::get_cnt() >= end {
                break;
            }

            for _i in 0..100 {
                unsafe { asm!("nop") }
            }
        }
    }
}
