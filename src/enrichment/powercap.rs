use super::Enricher;
use crate::model::Record;
use powercap::PowerCap;
use std::sync::Arc;

pub struct PowerCapEnricher {
    inner: Arc<PowerCap>,
    last: Option<u64>,
}

impl From<Arc<PowerCap>> for PowerCapEnricher {
    fn from(inner: Arc<PowerCap>) -> Self {
        Self { inner, last: None }
    }
}

impl Enricher for PowerCapEnricher {
    fn enrich(&mut self, record: Record) -> Record {
        let current = self.inner.intel_rapl.total_energy().ok();
        let result = if let (Some(last), Some(now)) = (self.last, current) {
            record.with_energy(Some(now - last))
        } else {
            record
        };
        self.last = current;
        result
    }

    fn reset(&mut self) {
        self.last = None;
    }
}
