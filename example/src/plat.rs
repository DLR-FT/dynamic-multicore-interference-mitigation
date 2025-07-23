#[cfg(feature = "qemu")]
mod qemu;

#[cfg(feature = "qemu")]
pub use qemu::*;

#[cfg(feature = "kr260")]
mod kr260;

#[cfg(feature = "kr260")]
pub use kr260::*;
