mod enrichment;
mod error;
mod exporter;
mod format;
mod model;
mod watcher;

use crate::watcher::Orchestrator;
use clap::Parser;
use std::convert::TryFrom;
use tokio::sync::mpsc;

#[derive(Parser)]
#[clap(about, version, author)]
struct Params {
    /// Size of the buffer.
    #[clap(long, default_value = "32")]
    pub buffer_size: usize,
    /// Level of logging.
    #[clap(long, default_value = "info")]
    pub log_level: tracing::Level,
    /// Name or ID of the container to monitor, separated by comma.
    #[clap(long)]
    pub containers: Option<String>,
    /// Disable monitoring power consumption.
    #[cfg(feature = "enrichment-powercap")]
    #[clap(long)]
    pub disable_powercap: bool,
    #[clap(subcommand)]
    pub output: exporter::Output,
}

#[tokio::main]
async fn main() {
    let params = Params::parse();

    tracing_subscriber::fmt()
        .with_max_level(params.log_level)
        .init();

    let (tx, mut rx) = mpsc::channel(params.buffer_size);
    let mut exporter = params.output.exporter();
    tokio::spawn(async move {
        let mut orchestrator = Orchestrator::try_from(params).expect("couldn't build orchestrator");
        orchestrator.run(tx).await
    });
    while let Some(snap) = rx.recv().await {
        exporter.handle(snap).expect("couldn't export event");
    }
}
