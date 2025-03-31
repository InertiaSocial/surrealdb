use crate::sql::{Duration, Ident, Idiom};
use crate::sql::Permission;
use crate::ctx::Context;
use crate::dbs::Options;
use crate::doc::CursorDoc;
use crate::err::Error;
use crate::iam::{Action, ResourceKind};
use crate::sql::value::Value;
use crate::sql::Base;
use revision::revisioned;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

/// Defines a database WASM module.
///
/// ```surql
/// DEFINE WASM example VERSION "1.0.0" HASH "abc..." COMMENT "Example WASM module";
/// ```
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Serialize, Deserialize, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct DefineWasmStatement {
    /// The name of the module
    pub name: Ident,
    /// The version of the module
    pub version: String,
    /// The hash of the module's binary content
    pub hash: String,
    /// Define only if the module does not already exist
    pub if_not_exists: bool,
    /// The comment for the module
    pub comment: Option<Idiom>,
    /// Permissions for the module
    pub perms: Permission,
    /// Timeout duration (currently unused, mirroring model)
    pub timeout: Option<Duration>,
}

impl Display for DefineWasmStatement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DEFINE WASM")?;
        if self.if_not_exists {
            write!(f, " IF NOT EXISTS")?;
        }
        write!(f, " {}", self.name)?;
        if !self.version.is_empty() {
            write!(f, " VERSION \"{}\"", self.version)?;
        }
        if !self.hash.is_empty() {
            write!(f, " HASH \"{}\"", self.hash)?;
        }
        if let Some(comment) = &self.comment {
            write!(f, " COMMENT {}", comment)?;
        }
        if self.perms != Permission::None {
            write!(f, " PERMISSIONS {}", self.perms)?
        }
        if let Some(timeout) = &self.timeout {
            write!(f, " TIMEOUT {}", timeout)?
        }
        Ok(())
    }
}

impl DefineWasmStatement {
    pub(crate) async fn compute(
        &self,
        ctx: &Context,
        opt: &Options,
        _doc: Option<&CursorDoc>,
    ) -> Result<Value, Error> {
        // Check permissions
        opt.is_allowed(Action::Edit, ResourceKind::Wasm, &Base::Db)?;

        // Get transaction
        let txn = ctx.tx();
        let (ns, db) = opt.ns_db()?;

        // Generate key
        let key = crate::key::database::wasm::new(ns, db, &self.name, &self.version);

        // Check if WASM module already exists if needed
        if !self.if_not_exists {
            if txn.exists(&key, None).await? {
                return Err(Error::WasmAlreadyExists {
                    name: format!("{}({})", self.name, self.version),
                });
            }
        }

        // Serialize the statement for storage
        let bytes = revision::to_vec(self)?;

        // Persist the definition using PUT (insert if not exists)
        txn.put(key, bytes, None).await?;

        // Clear relevant cache entries (optional, but good practice)
        // Specific entries can be removed if needed, but clearing all for simplicity here.
        txn.clear();

        Ok(Value::Null)
    }
}
