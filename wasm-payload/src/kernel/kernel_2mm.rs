use rand::SeedableRng;

use crate::kernel::array::*;

type T = i64;

pub fn run<const NI: usize, const NJ: usize, const NK: usize, const NL: usize>() {
    let ni = NI;
    let nj = NJ;
    let nk = NK;
    let nl = NL;

    let mut rng = rand::rngs::SmallRng::seed_from_u64(123);

    let alpha = -1;
    let beta = 2;
    let mut tmp = Array2D::<T, NI, NJ>::uninit();
    let mut a = Array2D::<T, NI, NK>::uninit();
    let mut b = Array2D::<T, NK, NJ>::uninit();
    let mut c = Array2D::<T, NJ, NL>::uninit();
    let mut d = Array2D::<T, NI, NL>::uninit();

    a.fill_rand(&mut rng);
    b.fill_rand(&mut rng);
    c.fill_rand(&mut rng);
    a.fill_rand(&mut rng);

    kernel(ni, nj, nk, nl, alpha, beta, &mut tmp, &a, &b, &c, &mut d);
    consume(d);
}

fn kernel<const NI: usize, const NJ: usize, const NK: usize, const NL: usize>(
    ni: usize,
    nj: usize,
    nk: usize,
    nl: usize,
    alpha: T,
    beta: T,
    tmp: &mut Array2D<T, NI, NJ>,
    a: &Array2D<T, NI, NK>,
    b: &Array2D<T, NK, NJ>,
    c: &Array2D<T, NJ, NL>,
    d: &mut Array2D<T, NI, NL>,
) {
    for i in 0..ni {
        for j in 0..nj {
            tmp[i][j] = T::default();
            for k in 0..nk {
                tmp[i][j] =
                    tmp[i][j].wrapping_add(alpha.wrapping_mul(a[i][k]).wrapping_mul(b[k][j]));
            }
        }
    }
    for i in 0..ni {
        for j in 0..nl {
            d[i][j] *= beta;
            for k in 0..nj {
                d[i][j] = d[i][j].wrapping_add(tmp[i][k].wrapping_mul(c[k][j]));
            }
        }
    }
}
