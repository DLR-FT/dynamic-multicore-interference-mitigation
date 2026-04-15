use core::{cell::RefCell, convert::Infallible, fmt::Write};

use embedded_hal_nb::serial::{self};
use log::Log;
use spin::mutex::SpinMutex;

use crate::spin_utils::SpinMutexExt;

pub struct Logger<'a, DRIVER: Send> {
    uart: &'a SpinMutex<RefCell<DRIVER>>,
}

impl<'a, DRIVER> Logger<'a, DRIVER>
where
    DRIVER: Send,
{
    pub fn new(uart: &'a SpinMutex<RefCell<DRIVER>>) -> Self {
        Self { uart }
    }
}

impl<'a, DRIVER> Logger<'a, DRIVER>
where
    DRIVER: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    pub fn write_bytes(&self, buf: &[u8]) {
        self.uart.lock_irq(|uart| {
            let uart = &mut *uart.borrow_mut() as &mut dyn serial::Write<_, Error = Infallible>;
            for b in buf {
                uart.write(*b).unwrap();
            }
        });
    }
}

impl<'a, DRIVER> Log for Logger<'a, DRIVER>
where
    DRIVER: Send + embedded_hal_nb::serial::Write<Error = Infallible>,
{
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        self.uart.lock_irq(|uart| {
            let uart = &mut *uart.borrow_mut() as &mut dyn serial::Write<_, Error = Infallible>;
            writeln!(uart, "[{}] {}", record.level(), record.args()).unwrap();
        });
    }

    fn flush(&self) {
        self.uart.lock_irq(|uart| {
            let mut uart = uart.borrow_mut();
            uart.flush().unwrap();
        });
    }
}
