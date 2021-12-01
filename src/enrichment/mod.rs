#[cfg(feature = "enrichment-powercap")]
mod powercap;

use crate::model::Record;
use crate::Params;
#[cfg(feature = "enrichment-powercap")]
use std::sync::Arc;

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
    #[cfg(feature = "enrichment-powercap")]
    powercap: Option<Arc<::powercap::PowerCap>>,
}

impl Params {
    #[cfg(feature = "enrichment-powercap")]
    fn create_powercap(&self) -> Option<::powercap::PowerCap> {
        if self.disable_powercap {
            None
        } else {
            match ::powercap::PowerCap::try_default() {
                Ok(value) => Some(value),
                Err(error) => {
                    tracing::warn!("unable to create powercap reader: {:?}", error);
                    None
                }
            }
        }
    }

    pub fn enrichment_builder(&self) -> EnrichmentBuilder {
        EnrichmentBuilder {
            #[cfg(feature = "enrichment-powercap")]
            powercap: self.create_powercap().map(Arc::new),
        }
    }
}

impl EnrichmentBuilder {
    #[cfg(not(any(feature = "enrichment-powercap")))]
    pub fn build(&self) -> EnrichmentStack {
        EnrichmentStack(Vec::new())
    }

    #[cfg(any(feature = "enrichment-powercap"))]
    pub fn build(&self) -> EnrichmentStack {
        let mut result: Vec<Box<dyn Enricher>> = Vec::new();
        #[cfg(feature = "enrichment-powercap")]
        if let Some(pcap) = self.powercap.as_ref() {
            result.push(Box::new(powercap::PowerCapEnricher::from(pcap.clone())));
        }
        EnrichmentStack(result)
    }
}
