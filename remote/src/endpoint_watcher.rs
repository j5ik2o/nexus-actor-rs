use crate::messages::{EndpointEvent, RemoteTerminate, RemoteUnwatch, RemoteWatch};
use crate::metrics::record_sender_snapshot;
use crate::remote::Remote;
use crate::serializer::SerializerId;
use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{ContextHandle, InfoPart, MessagePart, StopperPart};
use nexus_actor_core_rs::actor::core::{Actor, ActorError, ExtendedPid, PidSet};
use nexus_actor_core_rs::actor::message::{MessageHandle, SystemMessage};
use nexus_actor_core_rs::actor::process::Process;
use nexus_actor_core_rs::generated::actor::{Terminated, TerminatedReason, Unwatch, Watch};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct EndpointWatcher {
  remote: Weak<Remote>,
  address: String,
  watched: Arc<DashMap<String, PidSet>>,
  state: Arc<RwLock<State>>,
}

#[derive(Debug, Clone, PartialEq)]
enum State {
  Connected,
  Terminated,
}

impl EndpointWatcher {
  pub fn new(remote: Weak<Remote>, address: String) -> Self {
    EndpointWatcher {
      remote,
      address,
      watched: Arc::new(DashMap::new()),
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

  pub fn get_address(&self) -> String {
    self.address.clone()
  }

  pub fn get_watched(&self) -> Arc<DashMap<String, PidSet>> {
    self.watched.clone()
  }

  async fn initialize(&mut self) -> Result<(), ActorError> {
    Ok(())
  }

  async fn connected(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let system = self.get_actor_system();
    let _ = record_sender_snapshot(&ctx);
    let msg = if let Some(handle) = ctx.try_get_message_handle_opt() {
      handle
    } else {
      ctx.get_message_handle_opt().await.expect("message not found")
    };
    if let Some(remote_terminate) = msg.to_typed::<RemoteTerminate>() {
      let watcher_id = remote_terminate.watcher.clone().unwrap().id.clone();
      let watchee_opt = remote_terminate.watchee;
      if let Some(mut entry) = self.watched.get_mut(&watcher_id) {
        if let Some(watchee) = &watchee_opt {
          entry.remove(watchee).await;
        }
        if entry.is_empty().await {
          self.watched.remove(&watcher_id);
        }
        let why = TerminatedReason::Stopped as i32;
        let msg = Terminated { who: watchee_opt, why };
        if let Some(ref_process) = system.get_process_registry().await.get_local_process(&watcher_id).await {
          let pid = remote_terminate.watcher.unwrap();
          let pid = ExtendedPid::new(pid);
          ref_process
            .send_system_message(&pid, MessageHandle::new(SystemMessage::Terminate(msg)))
            .await;
        }
      }
    }
    if let Some(endpoint_event) = msg.to_typed::<EndpointEvent>() {
      if endpoint_event.is_terminated() {
        tracing::debug!(
          address = %self.address,
          watched_entries = self.watched.len(),
          "EndpointWatcher handling terminated"
        );
        for entry in self.watched.iter() {
          let (id, pid_set) = entry.pair();
          if let Some(ref_process) = system.get_process_registry().await.get_local_process(id).await {
            for pid in pid_set.to_vec().await.iter() {
              let why = TerminatedReason::AddressTerminated as i32;
              let msg = Terminated {
                who: Some(pid.clone()),
                why,
              };
              let pid = ExtendedPid::new(pid.clone());
              ref_process
                .send_system_message(&pid, MessageHandle::new(SystemMessage::Terminate(msg)))
                .await;
            }
          }
        }
        self.watched.clear();
        {
          let mut state = self.state.write().await;
          *state = State::Terminated;
        }
        ctx.stop(&ctx.get_self().await).await;
      }
    }
    if let Some(remote_watch) = msg.to_typed::<RemoteWatch>() {
      let watcher_id = remote_watch.watcher.clone().id.clone();
      let watchee = remote_watch.watchee.clone();
      if let Some(mut entry) = self.watched.get_mut(&watcher_id) {
        entry.add(watchee.clone()).await;
      } else {
        let mut pid_set = PidSet::new().await;
        pid_set.add(watchee.clone()).await;
        self.watched.insert(watcher_id.clone(), pid_set);
      }
      let u = SystemMessage::Watch(Watch {
        watcher: Some(remote_watch.watcher),
      });
      self
        .remote
        .upgrade()
        .unwrap()
        .send_message(watchee, None, MessageHandle::new(u), None, SerializerId::None)
        .await;
    }
    if let Some(remote_un_watch) = msg.to_typed::<RemoteUnwatch>() {
      let watcher_id = remote_un_watch.watcher.clone().id.clone();
      let watchee = remote_un_watch.watchee.clone();
      if let Some(mut entry) = self.watched.get_mut(&watcher_id) {
        entry.remove(&watchee).await;
        if entry.is_empty().await {
          self.watched.remove(&watcher_id);
        }
      }
      let w = SystemMessage::Unwatch(Unwatch {
        watcher: Some(remote_un_watch.watcher),
      });
      self
        .remote
        .upgrade()
        .unwrap()
        .send_message(watchee, None, MessageHandle::new(w), None, SerializerId::None)
        .await;
    }
    Ok(())
  }

  async fn terminated(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let system = self.get_actor_system();
    let _ = record_sender_snapshot(&ctx);
    let msg = if let Some(handle) = ctx.try_get_message_handle_opt() {
      handle
    } else {
      ctx.get_message_handle_opt().await.expect("message not found")
    };
    if let Some(remote_watch) = msg.to_typed::<RemoteWatch>() {
      let watcher_id = remote_watch.watcher.clone().id.clone();
      let watchee = remote_watch.watchee.clone();
      if let Some(ref_process) = system.get_process_registry().await.get_local_process(&watcher_id).await {
        let why = TerminatedReason::AddressTerminated as i32;
        let msg = Terminated {
          who: Some(watchee.clone()),
          why,
        };
        let pid = ExtendedPid::new(watchee.clone());
        ref_process
          .send_system_message(&pid, MessageHandle::new(SystemMessage::Terminate(msg)))
          .await;
      }
    }
    if let Some(endpoint_event) = msg.to_typed::<EndpointEvent>() {
      if endpoint_event.is_connected() {
        {
          let mut state = self.state.write().await;
          *state = State::Connected;
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
    self.initialize().await
  }
}

#[cfg(test)]
mod tests;
