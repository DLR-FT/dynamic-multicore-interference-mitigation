use core::cell::RefCell;

use spin::mutex::SpinMutex;

pub use sel4_pl011_driver as uart;

pub static UART_DRIVER: SpinMutex<RefCell<uart::Driver>> = SpinMutex::new(RefCell::new(unsafe {
    uart::Driver::new_uninit(0x09000000 as *mut _)
}));
