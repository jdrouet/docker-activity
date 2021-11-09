use crate::model::Record;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Format {
    Json,
    Csv,
}

impl Default for Format {
    fn default() -> Self {
        Self::Json
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            other => Err(format!("unknown format {:?}", other)),
        }
    }
}

impl Format {
    pub fn formatter(&self) -> Box<dyn Formatter> {
        match self {
            Self::Csv => Box::new(CsvFormatter),
            Self::Json => Box::new(JsonFormatter),
        }
    }
}

pub trait Formatter {
    fn format(&self, event: Record) -> Result<String, String>;
}

pub struct CsvFormatter;

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

pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn format(&self, event: Record) -> Result<String, String> {
        serde_json::to_string(&event).map_err(|err| err.to_string())
    }
}
