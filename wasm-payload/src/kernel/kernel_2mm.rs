use alloc::boxed::Box;
use rand::{RngExt, SeedableRng, rngs::SmallRng};

use crate::kernel::array::*;

type T = i8;
const N: usize = 512;

pub struct Kernel2MM {
    rng: SmallRng,

    tmp: Box<Array2D<T, N, N>>,
    a: Box<Array2D<T, N, N>>,
    b: Box<Array2D<T, N, N>>,
    c: Box<Array2D<T, N, N>>,
    d: Box<Array2D<T, N, N>>,
}

impl Kernel2MM {
    pub fn new() -> Self {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(123);

        let tmp = Array2D::<T, N, N>::uninit();
        let mut a = Array2D::<T, N, N>::uninit();
        let mut b = Array2D::<T, N, N>::uninit();
        let mut c = Array2D::<T, N, N>::uninit();
        let mut d = Array2D::<T, N, N>::uninit();

        a.fill_rand(&mut rng);
        b.fill_rand(&mut rng);
        c.fill_rand(&mut rng);
        d.fill_rand(&mut rng);

        Self {
            rng,
            tmp,
            a,
            b,
            c,
            d,
        }
    }

    pub fn run(&mut self) {
        let alpha: T = self.rng.random();
        let beta: T = self.rng.random();

        for i in 0..N {
            for j in 0..N {
                self.tmp[i][j] = T::default();
                for k in 0..N {
                    self.tmp[i][j] = self.tmp[i][j]
                        .wrapping_add(alpha.wrapping_mul(self.a[i][k].wrapping_mul(self.b[k][j])));
                }
            }
        }

        for i in 0..N {
            for l in 0..N {
                self.d[i][l] = T::default();
                for j in 0..N {
                    self.d[i][l] = self.d[i][l]
                        .wrapping_add(beta.wrapping_mul(self.tmp[i][j].wrapping_mul(self.c[j][l])));
                }
            }
        }
    }
}
