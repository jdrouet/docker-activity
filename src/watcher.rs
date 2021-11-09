use crate::error::Error;
use crate::model::Record;
use crate::Params;
use bollard::container::StatsOptions;
use bollard::models::SystemEventsResponse;
use bollard::system::EventsOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
use powercap::PowerCap;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

fn both<A, B>(a: Option<A>, b: Option<B>) -> Option<(A, B)> {
    if let (Some(a), Some(b)) = (a, b) {
        Some((a, b))
    } else {
        None
    }
}

struct ContainerWatcher {
    docker: Arc<Docker>,
    powercap: Arc<Option<PowerCap>>,
    name: String,
}

impl ContainerWatcher {
    async fn execute(
        docker: Arc<Docker>,
        powercap: Arc<Option<PowerCap>>,
        register: Arc<Mutex<HashSet<String>>>,
        name: String,
        tx: mpsc::Sender<Record>,
    ) -> Result<(), Error> {
        let mut watcher = ContainerWatcher {
            docker,
            powercap,
            name,
        };
        watcher.run(register, tx).await
    }

    async fn run(
        &mut self,
        register: Arc<Mutex<HashSet<String>>>,
        tx: mpsc::Sender<Record>,
    ) -> Result<(), Error> {
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
            let snap = Record::from(stat);
            let energy = self
                .powercap
                .as_ref()
                .as_ref()
                .and_then(|pcap| pcap.intel_rapl.total_energy().ok());
            let delta = both(last_energy, energy).map(|(prev, next)| next - prev);
            last_energy = energy;
            if let Err(err) = tx.send(snap.with_energy(delta)).await {
                eprintln!("unable to forward snapshot: {:?}", err);
            }
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

pub struct Orchestrator {
    docker: Arc<Docker>,
    powercap: Arc<Option<PowerCap>>,
    names: HashSet<String>,
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
            tasks: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}

impl Orchestrator {
    fn is_running(&self, name: &str) -> bool {
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
        tx: mpsc::Sender<Record>,
    ) -> Result<(), Error> {
        if self.is_running(&container_name) {
            return Ok(());
        }
        self.register_task(container_name.clone());
        let docker = self.docker.clone();
        let powercap = self.powercap.clone();
        let tasks = self.tasks.clone();
        tokio::spawn(async {
            if let Err(err) =
                ContainerWatcher::execute(docker, powercap, tasks, container_name, tx).await
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
        tx: mpsc::Sender<Record>,
    ) -> Result<(), Error> {
        if !self.names.is_empty() && !self.names.contains(&container_name) {
            return Ok(());
        }
        match action.as_deref() {
            Some("start") => self.handle_start_event(container_name, tx),
            _ => Ok(()),
        }
    }

    pub async fn run(&mut self, tx: mpsc::Sender<Record>) -> Result<(), Error> {
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
                if let Err(err) = self.handle_event(container_name, event.action, tx.clone()) {
                    eprintln!("couldn't handle event: {:?}", err);
                }
            }
        }
        Ok(())
    }
}
