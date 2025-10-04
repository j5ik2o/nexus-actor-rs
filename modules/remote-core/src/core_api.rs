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

use nexus_actor_core_rs::{CoreJoinHandle, CoreRuntime, CoreSpawnError, CoreTaskFuture};

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

/// 抽象的なリモートトランスポート層。
///
/// 実装は `no_std + alloc` 環境でも動作し、I/O の詳細を [`RemoteRuntime`] から隠蔽します。
/// 長時間ブロックする処理は [`RemoteRuntime::spawn`] 経由で [`CoreRuntime`] のスケジューラに委譲してください。
pub trait RemoteTransport: Send + Sync + 'static {
  type Handle: EndpointHandle;

  /// 指定したエンドポイントへ接続し、ハンドルを返却します。
  /// 非同期に実行され、エラー種別は [`TransportErrorKind`] で表現します。
  fn connect(&self, endpoint: &TransportEndpoint) -> BoxFuture<'_, Result<Self::Handle, TransportError>>;

  /// リスナーを起動し、受信ループを開始します。
  /// 失敗時には [`TransportErrorKind::Unavailable`] などで状態を表現します。
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
  core: CoreRuntime,
  transport: Arc<T>,
  serializer: Option<Arc<dyn SerializerRegistry>>,
  block_list: Option<Arc<dyn BlockListStore>>,
  metrics: Option<Arc<dyn MetricsSink>>,
}

impl<T: RemoteTransport> RemoteRuntimeConfig<T> {
  pub fn new(core: CoreRuntime, transport: Arc<T>) -> Self {
    Self {
      core,
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

  pub fn core(&self) -> CoreRuntime {
    self.core.clone()
  }

  pub fn transport(&self) -> Arc<T> {
    self.transport.clone()
  }

  pub fn serializer(&self) -> Option<Arc<dyn SerializerRegistry>> {
    self.serializer.clone()
  }

  pub fn block_list(&self) -> Option<Arc<dyn BlockListStore>> {
    self.block_list.clone()
  }

  pub fn metrics(&self) -> Option<Arc<dyn MetricsSink>> {
    self.metrics.clone()
  }
}

/// Minimal runtime wrapper to carry core dependencies for remoting.
#[derive(Clone)]
pub struct RemoteRuntime<T: RemoteTransport> {
  config: RemoteRuntimeConfig<T>,
}

impl<T: RemoteTransport> RemoteRuntime<T> {
  pub fn new(config: RemoteRuntimeConfig<T>) -> Self {
    Self { config }
  }

  pub fn core(&self) -> CoreRuntime {
    self.config.core()
  }

  pub fn transport(&self) -> Arc<T> {
    self.config.transport()
  }

  pub fn serializer(&self) -> Option<Arc<dyn SerializerRegistry>> {
    self.config.serializer()
  }

  pub fn block_list(&self) -> Option<Arc<dyn BlockListStore>> {
    self.config.block_list()
  }

  pub fn metrics(&self) -> Option<Arc<dyn MetricsSink>> {
    self.config.metrics()
  }

  /// [`CoreRuntime::spawner`] を介してタスクを実行します。
  ///
  /// 戻り値の [`CoreJoinHandle`] は、必要に応じてキャンセルや join に利用できます。
  pub fn spawn<F>(&self, future: F) -> Result<Arc<dyn CoreJoinHandle>, CoreSpawnError>
  where
    F: Future<Output = ()> + Send + 'static, {
    let task: CoreTaskFuture = Box::pin(future);
    self.config.core().spawner().spawn(task)
  }
}
