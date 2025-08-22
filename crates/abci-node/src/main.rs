use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber::{EnvFilter, fmt};
use tendermint_abci::ServerBuilder;

mod app;
mod exec;
mod wire;

use crate::app::EvmAbciApp;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with default INFO level if RUST_LOG not set
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .init();

    info!("Starting ABCI server for Reth-CometBFT integration");

    // Open Reth context (db, txpool, chain config)
    let app = match EvmAbciApp::boot("./data/reth") {
        Ok(app) => {
            info!("Successfully initialized Reth context");
            app
        },
        Err(e) => {
            error!("Failed to initialize Reth context: {}", e);
            return Err(e);
        }
    };

    // Start ABCI server
    info!("Starting ABCI server on 127.0.0.1:26658");
    
    // Handle graceful shutdown
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Received shutdown signal");
        tx.send(()).ok();
    });

    // ABCI over TCP for CometBFT's proxy_app
    let server = ServerBuilder::new(1024)
        .bind("127.0.0.1:26658", app)?;
    
        tokio::select! {
            _ = async {
                // Run the blocking server.listen() in a separate thread
                let server_result = tokio::task::spawn_blocking(move || {
                    server.listen()
                }).await.unwrap();
                
                if let Err(e) = server_result {
                    error!("ABCI server error: {}", e);
                    // Handle error
                }
            } => {}
            _ = rx => {
                info!("Shutting down ABCI server gracefully");
            }
        }

    info!("ABCI server stopped");
    Ok(())
}