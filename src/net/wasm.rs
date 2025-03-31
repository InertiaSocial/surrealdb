//! This file defines the endpoints for the WASM API for importing and exporting WebAssembly modules.
use super::AppState;
use crate::cnf::HTTP_MAX_ML_BODY_SIZE;
use crate::err::Error;
#[cfg(feature = "wasm")]
use crate::net::output;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path};
use axum::response::IntoResponse;
#[cfg(feature = "wasm")]
use axum::response::Response;
use axum::routing::{get, post};
use axum::Extension;
use axum::Router;
#[cfg(feature = "wasm")]
use bytes::Bytes;
#[cfg(feature = "wasm")]
use futures_util::StreamExt;
use http::HeaderMap;
use http::StatusCode;
use surrealdb::dbs::capabilities::RouteTarget;
use surrealdb::dbs::Session;
#[cfg(feature = "wasm")]
use surrealdb::iam::check::check_ns_db;
#[cfg(feature = "wasm")]
use surrealdb::iam::Action::{Edit, View};
#[cfg(feature = "wasm")]
use surrealdb::iam::ResourceKind;
#[cfg(feature = "wasm")]
use surrealdb_core::kvs::Datastore as CoreDatastore;
#[cfg(feature = "wasm")]
use surrealdb_core::obs; // Import obs module from surrealdb_core
#[cfg(feature = "wasm")]
use surrealdb_core::sql::statements::DefineStatement;
#[cfg(feature = "wasm")]
use surrealdb_core::sql::statements::DefineWasmStatement;
#[cfg(feature = "wasm")]
use surrealdb_core::sql::{Ident, Idiom}; // Import SQL types separately
use tracing::{info, warn};
use tower_http::limit::RequestBodyLimitLayer;
#[cfg(not(feature = "wasm"))]
use chrono::Utc;
#[cfg(feature = "wasm")]
use chrono::Utc;

/// The router definition for the WASM API endpoints.
pub(super) fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    // TODO: Add HTTP_MAX_WASM_BODY_SIZE to cnf.rs in future PR
    let wasm_body_size = std::env::var("HTTP_MAX_WASM_BODY_SIZE")
        .unwrap_or_else(|_| HTTP_MAX_ML_BODY_SIZE.to_string())
        .parse::<usize>()
        .unwrap_or(*HTTP_MAX_ML_BODY_SIZE);

    #[cfg(feature = "wasm")]
    let router = Router::new()
        .route("/wasm/import", post(import))
        .route("/wasm/export/:name/:version", get(export))
        .route_layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(wasm_body_size));
    
    #[cfg(not(feature = "wasm"))]
    let router = Router::new()
        .route("/wasm/import", post(import_unavailable))
        .route("/wasm/export/:name/:version", get(export_unavailable))
        .route_layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(wasm_body_size));

    router
}

/// This endpoint allows the user to import a WASM module into the database.
#[cfg(feature = "wasm")]
async fn import(
    Extension(state): Extension<AppState>,
    Extension(session): Extension<Session>,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, impl IntoResponse> {
    let mut stream = body.into_data_stream();
    let db = &state.datastore;
    let wasm_route_target = RouteTarget::Wasm;

    if !db.allows_http_route(&wasm_route_target) {
        warn!("Capabilities denied HTTP route request attempt, target: '{}'", wasm_route_target);
        return Err(Error::ForbiddenRoute(wasm_route_target.to_string()));
    }

    let (nsv, dbv) = check_ns_db(&session)?;
    db.check(&session, Edit, ResourceKind::Wasm.on_db(&nsv, &dbv))?;

    // Collect all chunks into a buffer
    let mut buffer = Vec::new();
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => buffer.extend_from_slice(&chunk),
            Err(e) => {
                warn!("Error reading request body: {}", e);
                return Err(Error::Other(format!("Failed to read request body: {}", e)));
            }
        }
    }
    
    // Check for empty uploads
    if buffer.is_empty() {
        return Err(Error::Other("Empty WASM module uploaded".to_string()));
    }

    // Validate WASM binary format
    if !is_valid_wasm_module(&buffer) {
        warn!("Invalid WebAssembly module format");
        return Err(Error::Other("Invalid WebAssembly module format".to_string()));
    }

    // Extract metadata from headers
    let name_str = match headers.get("wasm-name").and_then(|h| h.to_str().ok()).map(String::from) {
        Some(name) if !name.is_empty() => name,
        _ => {
            // Generate default name using timestamp
            let timestamp = Utc::now().timestamp();
            format!("wasm_{}", timestamp)
        }
    };

    let version = match headers.get("wasm-version").and_then(|h| h.to_str().ok()).map(String::from) {
        Some(ver) if !ver.is_empty() => ver,
        _ => "1.0.0".to_string(),
    };

    let comment_idiom = headers.get("wasm-description")
        .and_then(|h| h.to_str().ok())
        .map(|s| Idiom::from(s.to_string()));

    // Calculate the hash of the WASM binary
    let hash = match obs::hash(&buffer) {
        hash => {
            info!("Calculated hash for WASM module '{}': {}", name_str, hash);
            hash
        }
    };
    
    // Calculate the path for storing the binary
    let path = format!("wasm/{nsv}/{dbv}/{}-{}-{hash}.wasm", name_str, version);
    info!("Storing WASM module at path: {}", path);
    
    // Store the WASM binary in the object store
    if let Err(e) = obs::put(&path, buffer).await {
        warn!("Failed to store WASM binary: {}", e);
        return Err(Error::Other(format!("Failed to store WASM binary: {}", e)));
    }
    
    info!("Successfully stored WASM binary, creating database definition");

    // Create the WASM definition
    let mut wasm_def = DefineWasmStatement::default();
    wasm_def.name = Ident::from(name_str.clone());
    wasm_def.version = version.clone();
    wasm_def.hash = hash.clone();
    wasm_def.comment = comment_idiom;

    // Process the definition statement
    match db.process(DefineStatement::Wasm(wasm_def).into(), &session, None).await {
        Ok(_) => {
            info!("WASM module '{}' version '{}' (hash: {}) stored and defined successfully", name_str, version, hash);
            Ok(output::none())
        }
        Err(e) => {
            warn!("Failed to define WASM module '{}' after storing: {}. Cleaning up...", name_str, e);
            // Clean up the stored binary if definition fails
            if let Err(cleanup_err) = obs::del(&path).await {
                warn!("Failed to clean up WASM binary after definition failure: {}", cleanup_err);
            } else {
                info!("Successfully cleaned up WASM binary after definition failure");
            }
            Err(Error::from(e))
        }
    }
}

/// This endpoint allows the user to export a WASM module from the database.
#[cfg(feature = "wasm")]
async fn export(
    Extension(state): Extension<AppState>,
    Extension(session): Extension<Session>,
    Path((name, version)): Path<(String, String)>,
) -> Result<impl IntoResponse, Error> {
    // Get the datastore reference
    let db = &state.datastore;
    
    // Check if capabilities allow querying the requested HTTP route
    let wasm_route_target = RouteTarget::Wasm;
    if !db.allows_http_route(&wasm_route_target) {
        warn!("Capabilities denied HTTP route request attempt, target: '{}'", wasm_route_target);
        return Err(Error::ForbiddenRoute(wasm_route_target.to_string()));
    }

    // Ensure a NS and DB are set
    let (nsv, dbv) = check_ns_db(&session)?;
    
    // Check the permissions level
    db.check(&session, View, ResourceKind::Wasm.on_db(&nsv, &dbv))?;
    
    // Start a read-only transaction (similar to ML export)
    use surrealdb::kvs::{LockType::Optimistic, TransactionType::Read};
    let tx = db.transaction(Read, Optimistic).await?;
    
    // Get the WASM definition
    let wasm_def = match tx.get_db_wasm(&nsv, &dbv, &name, &version).await {
        Ok(def) => def,
        Err(e) => {
            // If the error is WasmNotFound, provide a more user-friendly message
            if let surrealdb::err::Error::WasmNotFound { .. } = e {
                return Err(Error::Other(format!("WASM module '{}' version '{}' not found", name, version)));
            }
            return Err(Error::from(e));
        }
    };
    
    // Calculate the path of the WASM file
    let path = format!("wasm/{nsv}/{dbv}/{name}-{version}-{}.wasm", wasm_def.hash);
    
    // Get the binary data
    match obs::get(&path).await {
        Ok(data) => {
            info!("Retrieved WASM module '{}' version '{}' (hash: {})", name, version, wasm_def.hash);
            
            // Build a simple response
            let response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/wasm")
                .header("Content-Disposition", format!("attachment; filename=\"{}-{}.wasm\"", name, version))
                .body(Body::from(data))
                .map_err(|e| Error::Other(format!("Failed to build response: {}", e)))?;
            
            Ok(response)
        }
        Err(e) => {
            Err(Error::Other(format!("Failed to retrieve WASM module binary: {}", e)))
        }
    }
}

/// Basic validation for WebAssembly modules
/// Checks for the magic bytes at the beginning of the file
fn is_valid_wasm_module(data: &[u8]) -> bool {
    // WASM binary format starts with these magic bytes: \0asm
    if data.len() < 8 {
        return false;
    }
    let magic = [0x00, 0x61, 0x73, 0x6D]; // \0asm
    let version_bytes = [0x01, 0x00, 0x00, 0x00]; // Version 1
    data.starts_with(&magic) && data[4..8] == version_bytes
}

// Add handlers for when wasm feature is disabled
#[cfg(not(feature = "wasm"))]
async fn import_unavailable(
    Extension(state): Extension<AppState>,
    _: Extension<Session>,
    _: HeaderMap,
    _: Body,
) -> Result<(), impl IntoResponse> {
    // Get the datastore reference
    let db = &state.datastore;
    // Check if capabilities allow querying the requested HTTP route
    if !db.allows_http_route(&RouteTarget::Wasm) {
        warn!("Capabilities denied HTTP route request attempt, target: '{}'", &RouteTarget::Wasm);
        return Err(Error::ForbiddenRoute(RouteTarget::Wasm.to_string()));
    }
    Err(Error::Request)
}

#[cfg(not(feature = "wasm"))]
async fn export_unavailable(
    Extension(state): Extension<AppState>,
    _: Extension<Session>,
    Path((_, _)): Path<(String, String)>,
) -> Result<(), impl IntoResponse> {
    // Get the datastore reference
    let db = &state.datastore;
    // Check if capabilities allow querying the requested HTTP route
    if !db.allows_http_route(&RouteTarget::Wasm) {
        warn!("Capabilities denied HTTP route request attempt, target: '{}'", &RouteTarget::Wasm);
        return Err(Error::ForbiddenRoute(RouteTarget::Wasm.to_string()));
    }
    Err(Error::Request)
} 