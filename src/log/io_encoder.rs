use crate::log::log_caller::LogCallerInfo;
use crate::log::log_encoder::LogEncoder;
use std::io::Write;

pub struct IoEncoder<'a> {
  writer: &'a mut Vec<u8>,
}

impl<'a> IoEncoder<'a> {
  pub fn new(writer: &'a mut Vec<u8>) -> Self {
    Self { writer }
  }

  pub fn write_space(&mut self) {
    self.writer.push(b' ');
  }

  pub fn write_newline(&mut self) {
    self.writer.push(b'\n');
  }
}

impl<'a> LogEncoder for IoEncoder<'a> {
  fn encode_bool(&mut self, key: &str, val: bool) {
    write!(self.writer, "{}={}", key, val).unwrap();
  }

  fn encode_float64(&mut self, key: &str, val: f64) {
    write!(self.writer, "{}={}", key, val).unwrap();
  }

  fn encode_int(&mut self, key: &str, val: i32) {
    write!(self.writer, "{}={}", key, val).unwrap();
  }

  fn encode_int64(&mut self, key: &str, val: i64) {
    write!(self.writer, "{}={}", key, val).unwrap();
  }

  fn encode_duration(&mut self, key: &str, val: std::time::Duration) {
    write!(self.writer, "{}={:?}", key, val).unwrap();
  }

  fn encode_uint(&mut self, key: &str, val: u32) {
    write!(self.writer, "{}={}", key, val).unwrap();
  }

  fn encode_uint64(&mut self, key: &str, val: u64) {
    write!(self.writer, "{}={}", key, val).unwrap();
  }

  fn encode_string(&mut self, key: &str, val: &str) {
    write!(self.writer, "{}={:?}", key, val).unwrap();
  }

  fn encode_object(&mut self, key: &str, val: &dyn std::any::Any) {
    write!(self.writer, "{}={:?}", key, val).unwrap();
  }

  fn encode_type(&mut self, key: &str, val: std::any::TypeId) {
    write!(self.writer, "{}={:?}", key, val).unwrap();
  }

  fn encode_caller(&mut self, key: &str, val: &LogCallerInfo) {
    let fname = val.short_file_name();
    write!(self.writer, "{}={}:{}", key, fname, val.line).unwrap();
  }
}
