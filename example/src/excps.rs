use core::arch::asm;

use arm64::exceptions::*;
use log::trace;

use crate::{INTRUDER_STATE, plat::GIC_DRIVER, spin_utils::SpinMutexExt};
pub struct Excps;

impl Exceptions<ELx_SP_EL0> for Excps {}

impl Exceptions<ELx_SP_ELx> for Excps {
    fn sync_excp(_frame: &mut ExceptionFrame) {
        loop {
            unsafe { asm!("mrs x11, ESR_EL2") }
        }
    }

    fn serror(_frame: &mut ExceptionFrame) {
        loop {}
    }

    fn irq(_frame: &mut ExceptionFrame) {
        let intid = GIC_DRIVER.lock_irq(|gic_lock| {
            let mut gic = gic_lock.borrow_mut();
            gic.get_and_acknowledge_interrupt(arm_gic::InterruptGroup::Group0)
        });

        let Some(intid) = intid else { return };

        let mpidr: u64;
        unsafe { asm!("mrs {r}, MPIDR_EL1", r = lateout(reg) mpidr) }
        let mpidr = (mpidr & 0xf) as usize;

        GIC_DRIVER.lock_irq(|gic_lock| {
            let mut gic = gic_lock.borrow_mut();
            gic.end_interrupt(intid, arm_gic::InterruptGroup::Group0);
        });

        // trace!("stop intruder mpidr: {}", mpidr);

        loop {
            let state = INTRUDER_STATE.load(core::sync::atomic::Ordering::Acquire);
            if state >= mpidr {
                break;
            }
        }

        // trace!("continue intruder mpidr: {}", mpidr);
    }
}

impl Exceptions<ELy_AARCH64> for Excps {}

impl Exceptions<ELy_AARCH32> for Excps {}
