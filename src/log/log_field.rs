use std::any::Any;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::log::log_encoder::LogEncoder;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogFieldType {
  Unknown,
  Bool,
  F64,
  I32,
  I64,
  Duration,
  U32,
  U64,
  String,
  Stringer,
  Error,
  Object,
  TypeOf,
  Skip,
  Caller,
}

#[derive(Debug, Clone)]
pub struct LogField {
  key: String,
  field_type: LogFieldType,
  val: i64,
  str: String,
  obj: Option<Arc<dyn Any + Send + Sync>>,
}

impl PartialEq for LogField {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && self.field_type == other.field_type && self.val == other.val && self.str == other.str
  }
}

impl LogField {
  pub fn bool(key: &str, val: bool) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::Bool,
      val: if val { 1 } else { 0 },
      str: String::new(),
      obj: None,
    }
  }

  pub fn f64(key: &str, val: f64) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::F64,
      val: val.to_bits() as i64,
      str: String::new(),
      obj: None,
    }
  }

  pub fn i32(key: &str, val: i32) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::I32,
      val: val as i64,
      str: String::new(),
      obj: None,
    }
  }

  pub fn i64(key: &str, val: i64) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::I64,
      val,
      str: String::new(),
      obj: None,
    }
  }

  pub fn u32(key: &str, val: u32) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::U32,
      val: val as i64,
      str: String::new(),
      obj: None,
    }
  }

  pub fn u64(key: &str, val: u64) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::U64,
      val: val as i64,
      str: String::new(),
      obj: None,
    }
  }

  pub fn string(key: &str, val: &str) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::String,
      val: 0,
      str: val.to_string(),
      obj: None,
    }
  }

  pub fn display<T: fmt::Display + Send + Sync + 'static>(key: &str, val: T) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::Stringer,
      val: 0,
      str: String::new(),
      obj: Some(Arc::new(val)),
    }
  }

  pub fn time(key: &str, val: SystemTime) -> Self {
    let duration = val.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = duration.as_secs_f64();
    Self::f64(key, seconds)
  }

  pub fn error(err: &dyn Error) -> Self {
    LogField {
      key: "error".to_string(),
      field_type: LogFieldType::Error,
      val: 0,
      str: String::new(),
      obj: Some(Arc::new(err.to_string())),
    }
  }

  pub fn duration(key: &str, val: Duration) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::Duration,
      val: val.as_nanos() as i64,
      str: String::new(),
      obj: None,
    }
  }

  pub fn object<T: Send + Sync + 'static>(key: &str, val: T) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::Object,
      val: 0,
      str: String::new(),
      obj: Some(Arc::new(val)),
    }
  }

  pub fn type_of<T: 'static>(key: &str, _: &T) -> Self {
    LogField {
      key: key.to_string(),
      field_type: LogFieldType::TypeOf,
      val: 0,
      str: String::new(),
      obj: Some(Arc::new(std::any::TypeId::of::<T>())),
    }
  }

  pub fn message<T: Send + Sync + 'static>(val: T) -> Self {
    Self::object("message", val)
  }

  // CallerSkip と Caller の実装はRustでは異なるアプローチが必要です。
  // 例えば、backtrace クレートを使用することができます。

  pub fn encode(&self, enc: &mut dyn LogEncoder) {
    match self.field_type {
      LogFieldType::Bool => enc.encode_bool(&self.key, self.val != 0),
      LogFieldType::F64 => enc.encode_float64(&self.key, f64::from_bits(self.val as u64)),
      LogFieldType::I32 => enc.encode_int(&self.key, self.val as i32),
      LogFieldType::I64 => enc.encode_int64(&self.key, self.val),
      LogFieldType::U32 => enc.encode_uint(&self.key, self.val as u32),
      LogFieldType::U64 => enc.encode_uint64(&self.key, self.val as u64),
      LogFieldType::String => enc.encode_string(&self.key, &self.str),
      LogFieldType::Duration => enc.encode_duration(&self.key, Duration::from_nanos(self.val as u64)),
      LogFieldType::Stringer => {
        if let Some(obj) = &self.obj {
          if let Some(stringer) = obj.downcast_ref::<Box<dyn fmt::Display>>() {
            enc.encode_string(&self.key, &stringer.to_string());
          }
        }
      }
      LogFieldType::Error => {
        if let Some(obj) = &self.obj {
          if let Some(err_str) = obj.downcast_ref::<String>() {
            enc.encode_string(&self.key, err_str);
          }
        }
      }
      LogFieldType::Object => {
        if let Some(obj) = &self.obj {
          enc.encode_object(&self.key, obj.as_ref());
        }
      }
      LogFieldType::TypeOf => {
        if let Some(obj) = &self.obj {
          if let Some(type_id) = obj.downcast_ref::<std::any::TypeId>() {
            enc.encode_type(&self.key, *type_id);
          }
        }
      }
      LogFieldType::Caller => {
        // CallerInfo の実装が必要です
      }
      LogFieldType::Skip => {}
      LogFieldType::Unknown => panic!("unknown field type found"),
    }
  }
}
