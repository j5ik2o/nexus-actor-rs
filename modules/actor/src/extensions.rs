use nexus_utils_std_rs::concurrent::Synchronized;
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

type ExtensionHandle = Arc<Mutex<dyn Extension + Send + Sync + 'static>>;
type ExtensionSlots = Synchronized<Vec<Option<ExtensionHandle>>>;

#[derive(Debug, Clone)]
pub struct Extensions {
  extensions: Arc<ExtensionSlots>,
}

impl Extensions {
  pub fn new() -> Self {
    Extensions {
      extensions: Arc::new(Synchronized::new((0..100).map(|_| None).collect())),
    }
  }

  pub async fn get(&self, id: ExtensionId) -> Option<ExtensionHandle> {
    self
      .extensions
      .read(|slots| slots.get(id as usize).and_then(|opt| opt.clone()))
      .await
  }

  pub async fn register(&mut self, extension: ExtensionHandle) {
    let id = {
      let mg = extension.lock().await;
      mg.extension_id() as usize
    };
    self
      .extensions
      .write(|slots| {
        if id >= slots.len() {
          slots.resize_with(id + 1, || None);
        }
        slots[id] = Some(extension.clone());
      })
      .await;
  }
}

impl Default for Extensions {
  fn default() -> Self {
    Self::new()
  }
}

pub fn next_extension_id() -> ExtensionId {
  CURRENT_ID.fetch_add(1, Ordering::SeqCst)
}
