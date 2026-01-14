use std::{cell::LazyCell, env, fs, path::PathBuf};

use anyhow::Result;
use bytemuck::checked::cast_slice;
use quote::quote;

type DataType = f64;
const BUF_SIZE: usize = 0x0010_0000;

const OUT_DIR: LazyCell<PathBuf> =
    LazyCell::new(|| env::var("OUT_DIR").expect("env: OUT_DIR").into());

pub fn main() -> Result<()> {
    let mut buf = [0f64; BUF_SIZE / size_of::<DataType>()];
    rand::fill(&mut buf[..]);

    let buf = cast_slice(&buf[..]);

    let buf_path = OUT_DIR.join("buf.bin");
    fs::write(buf_path, buf)?;

    let buf_rs = quote! {
        pub const BUF_SIZE: usize = #BUF_SIZE;
        pub static BUF: &[u8] = include_bytes!("buf.bin");
    }
    .to_string();

    let buf_rs_path = OUT_DIR.join("buf.rs");
    fs::write(buf_rs_path, buf_rs)?;

    Ok(())
}
