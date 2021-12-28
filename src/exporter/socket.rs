use crate::exporter::Exporter;
use crate::format::{Format, Formatter};
use crate::model::Record;
use clap::Parser;
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[cfg(not(windows))]
#[derive(Parser)]
pub struct UnixSocketOutput {
    /// Format of the output records.
    #[clap(short, long)]
    format: Option<Format>,
    /// Path to the unix socket.
    #[clap()]
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
    /// Format of the output records.
    #[clap(short, long)]
    format: Option<Format>,
    /// Server address.
    #[clap()]
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
        line.push_str(super::LINE_ENDING);
        self.stream
            .write_all(line.as_bytes())
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

#[cfg(all(test, feature = "formatter-json"))]
mod tests {
    use super::SocketExporter;
    use crate::exporter::Exporter;
    use crate::model::Record;
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn unix_socket_should_receive() {
        let (tx, mut rx) = mpsc::channel::<String>(10);
        let (ts, mut rs) = UnixStream::pair().unwrap();
        let handler = tokio::spawn(async move {
            let mut buffer: [u8; 170] = [0; 170];
            rs.read(&mut buffer).unwrap();
            let buffer = std::str::from_utf8(&buffer).unwrap().to_string();
            tx.send(buffer).await.unwrap();
        });
        let mut exporter = SocketExporter {
            stream: Box::new(ts),
            formatter: Box::new(crate::format::json::JsonFormatter),
        };
        exporter.handle(Record::random()).unwrap();
        handler.await.unwrap();
        let mut result = Vec::with_capacity(1);
        while let Some(next) = rx.recv().await {
            result.push(next);
        }
        assert_eq!(result.len(), 1);
    }
}
