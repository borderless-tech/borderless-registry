mod db;
mod error;
mod extractor;
mod migrator;
mod models;

use crate::error::Error;
use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::put,
    Json, Router,
};
use borderless_pkg::WasmPkg;
use clap::Parser;
use db::entities::{index::ActiveIndex, package::ActivePackage};
use extractor::OciId;
use sea_orm::{
    entity::prelude::*,
    ActiveValue::{NotSet, Set},
};
use sea_orm::{DatabaseConnection, TransactionTrait};
use tracing::{info, instrument};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Path to the database directory (global)
    #[arg(short, long)]
    db: String,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub db: DatabaseConnection,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Start Registry Server!");

    let args = Cli::parse();
    let db = db::setup_database(&args.db).await?;

    let app = AppState { db };

    let app = Router::new()
        .route("/api/v0/publish/{*oci}", put(publish))
        .with_state(app);

    info!("Start API Service");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

// PUT publish wasm package in registry
#[instrument]
pub async fn publish(
    State(state): State<AppState>,
    OciId(oid): OciId,
    Json(pkg): Json<WasmPkg>,
) -> Result<StatusCode, Error> {
    info!("Trigger route!");
    info!("Hey oci {:?}", oid);
    let txn = state.db.begin().await?;
    // add pkg to database
    let pkg_model = ActivePackage::from_model(&txn, pkg).await?;

    // and registry index
    let index_oci = oid.clone();
    let idx_entry = ActiveIndex {
        id: NotSet,
        pkg_id: Set(pkg_model.id),
        registry: Set(oid.registry.unwrap().to_string()),
        namespace: Set(oid.namespace),
        repository: Set(oid.repository),
        tag: Set(oid.tag.to_string()),
        yank: Set(false),
        deprecated: Set(false),
        created_at: Set(DateTimeUtc::default()),
    };

    ActiveIndex::insert(idx_entry, &txn).await?;
    txn.commit().await?;

    info!("Added Package with oci identifier: {:?}", index_oci);
    Ok(StatusCode::CREATED)
}

// GET search a package
#[instrument]
pub async fn search(State(state): State<AppState>, Path(path): Path<String>) -> Result<(), Error> {
    todo!()
}

// GET download a pkg by hash
#[instrument]
pub async fn download(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<(), Error> {
    todo!()
}
