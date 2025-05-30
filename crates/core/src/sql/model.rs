use crate::ctx::Context;
use crate::dbs::Options;
use crate::doc::CursorDoc;
use crate::err::Error;
use crate::sql::value::Value;
use crate::sql::ControlFlow;
use crate::sql::FlowResult;

use reblessive::tree::Stk;
use revision::revisioned;
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "ml")]
use crate::iam::Action;
#[cfg(feature = "ml")]
use crate::sql::Permission;
#[cfg(feature = "ml")]
use futures::future::try_join_all;
#[cfg(feature = "ml")]
use std::collections::HashMap;
#[cfg(feature = "ml")]
use surrealml::errors::error::SurrealError;
#[cfg(feature = "ml")]
use surrealml::execution::compute::ModelComputation;
#[cfg(feature = "ml")]
use surrealml::ndarray as mlNdarray;
#[cfg(feature = "ml")]
use surrealml::storage::surml_file::SurMlFile;

#[cfg(feature = "ml")]
const ARGUMENTS: &str = "The model expects 1 argument. The argument can be either a number, an object, or an array of numbers.";

pub(crate) const TOKEN: &str = "$surrealdb::private::sql::Model";

#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize, Hash)]
#[serde(rename = "$surrealdb::private::sql::Model")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[non_exhaustive]
pub struct Model {
	pub name: String,
	pub version: String,
	pub args: Vec<Value>,
}

impl fmt::Display for Model {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "ml::{}<{}>(", self.name, self.version)?;
		for (idx, p) in self.args.iter().enumerate() {
			if idx != 0 {
				write!(f, ",")?;
			}
			write!(f, "{}", p)?;
		}
		write!(f, ")")
	}
}

impl Model {
	#[cfg(feature = "ml")]
	pub(crate) async fn compute(
		&self,
		stk: &mut Stk,
		ctx: &Context,
		opt: &Options,
		doc: Option<&CursorDoc>,
	) -> FlowResult<Value> {
		use crate::sql::FlowResultExt;

		// Ensure futures are run
		let opt = &opt.new_with_futures(true);
		// Get the full name of this model
		let name = format!("ml::{}", self.name);
		// Check this function is allowed
		ctx.check_allowed_function(name.as_str())?;
		// Get the model definition
		let (ns, db) = opt.ns_db()?;
		let val = ctx.tx().get_db_model(ns, db, &self.name, &self.version).await?;
		// Calculate the model path
		let (ns, db) = opt.ns_db()?;
		let path = format!("ml/{}/{}/{}-{}-{}.surml", ns, db, self.name, self.version, val.hash);
		// Check permissions
		if opt.check_perms(Action::View)? {
			match &val.permissions {
				Permission::Full => (),
				Permission::None => {
					return Err(ControlFlow::from(Error::FunctionPermissions {
						name: self.name.clone(),
					}))
				}
				Permission::Specific(e) => {
					// Disable permissions
					let opt = &opt.new_with_perms(false);
					// Process the PERMISSION clause
					if !stk.run(|stk| e.compute(stk, ctx, opt, doc)).await?.is_truthy() {
						return Err(ControlFlow::from(Error::FunctionPermissions {
							name: self.name.clone(),
						}));
					}
				}
			}
		}
		// Compute the function arguments
		let mut args = stk
			.scope(|stk| {
				try_join_all(self.args.iter().map(|v| {
					stk.run(|stk| async { v.compute(stk, ctx, opt, doc).await.catch_return() })
				}))
			})
			.await?;
		// Check the minimum argument length
		if args.len() != 1 {
			return Err(ControlFlow::from(Error::InvalidArguments {
				name: format!("ml::{}<{}>", self.name, self.version),
				message: ARGUMENTS.into(),
			}));
		}
		// Take the first and only specified argument
		match args.swap_remove(0) {
			// Perform bufferered compute
			Value::Object(v) => {
				// Compute the model function arguments
				let mut args = v
					.into_iter()
					.map(|(k, v)| Ok((k, v.coerce_to::<f64>()? as f32)))
					.collect::<Result<HashMap<String, f32>, Error>>()
					.map_err(|_| Error::InvalidArguments {
						name: format!("ml::{}<{}>", self.name, self.version),
						message: ARGUMENTS.into(),
					})?;
				// Get the model file as bytes
				let bytes = crate::obs::get(&path).await?;
				// Run the compute in a blocking task
				let outcome: Vec<f32> = tokio::task::spawn_blocking(move || {
					let mut file = SurMlFile::from_bytes(bytes).map_err(|err: SurrealError| {
						Error::ModelComputation(err.message.to_string())
					})?;
					let compute_unit = ModelComputation {
						surml_file: &mut file,
					};
					compute_unit.buffered_compute(&mut args).map_err(|err: SurrealError| {
						Error::ModelComputation(err.message.to_string())
					})
				})
				.await
				.unwrap()
				.map_err(ControlFlow::from)?;
				// Convert the output to a value
				Ok(outcome.into())
			}
			// Perform raw compute
			Value::Number(v) => {
				// Compute the model function arguments
				let args: f32 = v.try_into().map_err(|_| Error::InvalidArguments {
					name: format!("ml::{}<{}>", self.name, self.version),
					message: ARGUMENTS.into(),
				})?;
				// Get the model file as bytes
				let bytes = crate::obs::get(&path).await?;
				// Convert the argument to a tensor
				let tensor = mlNdarray::arr1::<f32>(&[args]).into_dyn();
				// Run the compute in a blocking task
				let outcome: Vec<f32> = tokio::task::spawn_blocking(move || {
					let mut file = SurMlFile::from_bytes(bytes).map_err(|err: SurrealError| {
						Error::ModelComputation(err.message.to_string())
					})?;
					let compute_unit = ModelComputation {
						surml_file: &mut file,
					};
					compute_unit.raw_compute(tensor, None).map_err(|err: SurrealError| {
						Error::ModelComputation(err.message.to_string())
					})
				})
				.await
				.unwrap()
				.map_err(ControlFlow::from)?;
				// Convert the output to a value
				Ok(outcome.into())
			}
			// Perform raw compute
			Value::Array(v) => {
				// Compute the model function arguments
				let args = v
					.into_iter()
					.map(|x| x.coerce_to::<f64>().map(|x| x as f32).map_err(Error::from))
					.collect::<Result<Vec<f32>, Error>>()
					.map_err(|_| Error::InvalidArguments {
						name: format!("ml::{}<{}>", self.name, self.version),
						message: ARGUMENTS.into(),
					})
					.map_err(ControlFlow::from)?;
				// Get the model file as bytes
				let bytes = crate::obs::get(&path).await?;
				// Convert the argument to a tensor
				let tensor = mlNdarray::arr1::<f32>(&args).into_dyn();
				// Run the compute in a blocking task
				let outcome: Vec<f32> = tokio::task::spawn_blocking(move || {
					let mut file = SurMlFile::from_bytes(bytes).map_err(|err: SurrealError| {
						Error::ModelComputation(err.message.to_string())
					})?;
					let compute_unit = ModelComputation {
						surml_file: &mut file,
					};
					compute_unit.raw_compute(tensor, None).map_err(|err: SurrealError| {
						Error::ModelComputation(err.message.to_string())
					})
				})
				.await
				.unwrap()
				.map_err(ControlFlow::from)?;
				// Convert the output to a value
				Ok(outcome.into())
			}
			//
			_ => Err(ControlFlow::from(Error::InvalidArguments {
				name: format!("ml::{}<{}>", self.name, self.version),
				message: ARGUMENTS.into(),
			})),
		}
	}

	#[cfg(not(feature = "ml"))]
	pub(crate) async fn compute(
		&self,
		_stk: &mut Stk,
		_ctx: &Context,
		_opt: &Options,
		_doc: Option<&CursorDoc>,
	) -> FlowResult<Value> {
		Err(ControlFlow::from(Error::InvalidModel {
			message: String::from("Machine learning computation is not enabled."),
		}))
	}
}
