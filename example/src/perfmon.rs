use core::ops::{BitOr, Shl};

use analyzer::PerfInfo;
use arm64::pmu::{self, CounterValue, PMU};

pub struct PerfMon;

impl PerfMon {
    pub fn setup() {
        PMU::enable();
        PMU::reset();

        PMU::setup_counter(0, pmu::Event::INST_RETIRED);
        PMU::setup_counter(1, pmu::Event::CHAIN);

        PMU::setup_counter(2, pmu::Event::L1D_CACHE);
        PMU::setup_counter(3, pmu::Event::L2D_CACHE);

        PMU::setup_counter(4, pmu::Event::BUS_CYCLES);
        PMU::setup_counter(5, pmu::Event::BUS_ACCESS);
    }

    pub fn start() {
        PMU::reset();
        PMU::start();
    }

    pub fn stop() -> PerfInfo {
        PMU::stop();

        PerfInfo {
            cycles: PMU::get_cycle_counter().ok(),

            instr: PMU::get_counter(0).chain(PMU::get_counter(1)).ok(),
            l1d_access: PMU::get_counter(2).ok(),
            l2d_access: PMU::get_counter(3).ok(),
            bus_cycles: PMU::get_counter(4).ok(),
            bus_access: PMU::get_counter(5).ok(),
        }
    }
}

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
