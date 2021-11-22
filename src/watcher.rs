use crate::error::Error;
use crate::model::Record;
use crate::Params;
use bollard::container::{ListContainersOptions, StatsOptions};
use bollard::models::SystemEventsResponse;
use bollard::system::EventsOptions;
use bollard::Docker;
use futures_util::stream::StreamExt;
#[cfg(feature = "enrichment-powercap")]
use powercap::PowerCap;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

#[cfg(feature = "enrichment-powercap")]
fn both<A, B>(a: Option<A>, b: Option<B>) -> Option<(A, B)> {
    if let (Some(a), Some(b)) = (a, b) {
        Some((a, b))
    } else {
        None
    }
}

struct ContainerWatcher {
    docker: Arc<Docker>,
    #[cfg(feature = "enrichment-powercap")]
    powercap: Arc<Option<PowerCap>>,
    name: String,
    #[cfg(feature = "enrichment-powercap")]
    last_energy: Option<u64>,
}

#[cfg(feature = "enrichment-powercap")]
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
            last_energy: None,
        };
        watcher.run(register, tx).await
    }

    fn enrichment(&mut self, record: Record) -> Record {
        let energy = self
            .powercap
            .as_ref()
            .as_ref()
            .and_then(|pcap| pcap.intel_rapl.total_energy().ok());
        let delta = both(self.last_energy, energy).map(|(prev, next)| next - prev);
        self.last_energy = energy;
        record.with_energy(delta)
    }

    fn reset(&mut self) {
        self.last_energy = None;
    }
}

#[cfg(not(feature = "enrichment-powercap"))]
impl ContainerWatcher {
    async fn execute(
        docker: Arc<Docker>,
        register: Arc<Mutex<HashSet<String>>>,
        name: String,
        tx: mpsc::Sender<Record>,
    ) -> Result<(), Error> {
        let mut watcher = ContainerWatcher { docker, name };
        watcher.run(register, tx).await
    }

    fn enrichment(&mut self, record: Record) -> Record {
        record
    }

    fn reset(&mut self) {}
}

impl ContainerWatcher {
    async fn is_alive(&mut self) -> Result<bool, Error> {
        trace!("checking if container {:?} is still running", self.name);
        let mut filters: HashMap<&str, Vec<&str>> = HashMap::new();
        filters.insert("name", vec![self.name.as_str()]);
        filters.insert("status", vec!["running"]);
        self.docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                limit: None,
                size: false,
                filters,
            }))
            .await
            .map(|list| !list.is_empty())
            .map_err(|err| Error::Custom(format!("couldn't list containers: {:?}", err)))
    }

    async fn run(
        &mut self,
        register: Arc<Mutex<HashSet<String>>>,
        tx: mpsc::Sender<Record>,
    ) -> Result<(), Error> {
        info!("watching container {:?}", self.name);
        self.reset();
        while self.is_alive().await? {
            let stream = &mut self.docker.stats(
                &self.name,
                Some(StatsOptions {
                    stream: true,
                    one_shot: false,
                }),
            );
            debug!("starting the watch of {:?}", self.name);
            while let Some(Ok(stat)) = stream.next().await {
                let snap = Record::from(stat);
                let snap = self.enrichment(snap);
                if let Err(err) = tx.send(snap).await {
                    warn!("unable to forward snapshot: {:?}", err);
                }
            }
            debug!("lost connection with stats for container {:?}", self.name);
        }

        let mut lock = register.try_lock();
        if let Ok(ref mut mutex) = lock {
            mutex.remove(&self.name);
            info!("done watching container {:?}", self.name);
        } else {
            warn!("couldn't unregister container {:?}", self.name);
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
    #[cfg(feature = "enrichment-powercap")]
    powercap: Arc<Option<PowerCap>>,
    names: HashSet<String>,
    tasks: Arc<Mutex<HashSet<String>>>,
}

#[cfg(feature = "enrichment-powercap")]
impl TryFrom<Params> for Orchestrator {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Params) -> Result<Self, Self::Error> {
        let docker = Arc::new(Docker::connect_with_local_defaults()?);
        let powercap = if value.disable_powercap {
            Arc::new(None)
        } else {
            Arc::new(PowerCap::try_default().ok())
        };
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

#[cfg(not(feature = "enrichment-powercap"))]
impl TryFrom<Params> for Orchestrator {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Params) -> Result<Self, Self::Error> {
        let docker = Arc::new(Docker::connect_with_local_defaults()?);
        Ok(Self {
            docker,
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
            debug!("container {:?} already runnin", container_name);
            return Ok(());
        }
        self.register_task(container_name.clone());
        let docker = self.docker.clone();
        #[cfg(feature = "enrichment-powercap")]
        let powercap = self.powercap.clone();
        let tasks = self.tasks.clone();
        tokio::spawn(async {
            if let Err(err) = ContainerWatcher::execute(
                docker,
                #[cfg(feature = "enrichment-powercap")]
                powercap,
                tasks,
                container_name,
                tx,
            )
            .await
            {
                warn!("container watcher errored: {:?}", err);
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

    async fn list_running(&mut self) -> Result<Vec<String>, Error> {
        let mut filters: HashMap<&str, Vec<&str>> = HashMap::new();
        filters.insert("status", vec!["running"]);
        self.docker
            .list_containers(Some(ListContainersOptions {
                all: true,
                limit: None,
                size: false,
                filters,
            }))
            .await
            .map(|list| {
                list.into_iter()
                    .filter_map(|item| item.names.and_then(|names| names.first().cloned()))
                    .collect()
            })
            .map_err(|err| Error::Custom(format!("couldn't list running containers: {:?}", err)))
    }

    pub async fn run(&mut self, tx: mpsc::Sender<Record>) -> Result<(), Error> {
        for name in self.list_running().await? {
            self.handle_start_event(name, tx.clone())?;
        }
        let mut filters = HashMap::new();
        filters.insert("type", vec!["container"]);
        let stream = &mut self.docker.events(Some(EventsOptions {
            since: None,
            until: None,
            filters,
        }));
        while let Some(Ok(event)) = stream.next().await {
            if let Some(container_name) = get_container_name(&event) {
                debug!(
                    "received action {:?} for container {:?}",
                    event.action, container_name
                );
                if let Err(err) = self.handle_event(container_name, event.action, tx.clone()) {
                    warn!("couldn't handle event: {:?}", err);
                }
            }
        }
        Ok(())
    }
}
