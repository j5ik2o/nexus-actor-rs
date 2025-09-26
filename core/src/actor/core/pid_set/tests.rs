use crate::actor::core::pid_set::PidSet;
use crate::generated::actor::Pid;

#[test]
fn test_pid_set_empty() {
  let s = PidSet::new();
  assert!(s.is_empty());
}

#[test]
fn test_pid_set_clear() {
  let s = PidSet::new();
  s.add(Pid::new("local", "p1"));
  s.add(Pid::new("local", "p2"));
  s.add(Pid::new("local", "p3"));
  assert_eq!(3, s.len());
  s.clear();
  assert!(s.is_empty());
}

#[test]
fn test_pid_set_add_small() {
  let s = PidSet::new();
  let p1 = Pid::new("local", "p1");
  s.add(p1.clone());
  assert!(!s.is_empty());
  s.add(p1);
  assert_eq!(1, s.len());
}

#[test]
fn test_pid_set_values() {
  let s = PidSet::new();
  s.add(Pid::new("local", "p1"));
  s.add(Pid::new("local", "p2"));
  s.add(Pid::new("local", "p3"));
  assert!(!s.is_empty());

  let r = s.to_vec();
  assert_eq!(3, r.len());
}

#[test]
fn test_pid_set_add_remove() {
  let s = PidSet::new();
  let mut pids = Vec::new();
  for i in 0..1000 {
    let pid = Pid::new("local", &format!("p{}", i));
    pids.push(pid.clone());
    s.add(pid);
  }
  assert_eq!(1000, s.len());

  for pid in &pids {
    assert!(s.remove(pid));
  }
  assert!(s.is_empty());
}

#[test]
fn test_pid_set_same_id_different_address_are_distinct() {
  let set = PidSet::new();
  let local = Pid::new("node-a", "actor-1");
  let remote = Pid::new("node-b", "actor-1");

  set.add(local.clone());
  set.add(remote.clone());

  assert_eq!(2, set.len(), "アドレスが異なる PID は別物として保持されるべき");
  assert!(set.contains(&local), "ローカル PID が見つからない");
  assert!(set.contains(&remote), "リモート PID が見つからない");
}
