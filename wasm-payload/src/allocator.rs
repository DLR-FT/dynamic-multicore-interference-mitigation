use core::alloc::GlobalAlloc;

#[global_allocator]
pub static ALLOCATOR: Locked<StupidAlloc> = Locked::new(StupidAlloc::new());

pub struct StupidAlloc {
    start_addr: usize,
    end_addr: usize,
}

impl StupidAlloc {
    pub const fn new() -> Self {
        Self {
            start_addr: 0,
            end_addr: 0,
        }
    }

    pub unsafe fn init(&mut self, buf: &[u8]) {
        self.start_addr = (buf as *const _ as *const u8).addr();
        self.end_addr = unsafe { (buf as *const _ as *const u8).add(buf.len()) }.addr()
    }
}

unsafe impl GlobalAlloc for Locked<StupidAlloc> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut bump = self.lock();

        let mut ptr = bump.start_addr as *mut u8;
        let end_ptr = bump.end_addr as *mut u8;

        let align_offset = ptr.align_offset(layout.align());
        ptr = unsafe { ptr.byte_add(align_offset) };

        let next_ptr = unsafe { ptr.byte_add(layout.size()) };

        if (ptr >= end_ptr) || (next_ptr >= end_ptr) {
            panic!("alloc failed: out of memory.")
        }

        bump.start_addr = next_ptr.addr();
        bump.end_addr = end_ptr.addr();

        ptr
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<'_, A> {
        self.inner.lock()
    }
}
