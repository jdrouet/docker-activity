use crate::exporter::Exporter;
use crate::format::{Format, Formatter};
use crate::model::Record;
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
pub struct FileOutput {
    /// Format of the output records.
    #[clap(short, long)]
    format: Option<Format>,
    /// Path to write the file.
    #[clap()]
    output: PathBuf,
}

impl FileOutput {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        let file = File::options()
            .create(true)
            .write(true)
            .append(true)
            .open(&self.output)
            .expect("couldn't open output file");
        let formatter = self.format.clone().unwrap_or_default().formatter();
        Box::new(FileExporter { file, formatter })
    }
}

pub struct FileExporter {
    file: File,
    formatter: Box<dyn Formatter>,
}

impl Exporter for FileExporter {
    fn handle(&mut self, record: Record) -> Result<(), String> {
        let mut line = self.formatter.format(record)?;
        line.push_str(super::LINE_ENDING);
        self.file
            .write_all(line.as_bytes())
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

#[cfg(all(test, feature = "formatter-json"))]
mod tests {
    use super::FileExporter;
    use crate::exporter::Exporter;
    use crate::model::Record;
    use std::fs::File;

    #[tokio::test]
    async fn unix_socket_should_receive() {
        let tmp = std::env::temp_dir().join("output.jsonp");
        let file = File::create(&tmp).unwrap();
        let formatter = Box::new(crate::format::json::JsonFormatter);
        let source = Record::random();
        let mut exporter = FileExporter { file, formatter };
        exporter.handle(source.clone()).unwrap();
        let data = std::fs::read_to_string(tmp).unwrap();
        assert_eq!(serde_json::to_string(&source).unwrap(), data.trim());
    }
}
