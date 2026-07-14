use core::{cell::RefCell, convert::Infallible};

use embedded_hal_nb::serial;
use spin::mutex::SpinMutex;

use crate::spin_utils::*;

pub trait BufWrite {
    fn write_bytes(&self, buf: &[u8]);
}

impl<'a, T> BufWrite for SpinMutex<RefCell<T>>
where
    T: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    fn write_bytes(&self, buf: &[u8]) {
        self.lock_irq(|uart| {
            let uart = &mut *uart.borrow_mut() as &mut dyn serial::Write<_, Error = Infallible>;
            for b in buf {
                uart.write(*b).unwrap();
            }
        })
    }
}
