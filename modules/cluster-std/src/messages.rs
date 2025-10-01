use nexus_actor_std_rs::actor::message::Message;
use nexus_message_derive_rs::Message as MessageDerive;

use crate::virtual_actor::VirtualActorContext;

/// Virtual Actor の起動時に Cluster から配送される初期化メッセージ。
#[derive(Debug, Clone, MessageDerive)]
pub struct ClusterInit {
  context: VirtualActorContext,
}

impl ClusterInit {
  pub fn new(context: VirtualActorContext) -> Self {
    Self { context }
  }

  pub fn context(&self) -> &VirtualActorContext {
    &self.context
  }
}

impl PartialEq for ClusterInit {
  fn eq(&self, other: &Self) -> bool {
    self.context.identity() == other.context.identity()
  }
}

impl Eq for ClusterInit {}
