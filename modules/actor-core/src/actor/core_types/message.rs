#![allow(clippy::module_name_repetitions)]

use core::any::{type_name, Any};
use core::fmt::Debug;

#[cfg(feature = "alloc")]
use alloc::string::String;

use nexus_utils_core_rs::collections::DEFAULT_PRIORITY;

pub trait Message: Debug + Send + Sync + 'static {
  fn get_priority(&self) -> i8 {
    DEFAULT_PRIORITY
  }

  fn eq_message(&self, other: &dyn Message) -> bool;

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);

  #[cfg(feature = "alloc")]
  fn get_type_name(&self) -> String {
    String::from(type_name::<Self>())
  }
}

pub trait NotInfluenceReceiveTimeout: Message {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveTimeout;

impl Message for ReceiveTimeout {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().downcast_ref::<ReceiveTimeout>().is_some()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl NotInfluenceReceiveTimeout for ReceiveTimeout {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TerminateReason {
  Stopped = 0,
  AddressTerminated = 1,
  NotFound = 2,
}

impl TerminateReason {
  pub const fn as_str_name(&self) -> &'static str {
    match self {
      TerminateReason::Stopped => "Stopped",
      TerminateReason::AddressTerminated => "AddressTerminated",
      TerminateReason::NotFound => "NotFound",
    }
  }

  pub fn from_str_name(value: &str) -> Option<Self> {
    match value {
      "Stopped" => Some(Self::Stopped),
      "AddressTerminated" => Some(Self::AddressTerminated),
      "NotFound" => Some(Self::NotFound),
      _ => None,
    }
  }
}

macro_rules! impl_message_for_primitive {
  ($($ty:ty),* $(,)?) => {
    $(
      impl Message for $ty {
        fn eq_message(&self, other: &dyn Message) -> bool {
          other.as_any().downcast_ref::<$ty>().map_or(false, |value| value == self)
        }

        fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
          self
        }
      }
    )*
  };
}

impl_message_for_primitive!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64, bool);

#[cfg(feature = "alloc")]
impl Message for String {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<String>()
      .map_or(false, |value| value == self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
