mod file;
mod socket;
mod stdout;

use crate::model::Record;
use clap::Parser;

const LINE_ENDING: &str = "\n";

pub trait Exporter {
    fn handle(&mut self, record: Record) -> Result<(), String>;
}

#[derive(Parser)]
pub enum Output {
    #[clap(about = "Write the data to a file")]
    File(file::FileOutput),
    #[cfg(not(windows))]
    #[clap(about = "Write the data to a unix socket")]
    UnixSocket(socket::UnixSocketOutput),
    #[clap(about = "Write the data to a tcp socket")]
    TcpSocket(socket::TcpSocketOutput),
    #[clap(about = "Write to standard output")]
    Stdout(stdout::StdOutOutput),
}

impl Output {
    pub fn exporter(&self) -> Box<dyn Exporter> {
        match self {
            Self::File(file) => file.exporter(),
            #[cfg(not(windows))]
            Self::UnixSocket(socket) => socket.exporter(),
            Self::TcpSocket(socket) => socket.exporter(),
            Self::Stdout(socket) => socket.exporter(),
        }
    }
}
