//! Tokio ランタイム向けの ReceiveTimeout スケジューラ実装。
//!
//! `TokioDeadlineTimer` と優先度付きメールボックスを組み合わせ、アクターへ
//! `SystemMessage::ReceiveTimeout` を配信する仕組みを提供する。

use core::time::Duration;
use std::sync::Arc;

use futures::future::poll_fn;
use nexus_actor_core_rs::{
  DynMessage, MailboxFactory, MapSystemFn, PriorityEnvelope, QueueMailboxProducer, ReceiveTimeoutScheduler,
  ReceiveTimeoutSchedulerFactory, SystemMessage,
};
use nexus_utils_std_rs::{DeadlineTimer, DeadlineTimerExpired, DeadlineTimerKey, TimerDeadline, TokioDeadlineTimer};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use crate::TokioMailboxFactory;

/// Tokio メールボックスへ `PriorityEnvelope<DynMessage>` を送信するためのプロデューサ。
type TokioSender = QueueMailboxProducer<
  <TokioMailboxFactory as MailboxFactory>::Queue<PriorityEnvelope<DynMessage>>,
  <TokioMailboxFactory as MailboxFactory>::Signal,
>;

#[derive(Debug)]
enum Command {
  Set(Duration),
  Cancel,
  Reset,
  Shutdown,
}

struct TimerState {
  key: Option<DeadlineTimerKey>,
  duration: Option<Duration>,
}

impl TimerState {
  fn new() -> Self {
    Self {
      key: None,
      duration: None,
    }
  }
}

/// Tokio ランタイム上で `ReceiveTimeout` を駆動するスケジューラ。
///
/// 受信専用のタスクを起動し、`TokioDeadlineTimer` をポーリングしながら
/// 期限切れ時に `PriorityEnvelope<SystemMessage>` を優先度付きメールボックスへ送る。
/// `ActorCell` 側はタイマー実装を意識せずに `set` / `cancel` / `notify_activity` を呼び出すだけで済む。
pub struct TokioReceiveTimeoutScheduler {
  tx: UnboundedSender<Command>,
  handle: JoinHandle<()>,
}

impl TokioReceiveTimeoutScheduler {
  fn spawn_task(
    sender: TokioSender,
    map_system: Arc<MapSystemFn<DynMessage>>,
  ) -> (UnboundedSender<Command>, JoinHandle<()>) {
    let (tx, rx) = unbounded_channel();
    let handle = tokio::spawn(run_scheduler(rx, sender, map_system));
    (tx, handle)
  }
}

impl ReceiveTimeoutScheduler for TokioReceiveTimeoutScheduler {
  fn set(&mut self, duration: Duration) {
    let _ = self.tx.send(Command::Set(duration));
  }

  fn cancel(&mut self) {
    let _ = self.tx.send(Command::Cancel);
  }

  fn notify_activity(&mut self) {
    let _ = self.tx.send(Command::Reset);
  }
}

impl Drop for TokioReceiveTimeoutScheduler {
  fn drop(&mut self) {
    let _ = self.tx.send(Command::Shutdown);
    self.handle.abort();
  }
}

/// Tokio ランタイム向けの `ReceiveTimeoutSchedulerFactory` 実装。
///
/// 優先度付きメールボックスのプロデューサと SystemMessage 変換クロージャを受け取り、
/// 内部でスケジューラタスクを起動して `ReceiveTimeoutScheduler` を返す。
/// `install_receive_timeout_scheduler` から登録するだけで、Tokio ランタイムに `ReceiveTimeout` 支援を追加できる。
pub struct TokioReceiveTimeoutSchedulerFactory;

impl TokioReceiveTimeoutSchedulerFactory {
  /// 新しいファクトリを生成する。
  pub fn new() -> Self {
    Self
  }
}

impl Default for TokioReceiveTimeoutSchedulerFactory {
  fn default() -> Self {
    Self::new()
  }
}

impl ReceiveTimeoutSchedulerFactory<DynMessage, TokioMailboxFactory> for TokioReceiveTimeoutSchedulerFactory {
  fn create(&self, sender: TokioSender, map_system: Arc<MapSystemFn<DynMessage>>) -> Box<dyn ReceiveTimeoutScheduler> {
    let (tx, handle) = TokioReceiveTimeoutScheduler::spawn_task(sender, map_system);
    Box::new(TokioReceiveTimeoutScheduler { tx, handle })
  }
}

async fn wait_for_expired(timer: &mut TokioDeadlineTimer<()>) -> DeadlineTimerExpired<()> {
  poll_fn(|cx| timer.poll_expired(cx)).await.expect("poll expired")
}

async fn run_scheduler(
  mut commands: UnboundedReceiver<Command>,
  sender: TokioSender,
  map_system: Arc<MapSystemFn<DynMessage>>,
) {
  let mut timer = TokioDeadlineTimer::new();
  let mut state = TimerState::new();

  loop {
    tokio::select! {
      cmd = commands.recv() => {
        match cmd {
          Some(Command::Set(duration)) => {
            state.duration = Some(duration);
            match state.key {
              Some(key) => {
                let _ = timer.reset(key, TimerDeadline::from(duration));
              }
              None => {
                if let Ok(key) = timer.insert((), TimerDeadline::from(duration)) {
                  state.key = Some(key);
                }
              }
            }
          }
          Some(Command::Cancel) => {
            if let Some(key) = state.key.take() {
              let _ = timer.cancel(key);
            }
            state.duration = None;
          }
          Some(Command::Reset) => {
            if let (Some(key), Some(duration)) = (state.key, state.duration) {
              let _ = timer.reset(key, TimerDeadline::from(duration));
            }
          }
          Some(Command::Shutdown) | None => {
            break;
          }
        }
      }
      expired = wait_for_expired(&mut timer), if state.key.is_some() => {
        let _ = expired;
        state.key = None;
        let envelope = PriorityEnvelope::from_system(SystemMessage::ReceiveTimeout)
          .map(|sys| (map_system)(sys));
        let _ = sender.try_send(envelope);
        if let Some(duration) = state.duration {
          if let Ok(key) = timer.insert((), TimerDeadline::from(duration)) {
            state.key = Some(key);
          }
        }
      }
    }
  }
}
