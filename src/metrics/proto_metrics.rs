use std::collections::HashMap;
use opentelemetry::metrics::MetricsError;
use opentelemetry_sdk::metrics::reader::MetricReader;
use crate::metrics::ActorMetrics;

#[derive(Debug, Clone)]
pub struct ProtoMetrics {
    actor_metrics: ActorMetrics,
    known_metrics: HashMap<String, ActorMetrics>,
}

impl ProtoMetrics {
    const INTERNAL_ACTOR_METRICS: &'static str = "internal.actor.metrics";
    pub fn new(reader: impl MetricReader) -> Result<Self, MetricsError> {
        let actor_metrics = ActorMetrics::new(reader)?;
        let mut myself = ProtoMetrics {
            actor_metrics,
            known_metrics: HashMap::new(),
        };
        myself.register(Self::INTERNAL_ACTOR_METRICS, myself.actor_metrics.clone())?;
        Ok(myself)
    }

    pub fn instruments(&self) -> &ActorMetrics {
        &self.actor_metrics
    }

    pub fn register(&mut self, key: &str, instance: ActorMetrics) -> Result<(), MetricsError> {
        if self.known_metrics.contains_key(key) {
            return Err(MetricsError::Other(format!("Key {} already exists", key)));
        }
        self.known_metrics.insert(key.to_string(), instance);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ActorMetrics> {
        self.known_metrics.get(key)
    }
}