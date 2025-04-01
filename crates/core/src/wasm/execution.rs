#![cfg(feature = "wasm")]

use crate::err::Error;
use wasmtime::*;
use wasmtime_wasi::{WasiCtxBuilder, WasiView, WasiP1Ctx};

/// Executes a specific function within a given WASM binary using wasmtime.
///
/// # Arguments
///
/// * `wasm_bytes` - The raw byte vector of the WASM module.
/// * `func_name` - The name of the function to execute within the WASM module.
///
/// # Returns
///
/// * `Ok(())` if the execution was successful (for now, doesn't handle return values).
/// * `Err(Error)` if there was an issue loading, instantiating, or executing the module.
pub async fn execute_wasm_function(wasm_bytes: &[u8], func_name: &str) -> Result<(), Error> {
    tracing::info!(
        target: "surrealdb::core::wasm::execution",
        "Attempting to execute WASM function '{}'", func_name
    );

    // --- 1. Engine and Store Setup ---
    let mut config = Config::default();
    config.async_support(true);
    let engine = Engine::new(&config)?;
    let wasi = WasiP1Ctx::new(WasiCtxBuilder::new().inherit_stdio().build());
    let mut store = Store::new(&engine, wasi);

    // --- 2. Module Compilation & Linking ---
    let module = Module::from_binary(&engine, wasm_bytes)
        .map_err(|e| Error::WasmExecution(format!("Failed to compile WASM module: {}", e)))?;

    let mut linker = Linker::<WasiP1Ctx>::new(&engine);
    wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |ctx: &mut WasiP1Ctx| ctx)
        .map_err(|e| Error::WasmExecution(format!("Failed to link WASI: {}", e)))?;

    // --- 3. Instantiation ---
    let instance = linker
        .instantiate_async(&mut store, &module)
        .await
        .map_err(|e| Error::WasmExecution(format!("Failed to instantiate WASM module: {}", e)))?;

    // --- 4. Function Retrieval & Execution ---
    // First call _start to initialize WASI
    let start = instance
        .get_func(&mut store, "_start")
        .ok_or_else(|| Error::WasmExecution("WASI _start function not found".to_string()))?;
    
    start.call_async(&mut store, &[], &mut [])
        .await
        .map_err(|e| Error::WasmExecution(format!("Failed to call WASI _start: {}", e)))?;

    // Now get and call our target function
    let func = instance
        .get_func(&mut store, func_name)
        .ok_or_else(|| Error::WasmExecution(format!("Function '{}' not found in WASM module", func_name)))?;

    // --- 5. Function Call (No Args/Return Yet) ---
    // TODO: Handle function arguments and return values based on their types.
    // For now, assume a function with no parameters and no return value.
    func.call_async(&mut store, &[], &mut [])
        .await
        .map_err(|e| Error::WasmExecution(format!("Failed to call WASM function '{}': {}", func_name, e)))?;

    tracing::info!(
        target: "surrealdb::core::wasm::execution",
        "Successfully executed WASM function '{}'", func_name
    );

    Ok(())
}

// TODO: Add tests for wasm execution 