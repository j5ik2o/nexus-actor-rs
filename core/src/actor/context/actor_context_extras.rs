use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{
    Message,
    MessageHandle,
    Process,
    ProcessHandle,
    Pid,
    ExtendedPid,
    ActorError,
    ActorHandle,
    Props,
    SpawnError,
    ActorSystem,
    ActorContext,
};

pub struct ActorContextExtras {
    context: Arc<RwLock<dyn ActorContext>>,
    watchers: Arc<RwLock<Vec<Pid>>>,
    children: Arc<RwLock<Vec<Pid>>>,
}

impl ActorContextExtras {
    pub fn new(context: Arc<RwLock<dyn ActorContext>>) -> Self {
        Self {
            context,
            watchers: Arc::new(RwLock::new(Vec::new())),
            children: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn add_watcher(&self, pid: &Pid) {
        let mut watchers = self.watchers.write().await;
        if !watchers.contains(pid) {
            watchers.push(pid.clone());
        }
    }

    pub async fn remove_watcher(&self, pid: &Pid) {
        let mut watchers = self.watchers.write().await;
        if let Some(pos) = watchers.iter().position(|x| x == pid) {
            watchers.remove(pos);
        }
    }

    pub async fn add_child(&self, pid: &Pid) {
        let mut children = self.children.write().await;
        if !children.contains(pid) {
            children.push(pid.clone());
        }
    }

    pub async fn remove_child(&self, pid: &Pid) {
        let mut children = self.children.write().await;
        if let Some(pos) = children.iter().position(|x| x == pid) {
            children.remove(pos);
        }
    }
}
