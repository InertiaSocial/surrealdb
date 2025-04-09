use crate::{err::Error, sql::Value};
use wasmtime::component::{Component, Linker, ResourceTable, Val};
use wasmtime::*;
use wasmtime_wasi::bindings::Command;
use wasmtime_wasi::{IoView, WasiCtx, WasiCtxBuilder, WasiView};

pub struct ComponentRunStates {
	// These two are required basically as a standard way to enable the impl of IoView and
	// WasiView.
	// impl of WasiView is required by [`wasmtime_wasi::add_to_linker_sync`]
	pub wasi_ctx: WasiCtx,
	pub resource_table: ResourceTable,
	// You can add other custom host states if needed
}

impl IoView for ComponentRunStates {
	fn table(&mut self) -> &mut ResourceTable {
		&mut self.resource_table
	}
}
impl WasiView for ComponentRunStates {
	fn ctx(&mut self) -> &mut WasiCtx {
		&mut self.wasi_ctx
	}
}

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
pub async fn execute_wasm_function(
	wasm_bytes: &[u8],
	func_name: &str,
	args: &Vec<Value>,
) -> Result<Value, Error> {
	tracing::info!(
		target: "surrealdb::core::wasm::execution",
		"Attempting to execute WASM function '{}'", func_name
	);

	// --- 1. Engine and Store Setup ---
	let mut config = Config::default();
	config.async_support(true);

	let engine = Engine::new(&config).expect("WASM engine failed");
	let mut linker = Linker::new(&engine);
	wasmtime_wasi::add_to_linker_async(&mut linker).expect("Error linking WASI");

	// TODO: chain .allow_udp(VALUE).allow_tcp(VALUE) depending on if allow_net is enabled.
	let mut wasi = WasiCtxBuilder::new();

	let state = ComponentRunStates {
		wasi_ctx: wasi.build(),
		resource_table: ResourceTable::new(),
	};

	let mut store = Store::new(&engine, state);

	let component = Component::from_binary(&engine, wasm_bytes).expect("Unable to load component");
	let instance = linker
		.instantiate_async(&mut store, &component)
		.await
		.expect("Unable to asynchonously instantiate linker on instance");

	let func = instance.get_func(&mut store, func_name).expect("Function not found");

	let param_def = func.params(&mut store);
	let result_def = func.results(&mut store);

	let mut casted_params: Vec<wasmtime::component::Val> = Vec::with_capacity(param_def.len());

	for i in 0..param_def.len().min(args.len()) {
		let arg = args[i].clone().try_into().expect("Failed to convert value");
		casted_params.push(arg);
	}

	let mut results: Vec<wasmtime::component::Val> = Vec::with_capacity(1);

	info!("Arguments to WASM {:?}", casted_params);

	match func.call_async(&mut store, &mut casted_params, &mut results).await {
		Ok(_) => Ok(results[0].clone().try_into().expect("Failed to parse results")),
		Err(err) => {
			error!("WASM root cause: {:?}", err.root_cause());
			Err(err.into())
		}
	}
}

// #[tokio::main]
// async fn main() -> Result<()> {
//     // Construct the wasm engine with async support enabled.
//     let mut config = Config::new();
//     config.async_support(true);
//     let engine = Engine::new(&config)?;
//     let mut linker = Linker::new(&engine);
//     wasmtime_wasi::add_to_linker_async(&mut linker)?;

//     // Create a WASI context and put it in a Store; all instances in the store
//     // share this context. `WasiCtxBuilder` provides a number of ways to
//     // configure what the target program will have access to.
//     let wasi = WasiCtxBuilder::new().inherit_stdio().inherit_args().build();
//     let state = ComponentRunStates {
//         wasi_ctx: wasi,
//         resource_table: ResourceTable::new(),
//     };
//     let mut store = Store::new(&engine, state);

//     // Instantiate our component with the imports we've created, and run it.
//     let component = Component::from_file(&engine, "target/wasm32-wasip2/debug/wasi.wasm")?;
//     let command = Command::instantiate_async(&mut store, &component, &linker).await?;
//     let program_result = command.wasi_cli_run().call_run(&mut store).await?;
//     match program_result {
//         Ok(()) => Ok(()),
//         Err(()) => std::process::exit(1),
//     }
// }
