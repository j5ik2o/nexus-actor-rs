#![allow(unused)]

// Minimal, forward-looking core API for remote messaging.
// Intentionally std-free; compiles under `no_std + alloc`.

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

use core::future::Future;
use core::pin::Pin;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;

/// Utility alias for boxed futures.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ————————————— Transport —————————————

/// Remote endpoint descriptor (scheme/host/port, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportEndpoint {
  pub uri: String,
}

impl TransportEndpoint {
  pub fn new(uri: String) -> Self {
    Self { uri }
  }
}

/// Remote listener configuration (bind address, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportListener {
  pub bind: String,
}

impl TransportListener {
  pub fn new(bind: String) -> Self {
    Self { bind }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportErrorKind {
  Unavailable,
  Rejected,
  Connection,
  Protocol,
  Timeout,
  Other,
}

#[derive(Debug, Clone)]
pub struct TransportError {
  kind: TransportErrorKind,
  detail: Option<&'static str>,
}

impl TransportError {
  pub fn new(kind: TransportErrorKind) -> Self {
    Self { kind, detail: None }
  }

  pub fn with_detail(kind: TransportErrorKind, detail: &'static str) -> Self {
    Self {
      kind,
      detail: Some(detail),
    }
  }

  pub fn kind(&self) -> TransportErrorKind {
    self.kind
  }

  pub fn detail(&self) -> Option<&'static str> {
    self.detail
  }
}

/// Opaque handle to an established endpoint connection.
pub trait EndpointHandle: Send + Sync {
  fn close(&self);
}

/// Abstract transport interface, to be implemented by std/embedded layers.
pub trait RemoteTransport: Send + Sync + 'static {
  type Handle: EndpointHandle;

  fn connect(&self, endpoint: &TransportEndpoint) -> BoxFuture<'_, Result<Self::Handle, TransportError>>;
  fn serve(&self, listener: TransportListener) -> BoxFuture<'_, Result<(), TransportError>>;
}

// ————————————— Runtime/Registry —————————————

/// Serializer registry abstraction (protobuf/json/etc.).
pub trait SerializerRegistry: Send + Sync {}

/// Block list storage for allow/deny decisions.
pub trait BlockListStore: Send + Sync {
  fn is_blocked(&self, system: &str) -> bool;
}

/// Metrics sink abstraction for transports/remoting.
pub trait MetricsSink: Send + Sync {}

#[derive(Clone)]
pub struct RemoteRuntimeConfig<T: RemoteTransport> {
  pub transport: Arc<T>,
  pub serializer: Option<Arc<dyn SerializerRegistry>>,
  pub block_list: Option<Arc<dyn BlockListStore>>,
  pub metrics: Option<Arc<dyn MetricsSink>>,
}

impl<T: RemoteTransport> RemoteRuntimeConfig<T> {
  pub fn new(transport: Arc<T>) -> Self {
    Self {
      transport,
      serializer: None,
      block_list: None,
      metrics: None,
    }
  }

  pub fn with_serializer(mut self, reg: Arc<dyn SerializerRegistry>) -> Self {
    self.serializer = Some(reg);
    self
  }

  pub fn with_block_list(mut self, store: Arc<dyn BlockListStore>) -> Self {
    self.block_list = Some(store);
    self
  }

  pub fn with_metrics(mut self, sink: Arc<dyn MetricsSink>) -> Self {
    self.metrics = Some(sink);
    self
  }
}

/// Minimal runtime wrapper to carry core dependencies for remoting.
pub struct RemoteRuntime<T: RemoteTransport> {
  config: RemoteRuntimeConfig<T>,
}

impl<T: RemoteTransport> RemoteRuntime<T> {
  pub fn new(config: RemoteRuntimeConfig<T>) -> Self {
    Self { config }
  }

  pub fn transport(&self) -> Arc<T> {
    self.config.transport.clone()
  }

  pub fn serializer(&self) -> Option<Arc<dyn SerializerRegistry>> {
    self.config.serializer.clone()
  }

  pub fn block_list(&self) -> Option<Arc<dyn BlockListStore>> {
    self.config.block_list.clone()
  }

  pub fn metrics(&self) -> Option<Arc<dyn MetricsSink>> {
    self.config.metrics.clone()
  }
}
