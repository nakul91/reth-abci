use anyhow::Result;
use tracing_subscriber::{EnvFilter, fmt};
use tendermint_abci::ServerBuilder;

mod app;
mod exec;
mod wire;

use crate::app::EvmAbciApp;

#[tokio::main]
async fn main() -> Result<()> {
    // logging
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    // open Reth context (db, txpool, chain config)
    let app = EvmAbciApp::boot("./data/reth")?;

    // ABCI over TCP for CometBFT's proxy_app
    ServerBuilder::new(app)
        .bind("127.0.0.1:26658")?
        .listen()
        .await?;

    Ok(())
}
