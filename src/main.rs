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
struct Params {
    #[clap(long, about = "Size of the buffer.", default_value = "32")]
    pub buffer_size: usize,
    #[clap(long, about = "Level of logging.", default_value = "info")]
    pub log_level: tracing::Level,
    #[clap(
        long,
        about = "Name or ID of the container to monitor, separated by comma."
    )]
    pub containers: Option<String>,
    #[cfg(feature = "enrichment-powercap")]
    #[clap(long, about = "Disable monitoring power consumption.")]
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
