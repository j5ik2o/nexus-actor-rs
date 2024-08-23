use crate::actor::actor::Actor;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::Context;
use crate::actor::MetricsProvider;
use crate::extensions::{next_extension_id, Extension, ExtensionId};
use crate::metrics::ProtoMetrics;
use once_cell::sync::Lazy;
use opentelemetry::metrics::MetricsError;
use opentelemetry::KeyValue;
use std::sync::Arc;

static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(|| next_extension_id());

#[derive(Debug)]
pub struct Metrics {
  metrics: Option<ProtoMetrics>,
  enabled: bool,
  actor_system: ActorSystem,
}

impl Extension for Metrics {
  fn extension_id(&self) -> ExtensionId {
    *EXTENSION_ID
  }
}

impl Metrics {
  pub fn new(system: ActorSystem, metric_provider: Option<Arc<MetricsProvider>>) -> Result<Self, MetricsError> {
    match metric_provider {
      Some(mp) => Ok(Metrics {
        metrics: Some(ProtoMetrics::new(mp)?),
        enabled: true,
        actor_system: system,
      }),
      None => Ok(Metrics {
        metrics: None,
        enabled: false,
        actor_system: system,
      }),
    }
  }

  pub fn enabled(&self) -> bool {
    self.enabled
  }

  pub fn prepare_mailbox_length_gauge(&mut self) {
    if let Some(metrics) = &mut self.metrics {
      let meter = opentelemetry::global::meter("nexus_actor");
      match meter
        .i64_observable_gauge("nexus_actor_actor_mailbox_length")
        .with_description("Actor's Mailbox Length")
        .with_unit("1")
        .try_init()
      {
        Ok(gauge) => {
          metrics.instruments_mut().set_actor_mailbox_length_gauge(gauge);
        }
        Err(err) => {
          let error_msg = format!("Failed to create ActorMailBoxLength instrument: {}", err);
          tracing::error!("{}", error_msg);
        }
      }
    }
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
}
