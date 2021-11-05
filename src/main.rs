use bollard::container::{Stats, StatsOptions};
use bollard::Docker;
use clap::Parser;
use futures_util::stream::StreamExt;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
struct Params {
    #[clap(about = "Name or ID of the container to monitor")]
    container: String,
    #[clap(about = "Path to write the file")]
    output: PathBuf,
}

#[derive(Debug)]
struct Snapshot {
    ts: i64,
    pid_count: Option<u64>,
    pid_limit: Option<u64>,
    memory_usage: Option<u64>,
    memory_limit: Option<u64>,
    cpu_percent: f64,
    cpu_count: u64,
}

impl From<Stats> for Snapshot {
    fn from(item: Stats) -> Self {
        let cpu_delta =
            item.cpu_stats.cpu_usage.total_usage - item.precpu_stats.cpu_usage.total_usage;
        let system_delta = item.cpu_stats.system_cpu_usage.unwrap_or_default()
            - item.precpu_stats.system_cpu_usage.unwrap_or_default();
        let cpu_count = item.cpu_stats.online_cpus.unwrap_or(1);
        let factor = item
            .cpu_stats
            .cpu_usage
            .percpu_usage
            .map(|list| list.len() as u64)
            .unwrap_or(cpu_count) as f64;
        let cpu_percent = (cpu_delta as f64 / system_delta as f64) * factor * 100.0;

        Self {
            ts: item.read.timestamp(),
            pid_count: item.pids_stats.current,
            pid_limit: item.pids_stats.limit,
            memory_usage: item.memory_stats.usage,
            memory_limit: item.memory_stats.limit,
            cpu_percent,
            cpu_count,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let params = Params::parse();
    let mut file = File::create(params.output).expect("Couldn't create file");
    let docker = Docker::connect_with_local_defaults().unwrap();

    let stream = &mut docker.stats(
        &params.container,
        Some(StatsOptions {
            stream: true,
            one_shot: false,
        }),
    );

    writeln!(
        file,
        "ts,pid_count,pid_limit,memory_usage,memory_limit,cpu_percent,cpu_count",
    )?;

    while let Some(Ok(stat)) = stream.next().await {
        let snap = Snapshot::from(stat);
        writeln!(
            file,
            "{},{},{},{},{},{},{}",
            snap.ts,
            snap.pid_count.unwrap_or_default(),
            snap.pid_limit.unwrap_or_default(),
            snap.memory_usage.unwrap_or_default(),
            snap.memory_limit.unwrap_or_default(),
            snap.cpu_percent,
            snap.cpu_count
        )?;
    }

    Ok(())
}
