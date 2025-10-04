use std::sync::Arc;

pub use nexus_utils_std_rs::runtime::sync::{
  tokio_core_runtime, TokioMutex as StdAsyncMutex, TokioNotify as StdAsyncNotify, TokioRuntime,
  TokioRwLock as StdAsyncRwLock, TokioScheduler, TokioTimer,
};

pub async fn runtime_yield_now() {
  if let Some(yielder) = tokio_core_runtime().yielder() {
    yielder.yield_now().await;
  } else {
    tokio::task::yield_now().await;
  }
}

pub fn build_single_worker_runtime() -> Result<Arc<tokio::runtime::Runtime>, std::io::Error> {
  tokio::runtime::Builder::new_multi_thread()
    .worker_threads(1)
    .enable_all()
    .build()
    .map(Arc::new)
}
