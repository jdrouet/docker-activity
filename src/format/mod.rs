pub(crate) mod csv;
#[cfg(feature = "formatter-json")]
pub(crate) mod json;

use crate::model::Record;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum Format {
    #[cfg(feature = "formatter-json")]
    Json,
    Csv,
}

#[cfg(not(feature = "formatter-json"))]
impl Default for Format {
    fn default() -> Self {
        Self::Csv
    }
}

#[cfg(feature = "formatter-json")]
impl Default for Format {
    fn default() -> Self {
        Self::Json
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            #[cfg(feature = "formatter-json")]
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            other => Err(format!("unknown format {:?}", other)),
        }
    }
}

impl Format {
    pub fn formatter(&self) -> Box<dyn Formatter> {
        match self {
            Self::Csv => Box::new(csv::CsvFormatter),
            #[cfg(feature = "formatter-json")]
            Self::Json => Box::new(json::JsonFormatter),
        }
    }
}

pub trait Formatter {
    fn format(&self, event: Record) -> Result<String, String>;
}
