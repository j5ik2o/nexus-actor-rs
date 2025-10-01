use crate::actor::core::pid_set::PidSet;
use crate::generated::actor::Pid;

#[tokio::test]
async fn test_pid_set_empty() {
  let s = PidSet::new();
  assert!(s.is_empty().await);
}

#[tokio::test]
async fn test_pid_set_clear() {
  let s = PidSet::new();
  s.add(Pid::new("local", "p1")).await;
  s.add(Pid::new("local", "p2")).await;
  s.add(Pid::new("local", "p3")).await;
  assert_eq!(3, s.len().await);
  s.clear().await;
  assert!(s.is_empty().await);
}

#[tokio::test]
async fn test_pid_set_add_small() {
  let s = PidSet::new();
  let p1 = Pid::new("local", "p1");
  s.add(p1.clone()).await;
  assert!(!s.is_empty().await);
  s.add(p1).await;
  assert_eq!(1, s.len().await);
}

#[tokio::test]
async fn test_pid_set_values() {
  let s = PidSet::new();
  s.add(Pid::new("local", "p1")).await;
  s.add(Pid::new("local", "p2")).await;
  s.add(Pid::new("local", "p3")).await;
  assert!(!s.is_empty().await);

  let r = s.to_vec().await;
  assert_eq!(3, r.len());
}

#[tokio::test]
async fn test_pid_set_add_remove() {
  let s = PidSet::new();
  let mut pids = Vec::new();
  for i in 0..1000 {
    let pid = Pid::new("local", &format!("p{}", i));
    pids.push(pid.clone());
    s.add(pid).await;
  }
  assert_eq!(1000, s.len().await);

  for pid in &pids {
    assert!(s.remove(pid).await);
  }
  assert!(s.is_empty().await);
}

#[tokio::test]
async fn test_pid_set_same_id_different_address_are_distinct() {
  let set = PidSet::new();
  let local = Pid::new("node-a", "actor-1");
  let remote = Pid::new("node-b", "actor-1");

  set.add(local.clone()).await;
  set.add(remote.clone()).await;

  assert_eq!(2, set.len().await, "アドレスが異なる PID は別物として保持されるべき");
  assert!(set.contains(&local).await, "ローカル PID が見つからない");
  assert!(set.contains(&remote).await, "リモート PID が見つからない");
}
