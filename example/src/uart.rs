use core::fmt::Write as FmtWrite;

use embedded_hal_nb::serial::Write;

use crate::plat::UART_DRIVER;

pub struct UartWriter;

impl FmtWrite for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let temp = UART_DRIVER.lock();
        let mut driver = temp.borrow_mut();

        for c in s.chars() {
            driver.write(c as u8).unwrap()
        }

        Ok(())
    }
}
