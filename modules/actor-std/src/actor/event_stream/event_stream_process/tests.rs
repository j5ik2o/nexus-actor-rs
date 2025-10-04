use crate::actor::actor_system::ActorSystem;
use crate::actor::context::SenderPart;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use nexus_message_derive_rs::Message;
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct EsTestMsg;

#[tokio::test]
async fn test_sends_messages_to_event_stream() {
  let test_cases = vec![
    ("plain", MessageHandle::new(EsTestMsg)),
    ("envelope", MessageHandle::new(EsTestMsg)),
  ];

  for (_, message) in test_cases {
    let system = ActorSystem::new().await.unwrap();
    let (tx, mut rx) = mpsc::channel(5);
    let event_stream = system.get_event_stream().await;

    let subscription = event_stream
      .subscribe(move |evt| {
        let cloned_tx = tx.clone();
        async move {
          if evt.as_any().is::<EsTestMsg>() {
            cloned_tx.send(()).await.unwrap();
          }
        }
      })
      .await;

    let pid = system.new_local_pid("eventstream").await;

    system.get_root_context().await.send(pid, message).await;

    rx.recv().await.unwrap();

    event_stream.unsubscribe(subscription).await;
  }
}
