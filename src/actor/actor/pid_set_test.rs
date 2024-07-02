use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::pid_set::PidSet;
use crate::actor::actor::Pid;
use crate::actor::actor_system::ActorSystem;

async fn new_pid(system: ActorSystem, address: &str, id: &str, request_id: u32) -> ExtendedPid {
  let pid = Pid {
    address: address.to_string(),
    id: id.to_string(),
    request_id,
  };
  ExtendedPid::new(pid, system)
}

#[tokio::test]
async fn test_pid_set_empty() {
  let s = PidSet::new().await;
  assert!(s.is_empty().await);
}

#[tokio::test]
async fn test_pid_set_clear() {
  let system = ActorSystem::new().await;
  let mut s = PidSet::new().await;
  s.add(new_pid(system.clone(), "local", "p1", 0).await).await;
  s.add(new_pid(system.clone(), "local", "p2", 0).await).await;
  s.add(new_pid(system.clone(), "local", "p3", 0).await).await;
  assert_eq!(3, s.len().await);
  s.clear().await;
  assert!(s.is_empty().await);
  // assert_eq!(0, s.pids.len());
}

#[tokio::test]
async fn test_pid_set_add_small() {
  let system = ActorSystem::new().await;
  let mut s = PidSet::new().await;
  let p1 = new_pid(system.clone(), "local", "p1", 0).await;
  s.add(p1).await;
  assert!(!s.is_empty().await);
  let p1 = new_pid(system, "local", "p1", 0).await;
  s.add(p1).await;
  assert_eq!(1, s.len().await);
}

#[tokio::test]
async fn test_pid_set_values() {
  let system = ActorSystem::new().await;
  let mut s = PidSet::new().await;
  s.add(new_pid(system.clone(), "local", "p1", 0).await).await;
  s.add(new_pid(system.clone(), "local", "p2", 0).await).await;
  s.add(new_pid(system.clone(), "local", "p3", 0).await).await;
  assert!(!s.is_empty().await);

  let r = s.to_vec().await;
  assert_eq!(3, r.len());
}

#[tokio::test]
async fn test_pid_set_add_map() {
  let system = ActorSystem::new().await;
  let mut s = PidSet::new().await;
  let p1 = new_pid(system.clone(), "local", "p1", 0).await;
  s.add(p1).await;
  assert!(!s.is_empty().await);
  let p1 = new_pid(system, "local", "p1", 0).await;
  s.add(p1).await;
  assert_eq!(1, s.len().await);
}

#[tokio::test]
async fn test_pid_set_add_remove() {
  let system = ActorSystem::new().await;
  let mut pids = vec![];
  for i in 0..1000 {
    let pid = new_pid(system.clone(), "local", &format!("p{}", i), 0).await;
    pids.push(pid);
  }

  let mut s = PidSet::new().await;
  for pid in &pids {
    s.add(pid.clone()).await;
  }
  assert_eq!(1000, s.len().await);

  for pid in &pids {
    assert!(s.remove(pid).await);
  }
  assert!(s.is_empty().await);
}
