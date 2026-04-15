use core::arch::asm;

use spin::{
    RelaxStrategy,
    mutex::{SpinMutex, SpinMutexGuard},
};

pub trait SpinMutexExt<T: ?Sized> {
    fn lock_irq<X>(&self, f: impl Fn(SpinMutexGuard<'_, T>) -> X) -> X;
}

impl<T: ?Sized, R: RelaxStrategy> SpinMutexExt<T> for SpinMutex<T, R> {
    fn lock_irq<X>(&self, f: impl Fn(SpinMutexGuard<'_, T>) -> X) -> X {
        let daif: u64;
        unsafe { asm!("mrs {daif}, DAIF", "msr DAIFSet, #0xf", daif = lateout(reg) daif) }

        let x;
        {
            let lock = self.lock();
            x = f(lock);
        }

        unsafe { asm!("msr DAIF, {daif}", daif = in(reg) daif) }

        x
    }
}
