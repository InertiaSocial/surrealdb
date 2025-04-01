use crate::ctx::Context;
use crate::dbs::Options;
use crate::doc::CursorDoc;
use crate::err::Error;
use crate::sql::value::Value;
use crate::sql::ControlFlow;
use crate::sql::FlowResult;
use crate::sql::Ident;

use reblessive::tree::Stk;
use revision::revisioned;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::info;

#[cfg(feature = "wasm")]
use crate::iam::Action;
#[cfg(feature = "wasm")]
use crate::iam::ResourceKind;
#[cfg(feature = "wasm")]
use crate::sql::Permission;

#[cfg(feature = "wasm")]
const ARGUMENTS: &str = "The WASM module function can take various argument types, depending on the function exported.";

pub(crate) const TOKEN: &str = "$surrealdb::private::sql::Wasm";

#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize, Hash)]
#[serde(rename = "$surrealdb::private::sql::Wasm")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[non_exhaustive]
pub struct Wasm {
    pub name: Ident,
    pub version: String,
    pub func: Ident,
    pub args: Vec<Value>,
}

impl fmt::Display for Wasm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "wasm::{}<{}>::{}(", self.name, self.version, self.func)?;
        for (idx, p) in self.args.iter().enumerate() {
            if idx != 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", p)?;
        }
        write!(f, ")")
    }
}

impl Wasm {
    #[cfg(feature = "wasm")]
    pub(crate) async fn compute(
        &self,
        _stk: &mut Stk,
        ctx: &Context,
        opt: &Options,
        _doc: Option<&CursorDoc>,
    ) -> FlowResult<Value> {
        // Ensure futures are run
		let opt = &opt.new_with_futures(true);

        // Get the full name of this function (for permission checking)
		let _name = format!("wasm::{}", self.name); // TODO: Check if this is right for permissions

		// Check this function is allowed (Is this check sufficient? Need ResourceKind::Wasm?)
		// ctx.check_allowed_function(name.as_str())?; // TODO: Revisit permission check - maybe check Wasm resource kind instead?

        // Check permissions (explicit Wasm check)
		opt.is_allowed(Action::View, ResourceKind::Wasm, &crate::sql::Base::Db)?;

        // Get the wasm definition
        let (ns, db) = opt.ns_db()?;
		let def = ctx.tx().get_db_wasm(ns, db, &self.name, &self.version).await?;

        // Calculate the wasm path
        let path = format!("wasm/{}/{}/{}-{}-{}.wasm", ns, db, self.name, self.version, def.hash);

        // Get the wasm bytes
        let bytes = crate::obs::get(&path).await?;

        // TODO: Implement argument/return value handling
        // For now, just call the function with no args/returns
        crate::wasm::execution::execute_wasm_function(&bytes, &self.func.to_string())
            .map_err(ControlFlow::from)?;

        Ok(Value::None) // Return None for now
    }

    #[cfg(not(feature = "wasm"))]
    #[allow(clippy::unused_async)] // Keep signature consistent
    pub(crate) async fn compute(
        &self,
        _stk: &mut Stk,
        _ctx: &Context,
        _opt: &Options,
        _doc: Option<&CursorDoc>,
    ) -> FlowResult<Value> {
        use crate::sql::ControlFlow;
        Err(ControlFlow::from(Error::WasmFeatureNotEnabled))
    }
} 