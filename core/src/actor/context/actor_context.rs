use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
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
};

#[async_trait]
pub trait ActorContext: Send + Sync + 'static {
    async fn parent(&self) -> Option<Pid>;
    async fn self_pid(&self) -> Pid;
    async fn actor_system(&self) -> Arc<RwLock<ActorSystem>>;
    
    async fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
    async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
    
    async fn watch(&self, pid: &Pid);
    async fn unwatch(&self, pid: &Pid);
    
    async fn set_receive_timeout(&self, duration: Duration);
    async fn cancel_receive_timeout(&self);
    
    async fn forward(&self, pid: &Pid, message: MessageHandle);
    async fn forward_system(&self, pid: &Pid, message: MessageHandle);
    
    async fn stop(&self, pid: &Pid);
    async fn poison_pill(&self, pid: &Pid);
    
    async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>);
}

pub struct ActorContextImpl {
    parent: Option<Pid>,
    self_pid: Pid,
    actor_system: Arc<RwLock<ActorSystem>>,
    process: ProcessHandle,
}

#[async_trait]
impl ActorContext for ActorContextImpl {
    async fn parent(&self) -> Option<Pid> {
        self.parent.clone()
    }

    async fn self_pid(&self) -> Pid {
        self.self_pid.clone()
    }

    async fn actor_system(&self) -> Arc<RwLock<ActorSystem>> {
        self.actor_system.clone()
    }

    async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
        let system = self.actor_system.read().await;
        system.spawn(props).await
    }

    async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
        let system = self.actor_system.read().await;
        system.spawn_prefix(props, prefix).await
    }

    async fn watch(&self, pid: &Pid) {
        let system = self.actor_system.read().await;
        system.watch(&self.self_pid, pid).await;
    }

    async fn unwatch(&self, pid: &Pid) {
        let system = self.actor_system.read().await;
        system.unwatch(&self.self_pid, pid).await;
    }

    async fn set_receive_timeout(&self, duration: Duration) {
        self.process.set_receive_timeout(duration).await;
    }

    async fn cancel_receive_timeout(&self) {
        self.process.cancel_receive_timeout().await;
    }

    async fn forward(&self, pid: &Pid, message: MessageHandle) {
        pid.send_user_message(&*self.process, Box::new(message)).await;
    }

    async fn forward_system(&self, pid: &Pid, message: MessageHandle) {
        pid.send_system_message(&*self.process, Box::new(message)).await;
    }

    async fn stop(&self, pid: &Pid) {
        pid.stop(&*self.process).await;
    }

    async fn poison_pill(&self, pid: &Pid) {
        use crate::actor::message::PoisonPill;
        pid.send_system_message(&*self.process, Box::new(PoisonPill)).await;
    }

    async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) {
        let system = self.actor_system.read().await;
        system.handle_failure(who, error, message).await;
    }
}
