use crate::exporter::Exporter;
use crate::format::{Format, Formatter};
use crate::model::Snapshot;
use clap::Parser;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
pub struct FileOutput {
    #[clap(short, long, about = "Format of the output records")]
    format: Option<Format>,
    #[clap(about = "Path to write the file")]
    output: PathBuf,
}

impl FileOutput {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        let file = File::with_options()
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
    fn handle(&mut self, record: Snapshot) -> Result<(), String> {
        let line = self
            .formatter
            .format(record)
            .map_err(|err| err.to_string())?;
        writeln!(self.file, "{}", line).map_err(|err| err.to_string())?;
        Ok(())
    }
}
