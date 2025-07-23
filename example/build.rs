use std::{cell::LazyCell, env, fs, path::PathBuf};

use quote::quote;

const IS_FEAT_QEMU: LazyCell<bool> = LazyCell::new(|| env::var("CARGO_FEATURE_QEMU").is_ok());
const IS_FEAT_KR260: LazyCell<bool> = LazyCell::new(|| env::var("CARGO_FEATURE_KR260").is_ok());

const OUT_DIR: LazyCell<PathBuf> = LazyCell::new(|| env::var("OUT_DIR").unwrap().into());

pub fn main() {
    let memory_ld = if *IS_FEAT_QEMU {
        quote! {
            __NUM_CPU = 4;
            __STACK_SIZE = 0x10000;
            __TEXT_OFFSET = 0x40000000;
        }
    } else if *IS_FEAT_KR260 {
        quote! {
            __NUM_CPU = 4;
            __STACK_SIZE = 0x10000;
            __TEXT_OFFSET = 0x0;
        }
    } else {
        quote! {}
    };

    fs::write(OUT_DIR.join("memory.ld"), memory_ld.to_string()).unwrap();

    println!("cargo:rustc-link-search={}", OUT_DIR.display());
    println!("cargo:rustc-link-arg=-Tlink.ld");
}
