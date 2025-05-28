use nexus_actor_utils_rs::collections::DEFAULT_PRIORITY;
use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

/// Core Message trait that all actor messages must implement
pub trait Message: Debug + Send + Sync + 'static {
  fn get_priority(&self) -> i8 {
    DEFAULT_PRIORITY
  }
  fn eq_message(&self, other: &dyn Message) -> bool;
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);
  fn get_type_name(&self) -> String;
}

/// Marker trait for messages that don't influence receive timeout
pub trait NotInfluenceReceiveTimeout: Message {}

/// Represents the receive timeout event
#[derive(Debug, Clone, PartialEq)]
pub struct ReceiveTimeout;

impl Message for ReceiveTimeout {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().downcast_ref::<ReceiveTimeout>().is_some()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl NotInfluenceReceiveTimeout for ReceiveTimeout {}

/// Reason for actor termination
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TerminateReason {
  Stopped = 0,
  AddressTerminated = 1,
  NotFound = 2,
}

impl TerminateReason {
  /// String value of the enum field names used in the ProtoBuf definition.
  ///
  /// The values are not transformed in any way and thus are considered stable
  /// (if the ProtoBuf definition does not change) and safe for programmatic use.
  pub fn as_str_name(&self) -> &'static str {
    match self {
      TerminateReason::Stopped => "Stopped",
      TerminateReason::AddressTerminated => "AddressTerminated",
      TerminateReason::NotFound => "NotFound",
    }
  }

  /// Creates an enum from field names used in the ProtoBuf definition.
  pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
    match value {
      "Stopped" => Some(Self::Stopped),
      "AddressTerminated" => Some(Self::AddressTerminated),
      "NotFound" => Some(Self::NotFound),
      _ => None,
    }
  }
}

// Basic implementations for primitive types
impl Message for i8 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<i8>() {
      Some(other_i8) => self == other_i8,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for u8 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<u8>() {
      Some(other_u8) => self == other_u8,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for i16 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<i16>() {
      Some(other_i16) => self == other_i16,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for u16 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<u16>() {
      Some(other_u16) => self == other_u16,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for i32 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<i32>() {
      Some(other_i32) => self == other_i32,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for u32 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<u32>() {
      Some(other_u32) => self == other_u32,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for i64 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<i64>() {
      Some(other_i64) => self == other_i64,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for u64 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<u64>() {
      Some(other_u64) => self == other_u64,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for f32 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<f32>() {
      Some(other_f32) => self == other_f32,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for f64 {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<f64>() {
      Some(other_f64) => self == other_f64,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for bool {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<bool>() {
      Some(other_bool) => self == other_bool,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl Message for String {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<String>() {
      Some(other_string) => self == other_string,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

// ReadonlyMessageHeaders trait and implementation
pub trait ReadonlyMessageHeaders: Debug + Send + Sync + 'static {
  fn get(&self, key: &str) -> Option<String>;
  fn keys(&self) -> Vec<String>;
  fn length(&self) -> usize;
  fn to_map(&self) -> HashMap<String, String>;
}

#[derive(Debug, Clone)]
pub struct ReadonlyMessageHeadersHandle(Arc<dyn ReadonlyMessageHeaders>);

impl ReadonlyMessageHeadersHandle {
  pub fn new_arc(header: Arc<dyn ReadonlyMessageHeaders>) -> Self {
    ReadonlyMessageHeadersHandle(header)
  }

  pub fn new(header: impl ReadonlyMessageHeaders + 'static) -> Self {
    ReadonlyMessageHeadersHandle(Arc::new(header))
  }
}

impl ReadonlyMessageHeaders for ReadonlyMessageHeadersHandle {
  fn get(&self, key: &str) -> Option<String> {
    self.0.get(key)
  }

  fn keys(&self) -> Vec<String> {
    self.0.keys()
  }

  fn length(&self) -> usize {
    self.0.length()
  }

  fn to_map(&self) -> HashMap<String, String> {
    self.0.to_map()
  }
}

impl PartialEq for ReadonlyMessageHeadersHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReadonlyMessageHeadersHandle {}