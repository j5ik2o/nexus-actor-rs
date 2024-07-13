use time::OffsetDateTime;

use crate::log::log::LogLevel;
use crate::log::log_caller::LogCallerInfo;
use crate::log::log_field::LogField;

#[derive(Debug, Clone)]
pub struct LogEvent {
  pub time: OffsetDateTime,
  pub level: LogLevel,
  pub prefix: String,
  pub caller: Option<LogCallerInfo>,
  pub message: String,
  pub context: Vec<LogField>,
  pub fields: Vec<LogField>,
}

impl LogEvent {
  pub fn new(level: LogLevel, message: String) -> Self {
    LogEvent {
      time: OffsetDateTime::now_utc(),
      level,
      prefix: String::new(),
      caller: Some(LogCallerInfo::new(2)), // スキップ数は適宜調整が必要
      message,
      context: Vec::new(),
      fields: Vec::new(),
    }
  }

  pub fn add_field(&mut self, field: LogField) {
    self.fields.push(field);
  }

  pub fn add_context(&mut self, field: LogField) {
    self.context.push(field);
  }
}
