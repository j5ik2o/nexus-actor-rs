use time::OffsetDateTime;

use crate::log::caller::CallerInfo;
use crate::log::field::Field;
use crate::log::log::Level;

#[derive(Debug, Clone)]
pub struct LogEvent {
  pub time: OffsetDateTime,
  pub level: Level,
  pub prefix: String,
  pub caller: Option<CallerInfo>,
  pub message: String,
  pub context: Vec<Field>,
  pub fields: Vec<Field>,
}

impl LogEvent {
  pub fn new(level: Level, message: String) -> Self {
    LogEvent {
      time: OffsetDateTime::now_utc(),
      level,
      prefix: String::new(),
      caller: Some(CallerInfo::new(2)), // スキップ数は適宜調整が必要
      message,
      context: Vec::new(),
      fields: Vec::new(),
    }
  }

  pub fn add_field(&mut self, field: Field) {
    self.fields.push(field);
  }

  pub fn add_context(&mut self, field: Field) {
    self.context.push(field);
  }
}
