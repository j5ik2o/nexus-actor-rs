use std::fmt::Debug;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type ExtensionId = i32;

static CURRENT_ID: AtomicI32 = AtomicI32::new(0);

pub trait Extension: Debug {
  fn extension_id(&self) -> ExtensionId;

  fn as_any(&self) -> &dyn std::any::Any;

  fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

#[derive(Debug, Clone)]
pub struct Extensions {
  extensions: Arc<Mutex<Vec<Option<Arc<Mutex<dyn Extension + Send + Sync + 'static>>>>>>,
}

impl Extensions {
  pub fn new() -> Self {
    Extensions {
      extensions: Arc::new(Mutex::new((0..100).map(|_| None).collect())),
    }
  }

  pub async fn get(&self, id: ExtensionId) -> Option<Arc<Mutex<dyn Extension + Send + Sync + 'static>>> {
    // 戻り値の型を変更
    let lock = self.extensions.lock().await;
    let extension = lock.get(id as usize).and_then(|opt| opt.as_ref().map(|e| e.clone())); // map()を使用してクローン
    drop(lock);
    extension
  }

  pub async fn register(&mut self, extension: Arc<Mutex<dyn Extension + Send + Sync + 'static>>) {
    let id = {
      let mg = extension.lock().await;
      mg.extension_id() as usize
    };
    let mut lock = self.extensions.lock().await;
    if id >= lock.len() {
      lock.resize_with(id + 1, || None);
    }
    lock[id] = Some(extension);
  }
}

pub fn next_extension_id() -> ExtensionId {
  CURRENT_ID.fetch_add(1, Ordering::SeqCst)
}
