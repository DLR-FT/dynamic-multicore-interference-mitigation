#![no_std]

use core::{alloc::GlobalAlloc, mem::MaybeUninit};

pub struct SimpleAlloc {
    inner: spin::mutex::SpinMutex<RawSimpleAlloc>,
}

impl SimpleAlloc {
    pub const fn new() -> Self {
        Self {
            inner: spin::mutex::SpinMutex::new(RawSimpleAlloc::new()),
        }
    }

    pub unsafe fn init(&self, buf: &[MaybeUninit<u8>]) {
        unsafe { self.inner.lock().init(buf) };
    }
}

unsafe impl GlobalAlloc for SimpleAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut raw = self.inner.lock();
        unsafe { raw.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let mut raw = self.inner.lock();
        unsafe { raw.dealloc(ptr, layout) }
    }
}

struct RawSimpleAlloc {
    start_addr: usize,
    end_addr: usize,
}

impl RawSimpleAlloc {
    const fn new() -> Self {
        Self {
            start_addr: 0,
            end_addr: 0,
        }
    }

    unsafe fn init(&mut self, buf: &[MaybeUninit<u8>]) {
        self.start_addr = (buf as *const _ as *const u8).addr();
        self.end_addr = unsafe { (buf as *const _ as *const u8).add(buf.len()) }.addr();
    }

    unsafe fn alloc(&mut self, layout: core::alloc::Layout) -> *mut u8 {
        let mut ptr = self.start_addr as *mut u8;
        let end_ptr = self.end_addr as *mut u8;

        let align_offset = ptr.align_offset(layout.align());
        ptr = unsafe { ptr.byte_add(align_offset) };

        let next_ptr = unsafe { ptr.byte_add(layout.size()) };

        if (ptr >= end_ptr) || (next_ptr >= end_ptr) {
            panic!("alloc failed: out of memory.")
        }

        self.start_addr = next_ptr.addr();
        self.end_addr = end_ptr.addr();

        ptr
    }

    unsafe fn dealloc(&mut self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}
