use core::cell::RefCell;

use arm_gic::gicv2::{
    GicV2,
    registers::{Gicc, Gicd},
};
use spin::{Lazy, mutex::SpinMutex};

pub use sel4_pl011_driver as uart;

const GICD_BASE_ADDRESS: *mut Gicd = 0x0800_0000 as _;
const GICC_BASE_ADDRESS: *mut Gicc = 0x0801_0000 as _;

pub static GIC_DRIVER: Lazy<SpinMutex<RefCell<GicV2>>> = Lazy::new(|| {
    SpinMutex::new(RefCell::new(unsafe {
        GicV2::new(GICD_BASE_ADDRESS, GICC_BASE_ADDRESS)
    }))
});

pub static UART_DRIVER: SpinMutex<RefCell<uart::Driver>> = SpinMutex::new(RefCell::new(unsafe {
    uart::Driver::new_uninit(0x0900_0000 as *mut _)
}));
