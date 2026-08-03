use core::{cell::RefCell, convert::Infallible};

use arm64::stm::{Stm, StmType};
use embedded_io::{ErrorType, Write};
use spin::mutex::SpinMutex;

use crate::spin_utils::SpinMutexExt;

pub struct StmWriter<'a, 'stm> {
    port: u16,
    stm: &'a SpinMutex<RefCell<Stm<'stm>>>,
}

impl<'a, 'stm> StmWriter<'a, 'stm> {
    pub fn new(port: u16, stm: &'a SpinMutex<RefCell<Stm<'stm>>>) -> Self {
        Self { port, stm }
    }
}

impl<'a, 'stm> ErrorType for StmWriter<'a, 'stm> {
    type Error = Infallible;
}

impl<'a, 'stm> Write for StmWriter<'a, 'stm> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.len() == 0 {
            return Ok(0);
        }

        self.stm.lock_irq(|stm| {
            let mut stm = stm.borrow_mut();

            stm.write_u8(self.port, StmType::G_DTS, buf[0]);
            for b in buf[1..].iter() {
                stm.write_u8(self.port, StmType::G_D, *b);
            }

            stm.write_u8(self.port, StmType::G_FLAG, 0);
        });

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
