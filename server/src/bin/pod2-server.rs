use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use pod2_server::{config::Config, db, handlers, routes};
use tokio::net::TcpListener;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Optional path to a configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    // Use RUST_LOG=info,pod2_server=debug,tower_http=debug for detailed tracing
    env_logger::init();

    let args = Args::parse();

    // Load configuration
    let config = Config::load(args.config, None).context("Failed to load configuration")?;

    log::info!("Loaded configuration: {:?}", config);

    // Initialize the database
    let db = db::Db::new(Some(&config.db_path), &db::MIGRATIONS)
        .await
        .context("Failed to initialize database")?;

    handlers::playground::setup_zukyc_space(&db)
        .await
        .context("Failed to setup Zukyc space")?;

    // Create the Axum router
    let app = routes::create_router(Arc::new(db));

    // Bind the server to the address
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr)
        .await
        .context(format!("Failed to bind TCP listener to {}", addr))?;
    log::info!("Server listening on {}", addr);

    // Start the server
    axum::serve(listener, app.into_make_service())
        .await
        .context("Axum server failed")?;

    Ok(())
}
