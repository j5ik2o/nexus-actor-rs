use std::sync::atomic::{AtomicI32, Ordering};

pub type ExtensionId = i32;

static CURRENT_ID: AtomicI32 = AtomicI32::new(0);

pub trait Extension {
  fn extension_id(&self) -> ExtensionId;
}

pub struct Extensions {
  extensions: Vec<Option<Box<dyn Extension>>>,
}

impl Extensions {
  pub fn new() -> Self {
    Extensions {
      extensions: (0..100).map(|_| None).collect(),
    }
  }

  pub fn get(&self, id: ExtensionId) -> Option<&dyn Extension> {
    self.extensions.get(id as usize).and_then(|opt| opt.as_deref())
  }

  pub fn register(&mut self, extension: Box<dyn Extension>) {
    let id = extension.extension_id() as usize;
    if id >= self.extensions.len() {
      self.extensions.resize_with(id + 1, || None);
    }
    self.extensions[id] = Some(extension);
  }
}

pub fn next_extension_id() -> ExtensionId {
  CURRENT_ID.fetch_add(1, Ordering::SeqCst)
}
