mod powercap;

use crate::model::Record;
use crate::Params;
use std::sync::Arc;
use tracing::warn;

pub trait Enricher: Send {
    fn enrich(&mut self, record: Record) -> Record {
        record
    }

    fn reset(&mut self) {}
}

pub struct EnrichmentStack(Vec<Box<dyn Enricher>>);

impl Enricher for EnrichmentStack {
    fn enrich(&mut self, record: Record) -> Record {
        self.0.iter_mut().fold(record, |r, next| next.enrich(r))
    }

    fn reset(&mut self) {
        self.0.iter_mut().for_each(|next| next.reset())
    }
}

#[derive(Clone)]
pub struct EnrichmentBuilder {
    powercap: Option<Arc<::powercap::PowerCap>>,
}

impl Params {
    fn create_powercap(&self) -> Option<::powercap::PowerCap> {
        if self.disable_powercap {
            None
        } else {
            match ::powercap::PowerCap::try_default() {
                Ok(value) => Some(value),
                Err(error) => {
                    warn!("unable to create powercap reader: {:?}", error);
                    None
                }
            }
        }
    }

    pub fn enrichment_builder(&self) -> EnrichmentBuilder {
        EnrichmentBuilder {
            powercap: self.create_powercap().map(Arc::new),
        }
    }
}

impl EnrichmentBuilder {
    pub fn build(&self) -> EnrichmentStack {
        let mut result: Vec<Box<dyn Enricher>> = Vec::new();
        if let Some(pcap) = self.powercap.as_ref() {
            result.push(Box::new(powercap::PowerCapEnricher::from(pcap.clone())));
        }
        EnrichmentStack(result)
    }
}
