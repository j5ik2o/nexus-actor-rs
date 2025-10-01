use crate::messages::{EndpointEvent, RemoteTerminate, RemoteUnwatch, RemoteWatch};
use crate::metrics::record_sender_snapshot;
use crate::remote::Remote;
use crate::serializer::SerializerId;
use crate::watch_registry::WatchRegistry;
use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, InfoPart, MessagePart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ErrorReason, ExtendedPid, PidSet};
use nexus_actor_std_rs::actor::message::{MessageHandle, SystemMessage, TerminateReason};
use nexus_actor_std_rs::actor::process::Process;
use nexus_actor_std_rs::generated::actor::Pid;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct EndpointWatcher {
  remote: Weak<Remote>,
  address: String,
  registry: Arc<WatchRegistry>,
  state: Arc<RwLock<State>>,
}

#[derive(Debug, Clone, PartialEq)]
enum State {
  Connected,
  Terminated,
}

impl EndpointWatcher {
  pub fn new(remote: Weak<Remote>, address: String) -> Self {
    Self::with_registry(remote, address, Arc::new(WatchRegistry::new()))
  }

  pub fn with_registry(remote: Weak<Remote>, address: String, registry: Arc<WatchRegistry>) -> Self {
    EndpointWatcher {
      remote,
      address,
      registry,
      state: Arc::new(RwLock::new(State::Connected)),
    }
  }

  pub fn get_actor_system(&self) -> ActorSystem {
    self
      .remote
      .upgrade()
      .expect("Remote has been dropped")
      .get_actor_system()
      .clone()
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub fn get_address(&self) -> String {
    self.address.clone()
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub fn get_watched(&self) -> Arc<DashMap<String, PidSet>> {
    self.registry.inner()
  }

  /// 監視ピッド集合を取得する。ベンチマーク／テスト用途向けに公開。
  pub fn get_pid_set(&self, watcher_id: &str) -> Option<PidSet> {
    self.registry.get_pid_set(watcher_id)
  }

  /// 監視状態のスナップショットを取得する。ベンチマーク／テスト用途向けに公開。
  pub fn watched_snapshot(&self) -> Vec<(String, PidSet)> {
    self.registry.snapshot()
  }

  /// 指定ウォッチャーが空であればマップから削除する。
  pub async fn prune_if_empty(&self, watcher_id: &str) -> bool {
    self.registry.prune_if_empty(watcher_id).await
  }

  /// 当該ウォッチャーに Watchee を追加する。既に存在する場合はノップ。
  pub async fn add_watch_pid(&self, watcher_id: &str, watchee: Pid) {
    let _ = self.registry.watch(watcher_id, watchee).await;
  }

  /// 当該ウォッチャーから Watchee を削除し、空になればクリーンアップする。
  pub async fn remove_watch_pid(&self, watcher_id: &str, watchee: &Pid) -> bool {
    self
      .registry
      .unwatch(watcher_id, watchee)
      .await
      .map(|stat| stat.changed)
      .unwrap_or(false)
  }

  pub fn registry(&self) -> Arc<WatchRegistry> {
    self.registry.clone()
  }

  async fn initialize(&mut self) -> Result<(), ActorError> {
    Ok(())
  }

  async fn connected(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let system = self.get_actor_system();
    let sender_snapshot = record_sender_snapshot(&ctx).await;
    let msg = if let Some(handle) = ctx.try_get_message_handle_opt() {
      handle
    } else {
      ctx.get_message_handle_opt().await.expect("message not found")
    };
    if let Some(remote_terminate) = msg.to_typed::<RemoteTerminate>() {
      let watcher_pid = remote_terminate
        .watcher
        .clone()
        .or_else(|| sender_snapshot.as_ref().map(|pid| pid.to_pid()))
        .ok_or_else(|| ActorError::ReceiveError(ErrorReason::new("watcher pid missing", 0)))?;
      let watcher_id = watcher_pid.id.clone();
      let watchee_opt = remote_terminate.watchee.clone();

      let watchee_ref = watchee_opt.as_ref();
      let _ = self.registry.remove_watchee(&watcher_id, watchee_ref).await;

      let terminated_message =
        SystemMessage::terminate(watchee_opt.as_ref().map(Pid::to_core), TerminateReason::Stopped);
      if let Some(ref_process) = system.get_process_registry().await.get_local_process(&watcher_id).await {
        let pid = ExtendedPid::new(watcher_pid);
        ref_process
          .send_system_message(&pid, MessageHandle::new(terminated_message))
          .await;
      }
    }
    if let Some(endpoint_event) = msg.to_typed::<EndpointEvent>() {
      if endpoint_event.is_terminated() {
        tracing::debug!(
          address = %self.address,
          watched_entries = self.registry.len(),
          "EndpointWatcher handling terminated"
        );
        for (id, pid_set) in self.watched_snapshot() {
          if let Some(ref_process) = system.get_process_registry().await.get_local_process(&id).await {
            for pid in pid_set.to_vec().await.iter() {
              let terminated_message =
                SystemMessage::terminate(Some(pid.to_core()), TerminateReason::AddressTerminated);
              let pid = ExtendedPid::new(pid.clone());
              ref_process
                .send_system_message(&pid, MessageHandle::new(terminated_message))
                .await;
            }
          }
        }
        self.registry.clear().await;
        {
          let mut state = self.state.write().await;
          *state = State::Terminated;
        }
        if let Some(remote) = self.remote.upgrade() {
          if let Some(manager) = remote.get_endpoint_manager_opt().await {
            manager.unregister_watch_registry(&self.address);
          }
        }
        ctx.stop(&ctx.get_self().await).await;
      }
    }
    if let Some(remote_watch) = msg.to_typed::<RemoteWatch>() {
      let watcher = remote_watch.watcher.clone();
      let watcher_id = watcher.id.clone();
      let watchee = remote_watch.watchee.clone();
      self.add_watch_pid(&watcher_id, watchee.clone()).await;
      let message = SystemMessage::watch(watcher.to_core());
      self
        .remote
        .upgrade()
        .unwrap()
        .send_message(watchee, None, MessageHandle::new(message), None, SerializerId::None)
        .await;
    }
    if let Some(remote_un_watch) = msg.to_typed::<RemoteUnwatch>() {
      let watcher = remote_un_watch.watcher.clone();
      let watcher_id = watcher.id.clone();
      let watchee = remote_un_watch.watchee.clone();
      self.remove_watch_pid(&watcher_id, &watchee).await;
      let message = SystemMessage::unwatch(watcher.to_core());
      self
        .remote
        .upgrade()
        .unwrap()
        .send_message(watchee, None, MessageHandle::new(message), None, SerializerId::None)
        .await;
    }
    Ok(())
  }

  async fn terminated(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let system = self.get_actor_system();
    let _ = record_sender_snapshot(&ctx).await;
    let msg = if let Some(handle) = ctx.try_get_message_handle_opt() {
      handle
    } else {
      ctx.get_message_handle_opt().await.expect("message not found")
    };
    if let Some(remote_watch) = msg.to_typed::<RemoteWatch>() {
      let watcher_id = remote_watch.watcher.clone().id.clone();
      let watchee = remote_watch.watchee.clone();
      if let Some(ref_process) = system.get_process_registry().await.get_local_process(&watcher_id).await {
        let terminated_message =
          SystemMessage::terminate(Some(watchee.clone().to_core()), TerminateReason::AddressTerminated);
        let pid = ExtendedPid::new(watchee.clone());
        ref_process
          .send_system_message(&pid, MessageHandle::new(terminated_message))
          .await;
      }
    }
    if let Some(endpoint_event) = msg.to_typed::<EndpointEvent>() {
      if endpoint_event.is_connected() {
        {
          let mut state = self.state.write().await;
          *state = State::Connected;
        }
        if let Some(remote) = self.remote.upgrade() {
          if let Some(manager) = remote.get_endpoint_manager_opt().await {
            manager.register_watch_registry(&self.address, self.registry());
          }
        }
      }
    }

    Ok(())
  }
}

#[async_trait]
impl Actor for EndpointWatcher {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let state = {
      let state = self.state.read().await;
      state.clone()
    };
    match state {
      State::Connected => self.connected(context_handle).await,
      State::Terminated => self.terminated(context_handle).await,
    }
  }

  async fn post_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    if let Some(remote) = self.remote.upgrade() {
      if let Some(manager) = remote.get_endpoint_manager_opt().await {
        manager.register_watch_registry(&self.address, self.registry());
      }
    }
    self.initialize().await
  }
}

#[cfg(test)]
mod tests;
