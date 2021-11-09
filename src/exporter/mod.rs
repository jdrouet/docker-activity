mod file;

use crate::model::Record;
use clap::Parser;

pub trait Exporter {
    fn handle(&mut self, record: Record) -> Result<(), String>;
}

#[derive(Parser)]
pub enum Output {
    File(file::FileOutput),
}

impl Output {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        match self {
            Self::File(file) => file.exporter(),
        }
    }
}
