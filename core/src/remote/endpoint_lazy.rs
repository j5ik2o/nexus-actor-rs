use crate::remote::endpoint::Endpoint;
use crate::remote::endpoint_manager::EndpointManager;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct EndpointLazy {
  endpoint: Option<Arc<Mutex<Endpoint>>>,
  endpoint_manager: EndpointManager,
}
