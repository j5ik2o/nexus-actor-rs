use nexus_actor_utils_rs::collections::DEFAULT_PRIORITY;
use std::any::Any;
use std::fmt::Debug;

pub trait Message: Debug + Send + Sync + 'static {
  fn get_priority(&self) -> i8 {
    DEFAULT_PRIORITY
  }
  fn eq_message(&self, other: &dyn Message) -> bool;
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);

  fn get_type_name(&self) -> String;
}

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

// tests/test.rs
#[cfg(test)]
mod tests {
    use super::*;
    use nexus_actor_message_derive_rs::Message;
    #[derive(Debug, Clone, PartialEq, Message)]
  pub struct Hello {
    pub who: String,
  }

  #[test]
  fn test_message_derive() {
    let msg1 = Hello {
      who: "World".to_string(),
    };
    let msg2 = Hello {
      who: "World".to_string(),
    };
    let msg3 = Hello {
      who: "Rust".to_string(),
    };

    assert!(msg1.eq_message(&msg2));
    assert!(!msg1.eq_message(&msg3));
  }
}
