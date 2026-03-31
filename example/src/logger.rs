// pub struct UartWriter;

// impl UartWriter {
//     pub fn write_bytes(buf: &[u8]) -> Result<(), ()> {
//         let temp = UART_DRIVER.lock();
//         let mut driver = temp.borrow_mut();

//         for b in buf {
//             driver.write(*b).map_err(|_e| ())?;
//         }

//         Ok(())
//     }
// }

// impl FmtWrite for UartWriter {
//     fn write_str(&mut self, s: &str) -> core::fmt::Result {
//         let temp = UART_DRIVER.lock();
//         let mut driver = temp.borrow_mut();

//         for c in s.chars() {
//             driver.write(c as u8).map_err(|_e| core::fmt::Error)?;
//         }

//         Ok(())
//     }
// }

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
