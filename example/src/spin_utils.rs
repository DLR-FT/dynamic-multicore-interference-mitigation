use spin::{
    RelaxStrategy,
    mutex::{SpinMutex, SpinMutexGuard},
};

pub trait SpinMutexExt<T: ?Sized> {
    fn lock_irq<X>(&self, f: impl Fn(SpinMutexGuard<'_, T>) -> X) -> X;
}

impl<T: ?Sized, R: RelaxStrategy> SpinMutexExt<T> for SpinMutex<T, R> {
    fn lock_irq<X>(&self, f: impl Fn(SpinMutexGuard<'_, T>) -> X) -> X {
        arm_gic::irq_disable();

        let x;
        {
            let lock = self.lock();
            x = f(lock);
        }

        arm_gic::irq_enable();

        x
    }
}
