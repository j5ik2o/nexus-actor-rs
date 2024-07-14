#[cfg(test)]
mod tests {
  use crate::actor::actor::actor_receiver::ActorReceiver;
  use crate::actor::actor::behavior::Behavior;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::context_handle::ContextHandle;
  use crate::actor::context::mock_context::MockContext;
  use std::sync::Arc;
  use tokio::sync::Mutex;

  #[tokio::test]
  async fn len() {
    let mut bs = Behavior::new();
    assert_eq!(bs.len(), 0);
    bs.push(|_| async { Ok(()) }).await;
    bs.push(|_| async { Ok(()) }).await;
    assert_eq!(bs.len(), 2);
  }

  #[tokio::test]
  async fn push() {
    let mut bs = Behavior::new();
    assert_eq!(bs.len(), 0);
    bs.push(|_| async { Ok(()) }).await;
    assert_eq!(bs.len(), 1);
    bs.push(|_| async { Ok(()) }).await;
    assert_eq!(bs.len(), 2);
  }

  #[tokio::test]
  async fn clear() {
    let mut bs = Behavior::new();
    bs.push(|_| async { Ok(()) }).await;
    bs.push(|_| async { Ok(()) }).await;
    assert_eq!(bs.len(), 2);
    bs.clear().await;
    assert_eq!(bs.len(), 0);
  }

  #[tokio::test]
  async fn peek() {
    let system = ActorSystem::new().await;
    let called = Arc::new(Mutex::new(0));
    let cloned_called1 = called.clone();
    let cloned_called2 = called.clone();

    let fn1 = ActorReceiver::new(move |_| {
      let cloned_called = cloned_called1.clone();
      async move {
        let mut mg = cloned_called.lock().await;
        *mg = 1;
        Ok(())
      }
    });

    let fn2 = ActorReceiver::new(move |_| {
      let cloned_called = cloned_called2.clone();
      async move {
        let mut mg = cloned_called.lock().await;
        *mg = 2;
        Ok(())
      }
    });

    let cases = vec![(vec![fn1.clone(), fn2.clone()], 2), (vec![fn2, fn1], 1)];

    for (items, expected) in cases {
      let mut bs = Behavior::new();
      for f in items {
        bs.push_actor_receiver(f).await;
      }
      if let Some(a) = bs.peek().await {
        let ctx = ContextHandle::new(MockContext::new(system.clone()));
        a.run(ctx).await.unwrap();
        assert_eq!(expected, *called.lock().await);
      } else {
        panic!("peek() returned None");
      }
    }
  }

  #[tokio::test]
  async fn pop() {
    let system = ActorSystem::new().await;
    let called = Arc::new(Mutex::new(0));
    let cloned_called1 = called.clone();
    let cloned_called2 = called.clone();

    let fn1 = ActorReceiver::new(move |_| {
      let cloned_called = cloned_called1.clone();
      async move {
        let mut mg = cloned_called.lock().await;
        *mg = 1;
        Ok(())
      }
    });

    let fn2 = ActorReceiver::new(move |_| {
      let cloned_called = cloned_called2.clone();
      async move {
        let mut mg = cloned_called.lock().await;
        *mg = 2;
        Ok(())
      }
    });

    let cases = vec![
      (vec![fn1.clone(), fn2.clone()], vec![2, 1]),
      (vec![fn2, fn1], vec![1, 2]),
    ];

    for (i, (items, expected)) in cases.into_iter().enumerate() {
      let test_name = format!("order {}", i);
      println!("Running test: {}", test_name);

      let mut bs = Behavior::new();
      for f in items {
        bs.push_actor_receiver(f).await;
      }

      for e in expected {
        if let Some(a) = bs.pop().await {
          let ctx = ContextHandle::new(MockContext::new(system.clone()));
          a.run(ctx).await.unwrap();
          assert_eq!(e, *called.lock().await, "Failed in test: {}", test_name);
          *called.lock().await = 0;
        } else {
          panic!("pop() returned None in test: {}", test_name);
        }
      }
    }
  }
}
