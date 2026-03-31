use core::cell::RefCell;

use arm_gic::gicv2::{
    GicV2,
    registers::{Gicc, Gicd},
};
pub use sel4_zynqmp_xuartps_driver as uart;
use spin::{Lazy, mutex::SpinMutex};

pub const GICD_BASE_ADDRESS: *mut Gicd = 0xF901_0000u64 as _;
pub const GICC_BASE_ADDRESS: *mut Gicc = 0xF902_0000u64 as _;

pub static GIC_DRIVER: Lazy<SpinMutex<RefCell<GicV2>>> = Lazy::new(|| {
    SpinMutex::new(RefCell::new(unsafe {
        GicV2::new(GICD_BASE_ADDRESS, GICC_BASE_ADDRESS)
    }))
});

pub static UART_DRIVER: SpinMutex<RefCell<uart::Driver>> = SpinMutex::new(RefCell::new(unsafe {
    uart::Driver::new_uninit(0xFF00_0000 as *mut _)
}));
