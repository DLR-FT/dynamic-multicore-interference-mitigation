use core::{fmt::Write, ops::Sub};

use arm64::pmu::{CounterValue, PMU};
use wasm::{RuntimeInstance, validate};

use crate::uart::UartWriter;

pub fn run_wasm(wasm_bytes: &[u8]) -> Result<(), ()> {
    let validation_info = match validate(&wasm_bytes) {
        Ok(table) => table,
        Err(_err) => {
            return Err(());
        }
    };

    let mut instance = match RuntimeInstance::new(&validation_info) {
        Ok(instance) => instance,
        Err(_err) => {
            return Err(());
        }
    };

    let df = 1000 * 100;

    instance.set_fuel(Some(df));

    PMU::enable();
    PMU::setup_counter(0, arm64::pmu::Event::CPU_CYCLES);
    PMU::setup_counter(1, arm64::pmu::Event::INST_RETIRED);
    PMU::setup_counter(2, arm64::pmu::Event::L1D_CACHE);
    PMU::setup_counter(3, arm64::pmu::Event::L2D_CACHE);

    PMU::reset();
    let mut last = PerfState::get();

    PMU::start();
    let mut state = instance
        .invoke_resumable(
            &instance
                .get_function_by_name(&instance.modules[0].name, "main")
                .unwrap(),
            (0u32, 0u32),
        )
        .unwrap();

    let mut res: Option<i32> = None;
    loop {
        PMU::stop();
        match state {
            wasm::InvocationState::Finished(ret) => {
                res.replace(ret);
                break;
            }
            wasm::InvocationState::OutOfFuel(mut res) => {
                let current = PerfState::get();
                let delta = current - last;

                UartWriter.write_fmt(format_args!("{:?}\n", delta)).unwrap();

                res.set_fuel(Some(df));

                PMU::reset();
                last = PerfState::get();

                PMU::start();
                state = res.resume().unwrap();
            }
            wasm::InvocationState::Canceled => {
                break;
            }
        };
    }

    return Ok(());
}

#[derive(Debug, Clone, Copy)]
struct PerfState {
    cycles: Option<u64>,
    cpu_cycles: Option<u32>,
    inst_retired: Option<u32>,
    l1d_cache: Option<u32>,
    l2d_cache: Option<u32>,
}

impl PerfState {
    fn get() -> Self {
        Self {
            cycles: PMU::get_cycle_counter().into_option(),
            cpu_cycles: PMU::get_counter(0).into_option(),
            inst_retired: PMU::get_counter(1).into_option(),
            l1d_cache: PMU::get_counter(2).into_option(),
            l2d_cache: PMU::get_counter(3).into_option(),
        }
    }
}

impl Sub for PerfState {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            cycles: self.cycles.sub(rhs.cycles),
            cpu_cycles: self.cpu_cycles.sub(rhs.cpu_cycles),
            inst_retired: self.inst_retired.sub(rhs.inst_retired),
            l1d_cache: self.l1d_cache.sub(rhs.l1d_cache),
            l2d_cache: self.l2d_cache.sub(rhs.l2d_cache),
        }
    }
}

trait CounterValueExt<T> {
    fn into_option(self) -> Option<T>;
}

impl<T> CounterValueExt<T> for CounterValue<T> {
    fn into_option(self) -> Option<T> {
        match self {
            CounterValue::Ok(x) => Some(x),
            _ => None,
        }
    }
}

trait OptionExt<T> {
    fn sub(self, rhs: Self) -> Self;
}

impl<T: Sub<Output = T>> OptionExt<T> for Option<T> {
    fn sub(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Some(x), Some(y)) => Some(x - y),
            _ => None,
        }
    }
}
