use crate::generated::cluster::Member;
use crate::member_state_delta::MemberStateDelta;
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Clone)]
pub struct LocalStateSender(Arc<dyn Fn(&MemberStateDelta, &Member)>);

impl Debug for LocalStateSender {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "LocalStateSender")
  }
}

pub trait GossipStateStorer {
  fn get_state(&self);
}
