use alloc::boxed::Box;
use core::any::{Any, TypeId};
use core::fmt::{self, Debug};

use nexus_utils_core_rs::Element;

/// ランタイム内部で使用する型消去済みメッセージ。
pub struct DynMessage {
  inner: Box<dyn Any + Send + Sync>,
}

impl DynMessage {
  /// 任意の値をラップした `DynMessage` を生成する。
  pub fn new<T>(value: T) -> Self
  where
    T: Any + Send + Sync, {
    Self { inner: Box::new(value) }
  }

  /// 内部に保持している値の `TypeId` を取得する。
  pub fn type_id(&self) -> TypeId {
    self.inner.as_ref().type_id()
  }

  /// 所有権を移動させながら型 T へのダウンキャストを試みる。
  pub fn downcast<T>(self) -> Result<T, Self>
  where
    T: Any + Send + Sync, {
    match self.inner.downcast::<T>() {
      Ok(boxed) => Ok(*boxed),
      Err(inner) => Err(Self { inner }),
    }
  }

  /// 参照を通じて型 T へのダウンキャストを試みる。
  pub fn downcast_ref<T>(&self) -> Option<&T>
  where
    T: Any + Send + Sync, {
    self.inner.downcast_ref::<T>()
  }

  /// 可変参照を通じて型 T へのダウンキャストを試みる。
  pub fn downcast_mut<T>(&mut self) -> Option<&mut T>
  where
    T: Any + Send + Sync, {
    self.inner.downcast_mut::<T>()
  }

  /// 内部の型消去済み値を取り出す。
  pub fn into_any(self) -> Box<dyn Any + Send + Sync> {
    self.inner
  }
}

impl Debug for DynMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DynMessage<{}>", core::any::type_name::<Self>())
  }
}

impl Element for DynMessage {}
