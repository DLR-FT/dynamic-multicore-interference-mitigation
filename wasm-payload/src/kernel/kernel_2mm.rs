use crate::kernel::utils::*;

type T = f64;

pub fn run<const NI: usize, const NJ: usize, const NK: usize, const NL: usize>() {
    let ni = NI;
    let nj = NJ;
    let nk = NK;
    let nl = NL;

    let alpha = 1.5;
    let beta = 0.75;
    let mut tmp = Array2D::<T, NI, NJ>::uninit();
    let a = Array2D::<T, NI, NK>::uninit();
    let b = Array2D::<T, NK, NJ>::uninit();
    let c = Array2D::<T, NJ, NL>::uninit();
    let mut d = Array2D::<T, NI, NL>::uninit();

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
                tmp[i][j] += alpha * a[i][k] * b[k][j];
            }
        }
    }
    for i in 0..ni {
        for j in 0..nl {
            d[i][j] *= beta;
            for k in 0..nj {
                d[i][j] += tmp[i][k] * c[k][j];
            }
        }
    }
}
