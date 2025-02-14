use crate::actor::pid::Pid;
use crate::actor::process::Process;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct ProcessRegistry {
    host_id: String,
    next_id: AtomicU64,
    processes: Arc<DashMap<String, Arc<dyn Process>>>,
    address_lookup: Arc<DashMap<String, String>>,
}

impl ProcessRegistry {
    pub fn new(host_id: String) -> Self {
        Self {
            host_id,
            next_id: AtomicU64::new(0),
            processes: Arc::new(DashMap::new()),
            address_lookup: Arc::new(DashMap::new()),
        }
    }

    pub fn next_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("{}${}", self.host_id, id)
    }

    pub fn add(&self, id: String, process: Arc<dyn Process>) {
        self.processes.insert(id, process);
    }

    pub fn remove(&self, pid: &Pid) {
        if let Some(id) = &pid.id {
            self.processes.remove(id);
        }
    }

    pub fn get(&self, pid: &Pid) -> Option<Arc<dyn Process>> {
        pid.id.as_ref().and_then(|id| self.processes.get(id).map(|p| p.clone()))
    }

    pub fn set_address(&self, id: String, address: String) {
        self.address_lookup.insert(id, address);
    }

    pub fn get_address(&self, id: &str) -> Option<String> {
        self.address_lookup.get(id).map(|a| a.clone())
    }
}
