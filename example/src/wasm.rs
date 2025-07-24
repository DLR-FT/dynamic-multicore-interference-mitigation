use wasm::{RuntimeInstance, validate};

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

    instance.set_fuel(Some(1));

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
        match state {
            wasm::InvocationState::Finished(ret) => {
                res.replace(ret);
                break;
            }
            wasm::InvocationState::OutOfFuel(mut res) => {
                res.set_fuel(Some(1000));
                state = res.resume().unwrap();
            }
            wasm::InvocationState::Canceled => {
                break;
            }
        };
    }

    return Ok(());
}
