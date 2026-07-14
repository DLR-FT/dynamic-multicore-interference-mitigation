use core::ops::{BitOr, Shl};

use arm64::pmu::CounterValue;

pub trait CounterValueExt {
    type T;
    fn ok(self) -> Option<Self::T>;
    fn chain<U>(self, upper: Self) -> CounterValue<U>
    where
        Self::T: Into<U>,
        U: Shl<usize, Output = U>,
        U: BitOr<Output = U>;
}

impl<T> CounterValueExt for CounterValue<T> {
    type T = T;

    fn ok(self) -> Option<Self::T> {
        match self {
            CounterValue::Ok(x) => Some(x),
            CounterValue::Overflowed(_) => None,
        }
    }

    fn chain<U>(self, upper: Self) -> CounterValue<U>
    where
        T: Into<U>,
        U: Shl<usize, Output = U>,
        U: BitOr<Output = U>,
    {
        let upper = match upper {
            CounterValue::Overflowed(cnt) => return CounterValue::Overflowed(cnt.into()),
            CounterValue::Ok(cnt) => cnt.into(),
        };

        let lower = match self {
            CounterValue::Overflowed(cnt) => cnt.into(),
            CounterValue::Ok(cnt) => cnt.into(),
        };

        CounterValue::Ok((upper << 32) | lower)
    }
}
