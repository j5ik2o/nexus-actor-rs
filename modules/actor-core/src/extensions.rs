#![cfg(feature = "alloc")]

#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as SharedArc;
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as SharedArc;
use alloc::vec::Vec;
use core::any::Any;
use core::fmt::{self, Debug, Formatter};
use portable_atomic::{AtomicI32, Ordering};

use nexus_utils_core_rs::sync::ArcShared;
use spin::RwLock;

/// 一意な Extension を識別するための ID 型。
pub type ExtensionId = i32;

static NEXT_EXTENSION_ID: AtomicI32 = AtomicI32::new(0);

/// 新しい ExtensionId を払い出します。
#[must_use]
pub fn next_extension_id() -> ExtensionId {
  NEXT_EXTENSION_ID.fetch_add(1, Ordering::SeqCst)
}

/// ActorSystem に組み込まれる拡張の共通インターフェース。
pub trait Extension: Any + Send + Sync {
  /// 拡張固有の ID を返します。
  fn extension_id(&self) -> ExtensionId;
}

/// 登録済み Extension 群を管理するスロットコンテナ。
pub struct Extensions {
  slots: ArcShared<RwLock<Vec<Option<ArcShared<dyn Extension>>>>>,
}

impl Extensions {
  /// 空の Extension レジストリを生成します。
  #[must_use]
  pub fn new() -> Self {
    Self {
      slots: ArcShared::new(RwLock::new(Vec::new())),
    }
  }

  /// 既存スロット共有体からレジストリを作成します。
  #[must_use]
  pub fn from_shared(slots: ArcShared<RwLock<Vec<Option<ArcShared<dyn Extension>>>>>) -> Self {
    Self { slots }
  }

  /// 型付き Extension を登録します。
  pub fn register<E>(&self, extension: ArcShared<E>)
  where
    E: Extension, {
    let id = extension.extension_id();
    if id < 0 {
      return;
    }
    let idx = id as usize;
    let arc: SharedArc<E> = extension.into_arc();
    let trait_arc: SharedArc<dyn Extension> = arc;
    let handle = ArcShared::from_arc(trait_arc);
    let mut guard = self.slots.write();
    if guard.len() <= idx {
      guard.resize_with(idx + 1, || None);
    }
    guard[idx] = Some(handle);
  }

  /// Trait オブジェクトとして Extension を登録します。
  pub fn register_dyn(&self, extension: ArcShared<dyn Extension>) {
    let id = extension.extension_id();
    if id < 0 {
      return;
    }
    let idx = id as usize;
    let mut guard = self.slots.write();
    if guard.len() <= idx {
      guard.resize_with(idx + 1, || None);
    }
    guard[idx] = Some(extension);
  }

  /// 指定 ID の Extension を取得します。
  #[must_use]
  pub fn get(&self, id: ExtensionId) -> Option<ArcShared<dyn Extension>> {
    if id < 0 {
      return None;
    }
    let guard = self.slots.read();
    guard.get(id as usize).and_then(|slot| slot.as_ref().cloned())
  }

  /// 指定 ID の Extension に対してクロージャを適用します。
  pub fn with<E, F, R>(&self, id: ExtensionId, f: F) -> Option<R>
  where
    E: Extension,
    F: FnOnce(&E) -> R, {
    let guard = self.slots.read();
    guard.get(id as usize).and_then(|slot| {
      slot.as_ref().and_then(|handle| {
        let ext = &**handle as &dyn Any;
        ext.downcast_ref::<E>().map(f)
      })
    })
  }
}

impl Default for Extensions {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for Extensions {
  fn clone(&self) -> Self {
    Self {
      slots: self.slots.clone(),
    }
  }
}

impl Debug for Extensions {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let guard = self.slots.read();
    f.debug_struct("Extensions")
      .field("len", &guard.len())
      .finish()
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use super::*;
  use super::SharedArc;

  #[derive(Debug)]
  struct DummyExtension {
    id: ExtensionId,
    value: usize,
  }

  impl DummyExtension {
    fn new(value: usize) -> Self {
      Self {
        id: next_extension_id(),
        value,
      }
    }
  }

  impl Extension for DummyExtension {
    fn extension_id(&self) -> ExtensionId {
      self.id
    }
  }

  #[test]
  fn register_and_lookup_extension() {
    let extensions = Extensions::new();
    let extension = ArcShared::from_arc(SharedArc::new(DummyExtension::new(42)));
    let id = extension.extension_id();
    extensions.register(extension.clone());

    let stored = extensions.get(id).expect("extension should exist");
    assert_eq!(stored.extension_id(), id);

    let value = extensions
      .with::<DummyExtension, _, _>(id, |ext| ext.value)
      .expect("typed borrow");
    assert_eq!(value, 42);
  }
}
