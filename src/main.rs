use bollard::container::{Stats, StatsOptions};
use bollard::Docker;
use clap::Parser;
use futures_util::stream::StreamExt;
use powercap::PowerCap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

fn both<A, B>(a: Option<A>, b: Option<B>) -> Option<(A, B)> {
    if let (Some(a), Some(b)) = (a, b) {
        Some((a, b))
    } else {
        None
    }
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
    cpu_energy: Option<f64>,
}

impl Snapshot {
    fn build(item: Stats, total_cpu_energy: Option<u64>) -> Self {
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
        let cpu_ratio = cpu_delta as f64 / system_delta as f64;
        let cpu_percent = cpu_ratio * factor * 100.0;
        let cpu_energy = total_cpu_energy.map(|value| value as f64 * cpu_ratio);

        Self {
            ts: item.read.timestamp(),
            pid_count: item.pids_stats.current,
            pid_limit: item.pids_stats.limit,
            memory_usage: item.memory_stats.usage,
            memory_limit: item.memory_stats.limit,
            cpu_percent,
            cpu_count,
            cpu_energy,
        }
    }
}

#[derive(Parser)]
struct Application {
    #[clap(about = "Name or ID of the container to monitor")]
    container: String,
    #[clap(about = "Path to write the file")]
    output: PathBuf,
}

impl Application {
    fn write_header(&self, file: &mut File) -> Result<(), Box<dyn std::error::Error>> {
        writeln!(
            file,
            "ts,pid_count,pid_limit,memory_usage,memory_limit,cpu_percent,cpu_count,cpu_energy",
        )?;
        Ok(())
    }

    async fn process_stream(
        &self,
        file: &mut File,
        docker: &Docker,
        powercap: Option<&PowerCap>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let stream = &mut docker.stats(
            &self.container,
            Some(StatsOptions {
                stream: true,
                one_shot: false,
            }),
        );
        let mut last_energy: Option<u64> = None;
        while let Some(Ok(stat)) = stream.next().await {
            let energy = powercap.and_then(|pcap| pcap.intel_rapl.total_energy().ok());
            let delta = both(last_energy, energy).map(|(prev, next)| next - prev);
            last_energy = energy;
            let snap = Snapshot::build(stat, delta);
            writeln!(
                file,
                "{},{},{},{},{},{},{},{}",
                snap.ts,
                snap.pid_count.unwrap_or_default(),
                snap.pid_limit.unwrap_or_default(),
                snap.memory_usage.unwrap_or_default(),
                snap.memory_limit.unwrap_or_default(),
                snap.cpu_percent,
                snap.cpu_count,
                snap.cpu_energy.unwrap_or_default(),
            )?;
        }
        Ok(())
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::create(&self.output).expect("Couldn't create file");
        let docker =
            Docker::connect_with_local_defaults().expect("Couldn't connect to docker engine");
        let powercap = PowerCap::try_default().ok();
        self.write_header(&mut file)?;
        loop {
            self.process_stream(&mut file, &docker, powercap.as_ref())
                .await?;
            tokio::time::sleep(Duration::new(1, 0)).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = Application::parse();
    app.run().await
}
