use std::io::{self, Write};
use std::sync::Arc;

use once_cell::sync::Lazy;
use time::OffsetDateTime;
use tokio::sync::{mpsc, Mutex};

use crate::log::log::Level;
use crate::log::log_caller::LogCallerInfo;
use crate::log::log_encoder::LogEncoder;
use crate::log::log_event::LogEvent;
use crate::log::log_event_stream::{unsubscribe_stream, LOG_EVENT_STREAM};
use crate::log::log_subscription::LogSubscription;

pub struct IoLogger {
  sender: mpsc::Sender<LogEvent>,
  out: Arc<Mutex<Box<dyn Write + Send>>>,
}

static GLOBAL_LOGGER: Lazy<Mutex<Option<Arc<IoLogger>>>> = Lazy::new(|| Mutex::new(None));
static NO_STD_ERR_LOGS: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static SUB: Lazy<Mutex<Option<Arc<LogSubscription>>>> = Lazy::new(|| Mutex::new(None));

pub async fn set_no_std_err_logs() {
  let subscriptions_count = LOG_EVENT_STREAM.subscriptions.read().await.len();
  if subscriptions_count >= 2 {
    let mut no_std_err_logs = NO_STD_ERR_LOGS.lock().await;
    *no_std_err_logs = true;
  }
}

pub async fn init() {
  let (sender, receiver) = mpsc::channel(100);
  let logger = Arc::new(IoLogger {
    sender,
    out: Arc::new(Mutex::new(Box::new(io::stderr()))),
  });

  {
    let mut global_logger = GLOBAL_LOGGER.lock().await;
    *global_logger = Some(logger.clone());
  }

  let logger_for_subscriber = logger.clone();
  reset_subscription_with(move |evt: LogEvent| {
    let logger_for_subscriber = logger_for_subscriber.clone();
    async move {
      let _ = logger_for_subscriber.sender.try_send(evt);
    }
  })
  .await;

  tokio::spawn(async move {
    listen_event(receiver, logger.out.clone()).await;
  });
}

pub async fn reset_no_std_err_logs() {
  let mut no_std_err_logs = NO_STD_ERR_LOGS.lock().await;
  *no_std_err_logs = false;
}

pub async fn reset_global_logger() {
  let mut logger = GLOBAL_LOGGER.lock().await;
  *logger = None;
}

pub async fn reset_subscription_with<F, Fut>(f: F)
where
  F: Fn(LogEvent) -> Fut + Send + Sync + 'static,
  Fut: futures::Future<Output = ()> + Send + 'static, {
  let mut sub = SUB.lock().await;
  if let Some(old_sub) = sub.take() {
    unsubscribe_stream(&old_sub).await;
  }
  *sub = Some(LOG_EVENT_STREAM.subscribe(f).await);
}

pub async fn reset_subscription() {
  let mut sub = SUB.lock().await;
  if let Some(old_sub) = sub.take() {
    unsubscribe_stream(&old_sub).await;
  }
  *sub = None;
}

async fn listen_event(mut receiver: mpsc::Receiver<LogEvent>, out: Arc<Mutex<Box<dyn Write + Send>>>) {
  while let Some(event) = receiver.recv().await {
    if *NO_STD_ERR_LOGS.lock().await {
      if let Some(sub) = SUB.lock().await.take() {
        LOG_EVENT_STREAM.unsubscribe(&sub).await;
      }
      break;
    }
    write_event(&event, &out).await;
  }
}

async fn write_event(event: &LogEvent, out: &Arc<Mutex<Box<dyn Write + Send>>>) {
  let mut buf = Vec::new();
  format_header(&mut buf, &event.prefix, event.time, event.level);
  if let Some(caller) = &event.caller {
    if caller.line > 0 {
      format_caller(&mut buf, caller);
    }
  }
  if !event.message.is_empty() {
    buf.extend_from_slice(event.message.as_bytes());
    buf.push(b' ');
  }

  let mut encoder = IoEncoder { writer: &mut buf };
  for field in &event.context {
    field.encode(&mut encoder);
    encoder.write_space();
  }
  for field in &event.fields {
    field.encode(&mut encoder);
    encoder.write_space();
  }
  encoder.write_newline();

  let mut out = out.lock().await;
  out.write_all(&buf).unwrap();
  out.flush().unwrap();
}

fn format_header(buf: &mut Vec<u8>, prefix: &str, time: OffsetDateTime, level: Level) {
  write!(buf, "{} {} ", time, level).unwrap();
  if !prefix.is_empty() {
    buf.extend_from_slice(prefix.as_bytes());
  }
  buf.push(b'\t');
}

fn format_caller(buf: &mut Vec<u8>, caller: &LogCallerInfo) {
  let fname = caller.short_file_name();
  write!(buf, "{}:{}", fname, caller.line).unwrap();
  let v = 32 - fname.len();
  if v > 16 {
    buf.extend_from_slice(&[b'\t', b'\t', b'\t']);
  } else if v > 8 {
    buf.extend_from_slice(&[b'\t', b'\t']);
  } else {
    buf.push(b'\t');
  }
}

struct IoEncoder<'a> {
  writer: &'a mut Vec<u8>,
}

impl<'a> IoEncoder<'a> {
  fn write_space(&mut self) {
    self.writer.push(b' ');
  }

  fn write_newline(&mut self) {
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

pub async fn log(event: LogEvent) {
  if let Some(logger) = GLOBAL_LOGGER.lock().await.as_ref() {
    let _ = logger.sender.try_send(event);
  }
}
