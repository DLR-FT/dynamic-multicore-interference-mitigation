use core::{cell::RefCell, convert::Infallible};

use embedded_io::{ErrorType, Write};
use spin::mutex::SpinMutex;

use crate::spin_utils::SpinMutexExt;

pub struct UartWriter<'a, DRIVER>
where
    DRIVER: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    uart: &'a SpinMutex<RefCell<DRIVER>>,
}

impl<'a, DRIVER> UartWriter<'a, DRIVER>
where
    DRIVER: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    pub fn new(uart: &'a SpinMutex<RefCell<DRIVER>>) -> Self {
        Self { uart }
    }
}

impl<'a, DRIVER: Send> ErrorType for UartWriter<'a, DRIVER>
where
    DRIVER: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    type Error = Infallible;
}

impl<'a, DRIVER> Write for UartWriter<'a, DRIVER>
where
    DRIVER: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.len() == 0 {
            return Ok(0);
        }

        self.uart.lock_irq(|uart| {
            let mut uart = uart.borrow_mut();

            for b in buf[0..].iter() {
                uart.write(*b);
            }

            Ok(buf.len())
        })
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.uart.lock_irq(|uart| {
            let mut uart = uart.borrow_mut();
            uart.flush();
            Ok(())
        })
    }
}
