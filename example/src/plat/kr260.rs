use core::cell::RefCell;

use arm64::critical_section::Mutex;

pub use sel4_zynqmp_xuartps_driver as uart;

pub static UART_DRIVER: Mutex<RefCell<uart::Driver>> = Mutex::new(RefCell::new(unsafe {
    uart::Driver::new_uninit(0x00FF010000 as *mut _)
}));
