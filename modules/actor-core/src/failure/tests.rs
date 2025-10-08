use super::*;
use crate::ActorId;
use crate::ActorPath;

#[test]
fn escalation_stage_increments_with_parent_hops() {
  let root = ActorId(0);
  let child = ActorId(1);
  let grandchild = ActorId(2);

  let path = ActorPath::new()
    .push_child(root)
    .push_child(child)
    .push_child(grandchild);
  let failure = FailureInfo::new(grandchild, path, "boom".into());
  assert!(matches!(failure.stage, EscalationStage::Initial));

  let parent_failure = failure.escalate_to_parent().expect("parent exists");
  assert!(matches!(parent_failure.stage, EscalationStage::Escalated { hops: 1 }));

  let root_failure = parent_failure.escalate_to_parent().expect("root exists");
  assert!(matches!(root_failure.stage, EscalationStage::Escalated { hops: 2 }));
  assert_eq!(root_failure.path.segments(), &[root]);
}
