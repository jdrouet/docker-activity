use super::Formatter;
use crate::model::Record;

pub struct CsvFormatter;

#[cfg(feature = "enrichment-powercap")]
impl Formatter for CsvFormatter {
    fn format(&self, event: Record) -> Result<String, String> {
        Ok(format!(
            "{},{},{},{},{},{},{},{},{},{}",
            event.container_id,
            event.container_name,
            event.ts,
            event.pid_count.unwrap_or_default(),
            event.pid_limit.unwrap_or_default(),
            event.memory_usage.unwrap_or_default(),
            event.memory_limit.unwrap_or_default(),
            event.cpu_percent,
            event.cpu_count,
            event.cpu_energy.unwrap_or_default(),
        ))
    }
}

#[cfg(not(feature = "enrichment-powercap"))]
impl Formatter for CsvFormatter {
    fn format(&self, event: Record) -> Result<String, String> {
        Ok(format!(
            "{},{},{},{},{},{},{},{},{}",
            event.container_id,
            event.container_name,
            event.ts,
            event.pid_count.unwrap_or_default(),
            event.pid_limit.unwrap_or_default(),
            event.memory_usage.unwrap_or_default(),
            event.memory_limit.unwrap_or_default(),
            event.cpu_percent,
            event.cpu_count,
        ))
    }
}
