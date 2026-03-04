use core::ops::{Index, IndexMut};

use rand::{Fill, RngExt};

pub fn consume<T>(dummy: T) -> T {
    unsafe {
        // Taken from bencher crate:
        // https://docs.rs/bencher/0.1.5/src/bencher/lib.rs.html#590-596
        let ret = core::ptr::read_volatile(&dummy);
        core::mem::forget(dummy);
        ret
    }
}

#[repr(C, align(32))]
pub struct Array1D<T, const M: usize>(pub [T; M]);

#[repr(C, align(32))]
pub struct Array2D<T, const M: usize, const N: usize>(pub [Array1D<T, N>; M]);

#[repr(C, align(32))]
pub struct Array3D<T, const M: usize, const N: usize, const P: usize>(pub [Array2D<T, N, P>; M]);

impl<T: Fill, const M: usize> Array1D<T, M> {
    pub fn fill_rand(&mut self, rng: &mut impl RngExt) {
        rng.fill(&mut self.0);
    }
}

impl<T: Fill, const M: usize, const N: usize> Array2D<T, M, N> {
    pub fn fill_rand(&mut self, rng: &mut impl RngExt) {
        for x in &mut self.0 {
            x.fill_rand(rng);
        }
    }
}

impl<T: Fill, const M: usize, const N: usize, const P: usize> Array3D<T, M, N, P> {
    pub fn fill_rand(&mut self, rng: &mut impl RngExt) {
        for x in &mut self.0 {
            for y in &mut x.0 {
                y.fill_rand(rng);
            }
        }
    }
}

impl<T, const M: usize> Index<usize> for Array1D<T, M> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < M);
        unsafe { self.0.get_unchecked(index) }
    }
}

impl<T, const M: usize> IndexMut<usize> for Array1D<T, M> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < M);
        unsafe { self.0.get_unchecked_mut(index) }
    }
}

impl<T, const M: usize, const N: usize> Index<usize> for Array2D<T, M, N> {
    type Output = Array1D<T, N>;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < M);
        unsafe { self.0.get_unchecked(index) }
    }
}

impl<T, const M: usize, const N: usize> IndexMut<usize> for Array2D<T, M, N> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < M);
        unsafe { self.0.get_unchecked_mut(index) }
    }
}

impl<T, const M: usize, const N: usize, const P: usize> Index<usize> for Array3D<T, M, N, P> {
    type Output = Array2D<T, N, P>;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        debug_assert!(index < M);
        unsafe { self.0.get_unchecked(index) }
    }
}

impl<T, const M: usize, const N: usize, const P: usize> IndexMut<usize> for Array3D<T, M, N, P> {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        debug_assert!(index < M);
        unsafe { self.0.get_unchecked_mut(index) }
    }
}

pub trait ArrayAlloc: Sized {
    fn uninit() -> alloc::boxed::Box<Self> {
        let layout = core::alloc::Layout::new::<Self>();
        unsafe {
            let raw = alloc::alloc::alloc(layout) as *mut Self;
            alloc::boxed::Box::from_raw(raw)
        }
    }
}

impl<T, const N: usize> ArrayAlloc for Array1D<T, N> {}
impl<T, const M: usize, const N: usize> ArrayAlloc for Array2D<T, M, N> {}
impl<T, const M: usize, const N: usize, const P: usize> ArrayAlloc for Array3D<T, M, N, P> {}
