use crate::exporter::Exporter;
use crate::format::{Format, Formatter};
use crate::model::Record;
use clap::Parser;
use std::io::{self, Write};

#[derive(Parser)]
pub struct StdOutOutput {
    /// Format of the output records.
    #[clap(short, long)]
    format: Option<Format>,
}

impl StdOutOutput {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        let inner = io::stdout();
        let formatter = self.format.clone().unwrap_or_default().formatter();
        Box::new(StdOutExporter { inner, formatter })
    }
}

pub struct StdOutExporter {
    inner: io::Stdout,
    formatter: Box<dyn Formatter>,
}

impl Exporter for StdOutExporter {
    fn handle(&mut self, record: Record) -> Result<(), String> {
        let mut line = self.formatter.format(record)?;
        line.push_str(super::LINE_ENDING);
        self.inner
            .write_all(line.as_bytes())
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

#[cfg(all(test, feature = "formatter-json"))]
mod tests {
    use super::StdOutExporter;
    use crate::exporter::Exporter;
    use crate::model::Record;

    #[tokio::test]
    async fn unix_socket_should_receive() {
        let inner = std::io::stdout();
        let formatter = Box::new(crate::format::json::JsonFormatter);
        let source = Record::random();
        let mut exporter = StdOutExporter { inner, formatter };
        assert!(exporter.handle(source.clone()).is_ok());
    }
}
