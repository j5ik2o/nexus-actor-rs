use std::any::TypeId;
use std::time::Duration;

use crate::log::log_caller::LogCallerInfo;

pub trait LogEncoder {
  fn encode_bool(&mut self, key: &str, val: bool);
  fn encode_float64(&mut self, key: &str, val: f64);
  fn encode_int(&mut self, key: &str, val: i32);
  fn encode_int64(&mut self, key: &str, val: i64);
  fn encode_duration(&mut self, key: &str, val: Duration);
  fn encode_uint(&mut self, key: &str, val: u32);
  fn encode_uint64(&mut self, key: &str, val: u64);
  fn encode_string(&mut self, key: &str, val: &str);
  fn encode_object(&mut self, key: &str, val: &dyn std::any::Any);
  fn encode_type(&mut self, key: &str, val: TypeId);
  fn encode_caller(&mut self, key: &str, val: &LogCallerInfo);
}
