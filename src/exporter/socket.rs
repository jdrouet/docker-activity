use crate::exporter::Exporter;
use crate::format::{Format, Formatter};
use crate::model::Record;
use clap::Parser;
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[cfg(windows)]
const LINE_ENDING: &str = "\r\n";
#[cfg(not(windows))]
const LINE_ENDING: &str = "\n";

#[cfg(not(windows))]
#[derive(Parser)]
pub struct UnixSocketOutput {
    #[clap(short, long, about = "Format of the output records")]
    format: Option<Format>,
    #[clap(about = "Path to write the file")]
    output: PathBuf,
}

#[cfg(not(windows))]
impl UnixSocketOutput {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        let stream = Box::new(UnixStream::connect(&self.output).expect("couldn't open socket"));
        let formatter = self.format.clone().unwrap_or_default().formatter();
        Box::new(SocketExporter { stream, formatter })
    }
}

#[derive(Parser)]
pub struct TcpSocketOutput {
    #[clap(short, long, about = "Format of the output records")]
    format: Option<Format>,
    #[clap(about = "Server address")]
    address: String,
}

impl TcpSocketOutput {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        let stream = Box::new(TcpStream::connect(&self.address).expect("couldn't open socket"));
        let formatter = self.format.clone().unwrap_or_default().formatter();
        Box::new(SocketExporter { stream, formatter })
    }
}

pub struct SocketExporter {
    stream: Box<dyn Write>,
    formatter: Box<dyn Formatter>,
}

impl Exporter for SocketExporter {
    fn handle(&mut self, record: Record) -> Result<(), String> {
        let mut line = self.formatter.format(record)?;
        line.push_str(LINE_ENDING);
        self.stream
            .write_all(line.as_bytes())
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}
