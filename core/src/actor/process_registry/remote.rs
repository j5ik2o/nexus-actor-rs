use crate::actor::pid::Pid;
use crate::actor::process::Process;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct RemoteProcess {
    pid: Pid,
    address: String,
}

impl RemoteProcess {
    pub fn new(pid: Pid, address: String) -> Self {
        Self { pid, address }
    }

    pub fn get_pid(&self) -> &Pid {
        &self.pid
    }

    pub fn get_address(&self) -> &str {
        &self.address
    }
}

#[derive(Debug)]
pub struct RemoteProcessRegistry {
    processes: Arc<DashMap<String, Arc<RemoteProcess>>>,
    addresses: Arc<DashMap<String, String>>,
    status: Arc<RwLock<bool>>,
}

impl RemoteProcessRegistry {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(DashMap::new()),
            addresses: Arc::new(DashMap::new()),
            status: Arc::new(RwLock::new(true)),
        }
    }

    pub async fn register(&self, pid: Pid, address: String) {
        let process = Arc::new(RemoteProcess::new(pid.clone(), address.clone()));
        self.processes.insert(pid.id.unwrap(), process);
        self.addresses.insert(address.clone(), address);
    }

    pub fn get_process(&self, pid: &Pid) -> Option<Arc<RemoteProcess>> {
        pid.id.as_ref().and_then(|id| self.processes.get(id).map(|p| p.clone()))
    }

    pub fn remove(&self, pid: &Pid) {
        if let Some(id) = &pid.id {
            if let Some((_, process)) = self.processes.remove(id) {
                self.addresses.remove(process.get_address());
            }
        }
    }

    pub async fn is_available(&self) -> bool {
        *self.status.read().await
    }

    pub async fn set_available(&self, available: bool) {
        *self.status.write().await = available;
    }
}
