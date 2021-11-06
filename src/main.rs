use bollard::container::{Stats, StatsOptions};
use bollard::models::SystemEventsResponse;
use bollard::system::EventsOptions;
use bollard::Docker;
use clap::Parser;
use futures_util::stream::StreamExt;
use powercap::PowerCap;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

struct ContainerWatcher {
    docker: Arc<Docker>,
    powercap: Arc<Option<PowerCap>>,
    name: String,
    file: File,
}

impl ContainerWatcher {
    async fn execute(
        docker: Arc<Docker>,
        powercap: Arc<Option<PowerCap>>,
        register: Arc<Mutex<HashSet<String>>>,
        name: String,
        path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(&path)?;
        let mut watcher = ContainerWatcher {
            docker,
            powercap,
            name,
            file,
        };
        watcher.run(register).await
    }

    async fn run(
        &mut self,
        register: Arc<Mutex<HashSet<String>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("watching container={}", self.name);
        let stream = &mut self.docker.stats(
            &self.name,
            Some(StatsOptions {
                stream: true,
                one_shot: false,
            }),
        );
        let mut last_energy: Option<u64> = None;
        while let Some(Ok(stat)) = stream.next().await {
            let energy = self
                .powercap
                .as_ref()
                .as_ref()
                .and_then(|pcap| pcap.intel_rapl.total_energy().ok());
            let delta = both(last_energy, energy).map(|(prev, next)| next - prev);
            last_energy = energy;
            let snap = Snapshot::build(stat, delta);
            writeln!(
                self.file,
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

        let mut lock = register.try_lock();
        if let Ok(ref mut mutex) = lock {
            mutex.remove(&self.name);
            println!("done watching container={}", self.name);
        } else {
            eprintln!("couldn't unregister container={}", self.name);
        }
        Ok(())
    }
}

fn get_container_name(event: &SystemEventsResponse) -> Option<String> {
    event
        .actor
        .as_ref()
        .and_then(|actor| actor.attributes.as_ref())
        .and_then(|attrs| attrs.get("name"))
        .cloned()
}

struct Orchestrator {
    docker: Arc<Docker>,
    powercap: Arc<Option<PowerCap>>,
    names: HashSet<String>,
    path: PathBuf,
    tasks: Arc<Mutex<HashSet<String>>>,
}

impl TryFrom<Params> for Orchestrator {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Params) -> Result<Self, Self::Error> {
        let docker = Arc::new(Docker::connect_with_local_defaults()?);
        let powercap = Arc::new(PowerCap::try_default().ok());
        Ok(Self {
            docker,
            powercap,
            names: value
                .containers
                .map(|value| value.split(',').map(String::from).collect())
                .unwrap_or_default(),
            path: value.output,
            tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}

impl Orchestrator {
    fn is_running(&self, name: &String) -> bool {
        self.tasks
            .lock()
            .map(|lock| lock.contains(name))
            .unwrap_or(false)
    }

    fn register_task(&self, name: String) {
        self.tasks
            .lock()
            .map(|mut lock| lock.insert(name))
            .expect("mutex is corrupted");
    }

    fn handle_start_event(
        &mut self,
        container_name: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_running(&container_name) {
            return Ok(());
        }
        self.register_task(container_name.clone());
        let docker = self.docker.clone();
        let powercap = self.powercap.clone();
        let tasks = self.tasks.clone();
        let filename = format!("{}.csv", container_name);
        let path = self.path.join(&filename);
        tokio::spawn(async {
            if let Err(err) =
                ContainerWatcher::execute(docker, powercap, tasks, container_name, path).await
            {
                eprintln!("container watcher errored: {:?}", err);
            }
        });
        Ok(())
    }

    fn handle_event(
        &mut self,
        container_name: String,
        action: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.names.is_empty() && !self.names.contains(&container_name) {
            return Ok(());
        }
        match action.as_ref().map(|action| action.as_str()) {
            Some("start") => self.handle_start_event(container_name),
            _ => Ok(()),
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut filters = HashMap::new();
        filters.insert("type", vec!["container"]);
        let stream = &mut self.docker.events(Some(EventsOptions {
            since: None,
            until: None,
            filters,
        }));
        while let Some(Ok(event)) = stream.next().await {
            if let Some(container_name) = get_container_name(&event) {
                println!(
                    "received action {:?} for container {}",
                    event.action, container_name
                );
                if let Err(err) = self.handle_event(container_name, event.action) {
                    eprintln!("couldn't handle event: {:?}", err);
                }
            }
        }
        Ok(())
    }
}

#[derive(Parser)]
struct Params {
    #[clap(
        long,
        about = "Name or ID of the container to monitor, separated by comma."
    )]
    containers: Option<String>,
    #[clap(about = "Path to write the file")]
    output: PathBuf,
}

/*
impl Params {
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
*/

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let params = Params::parse();
    let mut orchestrator = Orchestrator::try_from(params)?;
    orchestrator.run().await
}
