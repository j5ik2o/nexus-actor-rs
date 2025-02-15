use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Pid, Process, ProcessHandle};

#[derive(Debug, Default)]
pub struct RemoteProcessRegistry {
    processes: Arc<RwLock<HashMap<u64, ProcessHandle>>>,
}

impl RemoteProcessRegistry {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, pid: Pid, process: ProcessHandle) {
        let mut processes = self.processes.write().await;
        processes.insert(pid.id, process);
    }

    pub async fn get_process(&self, pid: &Pid) -> Option<ProcessHandle> {
        let processes = self.processes.read().await;
        processes.get(&pid.id).cloned()
    }

    pub async fn remove(&self, pid: &Pid) {
        let mut processes = self.processes.write().await;
        processes.remove(&pid.id);
    }
}
