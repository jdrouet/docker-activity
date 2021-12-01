use bollard::container::Stats;

#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
#[cfg_attr(
    feature = "formatter-json",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct Record {
    pub container_id: String,
    pub container_name: String,
    pub ts: i64,
    pub pid_count: Option<u64>,
    pub pid_limit: Option<u64>,
    pub memory_usage: Option<u64>,
    pub memory_limit: Option<u64>,
    pub cpu_percent: f64,
    pub cpu_count: u64,
    #[cfg(feature = "enrichment-powercap")]
    pub cpu_energy: Option<f64>,
}

impl From<Stats> for Record {
    fn from(item: Stats) -> Self {
        let cpu_delta =
            item.cpu_stats.cpu_usage.total_usage - item.precpu_stats.cpu_usage.total_usage;
        let system_delta = item.cpu_stats.system_cpu_usage.unwrap_or_default()
            - item.precpu_stats.system_cpu_usage.unwrap_or_default();
        let cpu_count = item.cpu_stats.online_cpus.unwrap_or(1);
        let cpu_percent = cpu_delta as f64 / system_delta as f64;

        Self {
            container_id: item.id,
            container_name: item.name.trim_start_matches('/').to_string(),
            ts: item.read.timestamp(),
            pid_count: item.pids_stats.current,
            pid_limit: item.pids_stats.limit,
            memory_usage: item.memory_stats.usage,
            memory_limit: item.memory_stats.limit,
            cpu_percent,
            cpu_count,
            #[cfg(feature = "enrichment-powercap")]
            cpu_energy: None,
        }
    }
}

#[cfg(feature = "enrichment-powercap")]
impl Record {
    pub fn with_energy(mut self, total_cpu_energy: Option<u64>) -> Self {
        if let Some(total_cpu_energy) = total_cpu_energy {
            self.cpu_energy = Some(self.cpu_percent * total_cpu_energy as f64);
        }
        self
    }
}

#[cfg(test)]
impl Record {
    #[cfg(feature = "formatter-json")]
    pub fn random() -> Self {
        Self {
            container_id: "hello".into(),
            container_name: "world".into(),
            ts: 1234,
            pid_count: Some(12),
            pid_limit: Some(20),
            memory_usage: Some(14),
            memory_limit: None,
            cpu_count: 2,
            cpu_percent: 0.89,
            #[cfg(feature = "enrichment-powercap")]
            cpu_energy: Some(0.23),
        }
    }
}
