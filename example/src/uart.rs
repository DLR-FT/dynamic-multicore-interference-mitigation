use core::fmt::Write as FmtWrite;

use arm64::critical_section;
use embedded_hal_nb::serial::Write;

use crate::plat::UART_DRIVER;

pub struct UartWriter;

impl FmtWrite for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        critical_section::with(|cs| {
            let mut driver = UART_DRIVER.borrow_ref_mut(cs);
            for c in s.chars() {
                driver.write(c as u8).unwrap()
            }
        });

        Ok(())
    }
}
