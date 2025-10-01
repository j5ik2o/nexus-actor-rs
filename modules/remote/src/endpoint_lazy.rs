use crate::endpoint::Endpoint;
use crate::endpoint_manager::EndpointManager;
use nexus_actor_std_rs::actor::context::SenderPart;
use nexus_actor_std_rs::actor::core::ExtendedPid;
use nexus_actor_std_rs::actor::message::MessageHandle;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct EndpointLazy {
  unloaded: Arc<AtomicBool>,
  endpoint: Arc<RwLock<Option<Endpoint>>>,
  endpoint_manager: EndpointManager,
  address: String,
}

impl EndpointLazy {
  pub fn new(endpoint_manager: EndpointManager, address: &str) -> Self {
    tracing::debug!(">>> EndpointLazy::new: address={}", address);
    Self {
      unloaded: Arc::new(AtomicBool::new(false)),
      endpoint: Arc::new(RwLock::new(None)),
      endpoint_manager,
      address: address.to_string(),
    }
  }

  pub(crate) fn get_unloaded(&self) -> Arc<AtomicBool> {
    self.unloaded.clone()
  }

  pub(crate) async fn get(&self) -> Result<Endpoint, Box<dyn std::error::Error>> {
    let mut endpoint = self.endpoint.write().await;
    if let Some(ep) = endpoint.as_ref() {
      Ok(ep.clone())
    } else {
      let new_endpoint = self.connect().await?;
      *endpoint = Some(new_endpoint.clone());
      Ok(new_endpoint)
    }
  }

  async fn connect(&self) -> Result<Endpoint, Box<dyn std::error::Error>> {
    let manager = &self.endpoint_manager;
    let system = manager.get_actor_system().await;
    let pid = ExtendedPid::new(manager.get_endpoint_supervisor().await);
    let af = system
      .get_root_context()
      .await
      .request_future(pid, MessageHandle::new(self.address.clone()), Duration::from_secs(5))
      .await;
    let ep = af
      .result()
      .await
      .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let result = ep.to_typed::<Endpoint>().ok_or_else(|| {
      Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Failed to convert to Endpoint",
      )) as Box<dyn std::error::Error>
    });
    tracing::debug!("connected, address = {}", self.address);
    result
  }
}
