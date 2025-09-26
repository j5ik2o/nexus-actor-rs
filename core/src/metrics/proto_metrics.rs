use crate::actor::MetricsProvider;
use crate::metrics::{ActorMetrics, MetricsError};
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ProtoMetrics {
  actor_metrics: ActorMetrics,
  known_metrics: Arc<DashMap<String, ActorMetrics>>,
}

impl ProtoMetrics {
  pub const INTERNAL_ACTOR_METRICS: &'static str = "internal.actor.metrics";

  pub fn new(meter_provider: Arc<MetricsProvider>) -> Result<Self, MetricsError> {
    let actor_metrics = ActorMetrics::new(meter_provider.clone());
    let mut myself = ProtoMetrics {
      actor_metrics,
      known_metrics: Arc::new(DashMap::new()),
    };
    myself.register(Self::INTERNAL_ACTOR_METRICS, ActorMetrics::new(meter_provider))?;
    Ok(myself)
  }

  pub fn actor_metrics(&self) -> &ActorMetrics {
    &self.actor_metrics
  }

  pub fn actor_metrics_mut(&mut self) -> &mut ActorMetrics {
    &mut self.actor_metrics
  }

  pub fn register(&mut self, key: &str, instance: ActorMetrics) -> Result<(), MetricsError> {
    if self.known_metrics.contains_key(key) {
      return Err(MetricsError::DuplicateMetricKey(key.to_string()));
    }
    self.known_metrics.insert(key.to_string(), instance);
    Ok(())
  }

  pub fn get(&self, key: &str) -> Option<ActorMetrics> {
    self.known_metrics.get(key).map(|e| e.value().clone())
  }
}
