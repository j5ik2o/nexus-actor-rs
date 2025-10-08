use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use crate::runtime::context::{ActorContext, ActorHandlerFn, InternalActorRef, MapSystemFn};
use crate::runtime::guardian::{AlwaysRestart, Guardian, GuardianStrategy};
use crate::runtime::supervision::{CompositeEscalationSink, EscalationSink};
use crate::ActorId;
use crate::ActorPath;
use crate::FailureInfo;
use crate::Supervisor;
use crate::{MailboxFactory, MailboxOptions, PriorityEnvelope};
use crate::{MailboxSignal, SystemMessage};
use futures::future::select_all;
use futures::FutureExt;
use nexus_utils_core_rs::{Element, QueueError};

use super::actor_cell::ActorCell;

/// 優先度付きメールボックスを前提とした単純なスケジューラ実装。
pub struct PriorityScheduler<M, R, Strat = AlwaysRestart>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>, {
  runtime: R,
  pub(super) guardian: Guardian<M, R, Strat>,
  actors: Vec<ActorCell<M, R, Strat>>,
  escalations: Vec<FailureInfo>,
  escalation_sink: CompositeEscalationSink<M, R>,
}

#[allow(dead_code)]
impl<M, R> PriorityScheduler<M, R, AlwaysRestart>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(runtime: R) -> Self {
    Self {
      runtime: runtime.clone(),
      guardian: Guardian::new(AlwaysRestart),
      actors: Vec::new(),
      escalations: Vec::new(),
      escalation_sink: CompositeEscalationSink::new(),
    }
  }

  pub fn with_strategy<Strat>(runtime: R, strategy: Strat) -> PriorityScheduler<M, R, Strat>
  where
    Strat: GuardianStrategy<M, R>, {
    PriorityScheduler {
      runtime,
      guardian: Guardian::new(strategy),
      actors: Vec::new(),
      escalations: Vec::new(),
      escalation_sink: CompositeEscalationSink::new(),
    }
  }
}

#[allow(dead_code)]
impl<M, R, Strat> PriorityScheduler<M, R, Strat>
where
  M: Element,
  R: MailboxFactory + Clone,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
  Strat: GuardianStrategy<M, R>,
{
  pub fn spawn_actor<F, Sup>(
    &mut self,
    supervisor: Sup,
    options: MailboxOptions,
    map_system: Arc<MapSystemFn<M>>,
    handler: F,
  ) -> Result<InternalActorRef<M, R>, QueueError<PriorityEnvelope<M>>>
  where
    F: for<'ctx> FnMut(&mut ActorContext<'ctx, M, R, dyn Supervisor<M>>, M) + 'static,
    Sup: Supervisor<M>, {
    let (mailbox, sender) = self.runtime.build_mailbox::<PriorityEnvelope<M>>(options);
    let actor_sender = sender.clone();
    let handler_box: Box<ActorHandlerFn<M, R>> = Box::new(handler);
    let control_ref = InternalActorRef::new(actor_sender.clone());
    let watchers = vec![ActorId::ROOT];
    let primary_watcher = watchers.first().copied();
    let parent_path = ActorPath::new();
    let (actor_id, actor_path) =
      self
        .guardian
        .register_child(control_ref.clone(), map_system.clone(), primary_watcher, &parent_path)?;
    let cell = ActorCell::new(
      actor_id,
      map_system,
      watchers,
      actor_path,
      self.runtime.clone(),
      mailbox,
      sender,
      Box::new(supervisor),
      handler_box,
    );
    self.actors.push(cell);
    Ok(control_ref)
  }

  /// レガシーな同期 API。内部的には `dispatch_next` と同じ経路を使用するが、
  /// 新しいコードでは `run_until` / `dispatch_next` を推奨。
  #[deprecated(since = "3.1.0", note = "dispatch_next / run_until を使用してください")]
  pub fn dispatch_all(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    #[cfg(feature = "std")]
    {
      use core::sync::atomic::{AtomicBool, Ordering};
      static WARNED: AtomicBool = AtomicBool::new(false);
      if !WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
          "PriorityScheduler::dispatch_all は今後廃止予定です。dispatch_next / run_until の利用を検討してください。"
        );
      }
    }
    let _ = self.drain_ready_cycle()?;
    Ok(())
  }

  /// 条件が成立する限り `dispatch_next` を繰り返すヘルパ。ランタイム側で制御したい待機
  /// ループをシンプルに構築できる。
  pub async fn run_until<F>(&mut self, mut should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool, {
    while should_continue() {
      self.dispatch_next().await?;
    }
    Ok(())
  }

  /// スケジューラを非同期タスクとして常駐させる。`tokio::spawn(async move { scheduler.run_forever().await })`
  /// のように利用でき、停止はエラー発生またはタスクキャンセルで行う。
  pub async fn run_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<M>>> {
    loop {
      self.dispatch_next().await?;
    }
  }

  /// `std` 環境向け。`dispatch_next` をブロックしつつループし、アプリケーションが指定する
  /// 条件で停止できる。
  #[cfg(feature = "std")]
  pub fn blocking_dispatch_loop<F>(&mut self, mut should_continue: F) -> Result<(), QueueError<PriorityEnvelope<M>>>
  where
    F: FnMut() -> bool, {
    while should_continue() {
      futures::executor::block_on(self.dispatch_next())?;
    }
    Ok(())
  }

  /// `dispatch_next` を無限にブロック実行する。明示的な停止条件が不要なシンプルな常駐用途向け。
  #[cfg(feature = "std")]
  pub fn blocking_dispatch_forever(&mut self) -> Result<Infallible, QueueError<PriorityEnvelope<M>>> {
    loop {
      futures::executor::block_on(self.dispatch_next())?;
    }
  }

  pub async fn dispatch_next(&mut self) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    loop {
      if self.drain_ready_cycle()? {
        return Ok(());
      }

      let Some(index) = self.wait_for_any_signal().await else {
        return Ok(());
      };

      if self.process_waiting_actor(index).await? {
        return Ok(());
      }
    }
  }

  pub fn actor_count(&self) -> usize {
    self.actors.len()
  }

  pub fn take_escalations(&mut self) -> Vec<FailureInfo> {
    core::mem::take(&mut self.escalations)
  }

  /// Ready キューに存在するメッセージを 1 サイクル分処理する。処理が行われた場合は true。
  pub fn drain_ready(&mut self) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    self.drain_ready_cycle()
  }

  pub fn on_escalation<F>(&mut self, handler: F)
  where
    F: FnMut(&FailureInfo) -> Result<(), QueueError<PriorityEnvelope<M>>> + 'static, {
    self.escalation_sink.set_custom_handler(handler);
  }

  pub fn set_parent_guardian(&mut self, control_ref: InternalActorRef<M, R>, map_system: Arc<MapSystemFn<M>>) {
    self.escalation_sink.set_parent_guardian(control_ref, map_system);
  }

  pub fn set_root_escalation_handler(&mut self, handler: Option<crate::FailureEventHandler>) {
    self.escalation_sink.set_root_handler(handler);
  }

  pub fn set_root_event_listener(&mut self, listener: Option<crate::FailureEventListener>) {
    self.escalation_sink.set_root_listener(listener);
  }

  fn handle_escalations(&mut self) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    if self.escalations.is_empty() {
      return Ok(false);
    }

    let pending = core::mem::take(&mut self.escalations);
    let mut remaining = Vec::new();
    let mut handled = false;
    for info in pending.into_iter() {
      let handled_locally = self.forward_to_local_parent(&info);
      match self.escalation_sink.handle(info, handled_locally) {
        Ok(()) => handled = true,
        Err(unhandled) => remaining.push(unhandled),
      }
    }
    self.escalations = remaining;
    Ok(handled)
  }

  async fn wait_for_any_signal(&self) -> Option<usize> {
    if self.actors.is_empty() {
      return None;
    }

    let mut waiters = Vec::with_capacity(self.actors.len());
    for (idx, cell) in self.actors.iter().enumerate() {
      let signal = cell.signal_clone();
      waiters.push(
        async move {
          signal.wait().await;
          idx
        }
        .boxed_local(),
      );
    }

    let (idx, _, _) = select_all(waiters).await;
    Some(idx)
  }

  fn drain_ready_cycle(&mut self) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    let mut new_children = Vec::new();
    let len = self.actors.len();
    let mut processed_any = false;
    for idx in 0..len {
      let cell = &mut self.actors[idx];
      if cell.process_pending(&mut self.guardian, &mut new_children, &mut self.escalations)? > 0 {
        processed_any = true;
      }
    }
    self.finish_cycle(new_children, processed_any)
  }

  async fn process_waiting_actor(&mut self, index: usize) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    if index >= self.actors.len() {
      return Ok(false);
    }

    let mut new_children = Vec::new();
    let processed = self.actors[index]
      .wait_and_process(&mut self.guardian, &mut new_children, &mut self.escalations)
      .await?
      > 0;

    self.finish_cycle(new_children, processed)
  }

  fn finish_cycle(
    &mut self,
    new_children: Vec<ActorCell<M, R, Strat>>,
    processed_any: bool,
  ) -> Result<bool, QueueError<PriorityEnvelope<M>>> {
    if !new_children.is_empty() {
      self.actors.extend(new_children);
    }

    let handled = self.handle_escalations()?;
    Ok(processed_any || handled)
  }

  fn forward_to_local_parent(&self, info: &FailureInfo) -> bool {
    if let Some(parent_info) = info.escalate_to_parent() {
      if parent_info.path.is_empty() {
        return false;
      }

      if let Some((parent_ref, map_system)) = self.guardian.child_route(parent_info.actor) {
        let envelope =
          PriorityEnvelope::from_system(SystemMessage::Escalate(parent_info.clone())).map(|sys| (map_system)(sys));
        if parent_ref.sender().try_send(envelope).is_ok() {
          return true;
        }
      }
    }

    false
  }
}
