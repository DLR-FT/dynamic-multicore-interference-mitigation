use core::cell::RefCell;

use spin::mutex::SpinMutex;

pub use sel4_zynqmp_xuartps_driver as uart;

pub static UART_DRIVER: SpinMutex<RefCell<uart::Driver>> = SpinMutex::new(RefCell::new(unsafe {
    uart::Driver::new_uninit(0x00FF000000 as *mut _)
}));
