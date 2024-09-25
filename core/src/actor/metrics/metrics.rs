use crate::actor::actor::Actor;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::Context;
use crate::extensions::{next_extension_id, Extension, ExtensionId};
use crate::metrics::{ActorMetrics, ProtoMetrics};
use once_cell::sync::Lazy;
use opentelemetry::metrics::MetricsError;
use opentelemetry::KeyValue;
use std::any::Any;

pub static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(|| next_extension_id());

#[derive(Debug, Clone)]
pub struct Metrics {
  proto_metrics: Option<ProtoMetrics>,
  enabled: bool,
  actor_system: ActorSystem,
}

impl Extension for Metrics {
  fn extension_id(&self) -> ExtensionId {
    *EXTENSION_ID
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

impl Metrics {
  pub async fn new(system: ActorSystem) -> Result<Self, MetricsError> {
    match system.clone().get_config().await.metrics_provider {
      Some(mp) => Ok(Metrics {
        proto_metrics: Some(ProtoMetrics::new(mp)?),
        enabled: true,
        actor_system: system,
      }),
      None => Ok(Metrics {
        proto_metrics: None,
        enabled: false,
        actor_system: system,
      }),
    }
  }

  pub fn get_proto_metrics(&self) -> Option<&ProtoMetrics> {
    self.proto_metrics.as_ref()
  }

  pub fn is_enabled(&self) -> bool {
    self.enabled
  }

  pub async fn common_labels(&self, ctx: &impl Context) -> Vec<KeyValue> {
    vec![
      KeyValue::new("address", self.actor_system.get_address().await.to_string()),
      KeyValue::new(
        "actor_type",
        format!(
          "{}",
          ctx
            .get_actor()
            .await
            .map_or_else(|| "".to_string(), |e| e.get_type_name())
        )
        .replace("*", ""),
      ),
    ]
  }

  pub async fn foreach<F, Fut>(&mut self, f: F)
  where
    F: Fn(&ActorMetrics, &Metrics) -> Fut,
    Fut: std::future::Future<Output = ()>, {
    if self.is_enabled() {
      if let Some(pm) = self.get_proto_metrics() {
        if let Some(am) = pm.get(ProtoMetrics::INTERNAL_ACTOR_METRICS) {
          f(&am, self).await;
        }
      }
    }
  }
}
