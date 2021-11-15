use super::Formatter;
use crate::model::Record;

pub struct JsonFormatter;

impl Formatter for JsonFormatter {
    fn format(&self, event: Record) -> Result<String, String> {
        serde_json::to_string(&event).map_err(|err| err.to_string())
    }
}
