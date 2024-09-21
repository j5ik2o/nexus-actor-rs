#[cfg(test)]
mod tests {

  use crate::actor::actor::pid_set::PidSet;

  use crate::actor::actor_system::ActorSystem;

  use crate::generated::actor::Pid;

  #[tokio::test]
  async fn test_pid_set_empty() {
    let s = PidSet::new().await;
    assert!(s.is_empty().await);
  }

  #[tokio::test]
  async fn test_pid_set_clear() {
    let mut s = PidSet::new().await;
    s.add(Pid::new("local", "p1")).await;
    s.add(Pid::new("local", "p2")).await;
    s.add(Pid::new("local", "p3")).await;
    assert_eq!(3, s.len().await);
    s.clear().await;
    assert!(s.is_empty().await);
    // assert_eq!(0, s.pids.len());
  }

  #[tokio::test]
  async fn test_pid_set_add_small() {
    let mut s = PidSet::new().await;
    let p1 = Pid::new("local", "p1");
    s.add(p1).await;
    assert!(!s.is_empty().await);
    let p1 = Pid::new("local", "p1");
    s.add(p1).await;
    assert_eq!(1, s.len().await);
  }

  #[tokio::test]
  async fn test_pid_set_values() {
    let mut s = PidSet::new().await;
    s.add(Pid::new("local", "p1")).await;
    s.add(Pid::new("local", "p2")).await;
    s.add(Pid::new("local", "p3")).await;
    assert!(!s.is_empty().await);

    let r = s.to_vec().await;
    assert_eq!(3, r.len());
  }

  #[tokio::test]
  async fn test_pid_set_add_map() {
    let mut s = PidSet::new().await;
    let p1 = Pid::new("local", "p1");
    s.add(p1).await;
    assert!(!s.is_empty().await);
    let p1 = Pid::new("local", "p1");
    s.add(p1).await;
    assert_eq!(1, s.len().await);
  }

  #[tokio::test]
  async fn test_pid_set_add_remove() {
    let mut pids = vec![];
    for i in 0..1000 {
      let pid = Pid::new("local", &format!("p{}", i));
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
}
