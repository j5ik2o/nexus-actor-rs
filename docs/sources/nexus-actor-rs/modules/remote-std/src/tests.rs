use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::endpoint_manager::{EndpointManager, RequestKeyWrapper};
use crate::endpoint_reader::EndpointReader;
use crate::endpoint_state::ConnectionState;
use crate::endpoint_writer_mailbox::EndpointWriterMailbox;
use crate::generated::remote::{
  self, connect_request::ConnectionType, ClientConnection, ConnectRequest, RemoteMessage,
};
use crate::messages::{BackpressureLevel, RemoteDeliver};
use crate::remote::Remote;
use bytes::Bytes;
use http_body_util::Empty;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::core::{ActorError, ErrorReason};
use nexus_actor_std_rs::actor::dispatch::DeadLetterEvent;
use nexus_actor_std_rs::actor::dispatch::{
  Dispatcher, DispatcherHandle, Mailbox, MailboxQueueKind, MessageInvoker, MessageInvokerHandle, Runnable,
};
use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::generated::actor::Pid;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, OnceCell, RwLock};
use tokio::time::{timeout, Duration};
use tonic::codec::{Codec, ProstCodec, Streaming};
use tonic::{Request, Status};

use async_trait::async_trait;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

async fn setup_remote_with_options(options: Vec<ConfigOption>) -> TestResult<(Arc<Remote>, EndpointReader)> {
  static INIT: OnceCell<()> = OnceCell::const_new();
  let _ = INIT
    .get_or_init(|| async {
      let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    })
    .await;

  let system = ActorSystem::new().await.expect("actor system init");
  let config = Config::from(options).await;
  let remote = Remote::new(system, config).await;
  let remote_arc = Arc::new(remote);
  let endpoint_manager = EndpointManager::new(Arc::downgrade(&remote_arc));
  remote_arc.set_endpoint_manager_for_test(endpoint_manager).await;
  let endpoint_reader = EndpointReader::new(Arc::downgrade(&remote_arc));
  Ok((remote_arc, endpoint_reader))
}

async fn setup_remote_with_manager() -> TestResult<(Arc<Remote>, EndpointReader)> {
  setup_remote_with_options(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19080),
  ])
  .await
}

fn make_request_key() -> RequestKeyWrapper {
  let mut codec: ProstCodec<RemoteMessage, RemoteMessage> = ProstCodec::default();
  let streaming = Streaming::new_request(codec.decoder(), Empty::<Bytes>::new(), None, None);
  RequestKeyWrapper::new(Arc::new(Mutex::new(Request::new(streaming))))
}

#[tokio::test]
async fn client_connection_registers_and_receives_disconnect() -> TestResult<()> {
  let (remote_arc, endpoint_reader) = setup_remote_with_manager().await?;
  let (response_tx, mut response_rx) = mpsc::channel::<Result<RemoteMessage, Status>>(4);
  let connection_key = make_request_key();

  endpoint_reader
    .on_connect_request(
      &response_tx,
      &ConnectRequest {
        connection_type: Some(ConnectionType::ClientConnection(ClientConnection {
          system_id: "client-A".to_string(),
        })),
      },
      Some(&connection_key),
    )
    .await?;

  let connect_msg = timeout(Duration::from_millis(100), response_rx.recv())
    .await?
    .ok_or_else(|| "no connect response".to_string())?
    .map_err(|status| Box::new(status) as Box<dyn std::error::Error>)?;
  match connect_msg.message_type {
    Some(remote::remote_message::MessageType::ConnectResponse(response)) => {
      assert_eq!(response.member_id, "client-A");
      assert!(!response.blocked);
    }
    other => panic!("unexpected connect response: {other:?}"),
  }

  let manager = remote_arc.get_endpoint_manager().await;
  assert!(manager.has_client_connection("client-A"));

  manager
    .send_to_client(
      "client-A",
      RemoteMessage {
        message_type: Some(remote::remote_message::MessageType::DisconnectRequest(
          remote::DisconnectRequest {},
        )),
      },
    )
    .await?;

  let disconnect_msg = timeout(Duration::from_millis(100), response_rx.recv())
    .await?
    .ok_or_else(|| "no disconnect response".to_string())?
    .map_err(|status| Box::new(status) as Box<dyn std::error::Error>)?;
  match disconnect_msg.message_type {
    Some(remote::remote_message::MessageType::DisconnectRequest(_)) => {}
    other => panic!("unexpected disconnect message: {other:?}"),
  }

  manager.deregister_client_connection(&connection_key);
  assert!(!manager.has_client_connection("client-A"));

  Ok(())
}

#[derive(Debug, Clone, PartialEq, nexus_message_derive_rs::Message)]
struct DummyPayload {
  value: &'static str,
}

#[derive(Debug, Clone)]
struct NoopInvoker;

#[async_trait]
impl MessageInvoker for NoopInvoker {
  async fn invoke_system_message(&mut self, _message_handle: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn invoke_user_message(&mut self, _message_handle: MessageHandle) -> Result<(), ActorError> {
    Ok(())
  }

  async fn escalate_failure(&mut self, _reason: ErrorReason, _message_handle: MessageHandle) {}

  async fn record_mailbox_queue_latency(&mut self, _: MailboxQueueKind, _: Duration) {}
}

#[derive(Debug, Clone)]
struct NoopDispatcher;

#[async_trait]
impl Dispatcher for NoopDispatcher {
  async fn schedule(&self, _runner: Runnable) {}

  async fn throughput(&self) -> i32 {
    0
  }
}

#[tokio::test]
async fn client_connection_backpressure_overflow() -> TestResult<()> {
  let (remote_arc, endpoint_reader) = setup_remote_with_options(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19081),
    ConfigOption::with_endpoint_writer_queue_size(1),
    ConfigOption::with_endpoint_writer_batch_size(1),
  ])
  .await?;

  let (response_tx, mut response_rx) = mpsc::channel::<Result<RemoteMessage, Status>>(4);
  let connection_key = make_request_key();

  endpoint_reader
    .on_connect_request(
      &response_tx,
      &ConnectRequest {
        connection_type: Some(ConnectionType::ClientConnection(ClientConnection {
          system_id: "client-overflow".to_string(),
        })),
      },
      Some(&connection_key),
    )
    .await?;

  timeout(Duration::from_millis(100), response_rx.recv())
    .await?
    .ok_or_else(|| "no connect response".to_string())??;

  let snapshot_interval = remote_arc
    .get_config()
    .get_endpoint_writer_queue_snapshot_interval()
    .await;
  let mut mailbox = EndpointWriterMailbox::new(Arc::downgrade(&remote_arc), 1, 1, snapshot_interval);
  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(Arc::new(RwLock::new(NoopInvoker)))),
      Some(DispatcherHandle::new(NoopDispatcher)),
    )
    .await;

  let actor_system = remote_arc.get_actor_system().clone();
  let deadletters = Arc::new(Mutex::new(Vec::new()));
  let subscription = actor_system
    .get_event_stream()
    .await
    .subscribe({
      let deadletters = deadletters.clone();
      move |message: MessageHandle| {
        let deadletters = deadletters.clone();
        async move {
          if let Some(event) = message.to_typed::<DeadLetterEvent>() {
            deadletters.lock().await.push(event);
          }
        }
      }
    })
    .await;

  let target_pid = Pid {
    address: "127.0.0.1:9000".to_string(),
    id: "dummy".to_string(),
    request_id: 0,
  };

  let deliver = RemoteDeliver {
    header: None,
    message: MessageHandle::new(DummyPayload { value: "payload" }),
    target: target_pid.clone(),
    sender: None,
    serializer_id: 0,
  };

  mailbox.post_user_message(MessageHandle::new(deliver.clone())).await;
  mailbox.post_user_message(MessageHandle::new(deliver)).await;

  tokio::time::sleep(Duration::from_millis(50)).await;

  let dead_letters = deadletters.lock().await;
  assert!(dead_letters
    .iter()
    .any(|event| event.pid.as_ref().map(|pid| pid.inner_pid.id.as_str()) == Some(target_pid.id.as_str())));
  drop(dead_letters);

  let stats = remote_arc
    .get_endpoint_manager()
    .await
    .statistics_snapshot(&target_pid.address)
    .expect("statistics should exist for endpoint");
  assert_eq!(stats.queue_capacity, 1);
  assert!(stats.queue_size <= stats.queue_capacity);
  assert!(stats.dead_letters >= 1);
  assert_eq!(stats.backpressure_level, BackpressureLevel::Normal);

  actor_system.get_event_stream().await.unsubscribe(subscription).await;
  remote_arc
    .get_endpoint_manager()
    .await
    .deregister_client_connection(&connection_key);

  Ok(())
}

#[tokio::test]
async fn remote_runtime_tracks_block_list_updates() -> TestResult<()> {
  let (remote_arc, _endpoint_reader) = setup_remote_with_manager().await?;
  let factory = remote_arc.runtime().await;

  let block_list = runtime.block_list().expect("runtime exposes block list store");
  assert!(!block_list.is_blocked("node-x"));

  remote_arc.block_system("node-x").await;
  assert!(block_list.is_blocked("node-x"));

  remote_arc.unblock_system("node-x").await;
  assert!(!block_list.is_blocked("node-x"));

  Ok(())
}

#[tokio::test]
async fn remote_runtime_applies_initial_blocked_members() -> TestResult<()> {
  let (remote_arc, _endpoint_reader) = setup_remote_with_options(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19082),
    ConfigOption::with_initial_blocked_member("node-initial"),
  ])
  .await?;

  let factory = remote_arc.runtime().await;
  let block_list = runtime
    .block_list()
    .expect("runtime exposes block list store for initial members");
  assert!(block_list.is_blocked("node-initial"));

  let blocked = remote_arc.list_blocked_systems().await;
  assert!(blocked.contains(&"node-initial".to_string()));

  Ok(())
}

#[tokio::test]
async fn remote_await_reconnect_returns_connection_state() -> TestResult<()> {
  let (remote_arc, _endpoint_reader) = setup_remote_with_manager().await?;
  let manager = remote_arc.get_endpoint_manager().await;

  let waiter = {
    let remote = remote_arc.clone();
    async move { remote.await_reconnect("endpoint-remote-await").await }
  };

  let updater = async {
    tokio::time::sleep(Duration::from_millis(10)).await;
    manager
      .update_connection_state("endpoint-remote-await", ConnectionState::Connected)
      .await;
  };

  let (_, state) = tokio::join!(updater, waiter);
  assert_eq!(state, Some(ConnectionState::Connected));

  Ok(())
}

#[tokio::test]
async fn client_connection_backpressure_drain() -> TestResult<()> {
  let (remote_arc, _endpoint_reader) = setup_remote_with_options(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19110),
    ConfigOption::with_endpoint_writer_queue_size(4),
    ConfigOption::with_endpoint_writer_batch_size(2),
  ])
  .await?;

  let manager = remote_arc.get_endpoint_manager().await;
  let snapshot_interval = remote_arc
    .get_config()
    .get_endpoint_writer_queue_snapshot_interval()
    .await;
  let mut mailbox = EndpointWriterMailbox::new(Arc::downgrade(&remote_arc), 2, 4, snapshot_interval);
  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(Arc::new(RwLock::new(NoopInvoker)))),
      Some(DispatcherHandle::new(NoopDispatcher)),
    )
    .await;

  let target_pid = Pid {
    address: "endpoint-drain".to_string(),
    id: "target".to_string(),
    request_id: 0,
  };

  let payloads = ["drain-0", "drain-1", "drain-2"];
  for value in payloads {
    let payload = DummyPayload { value };
    let message = MessageHandle::new(payload);
    let deliver = RemoteDeliver {
      header: None,
      message,
      target: target_pid.clone(),
      sender: None,
      serializer_id: 0,
    };
    mailbox.post_user_message(MessageHandle::new(deliver)).await;
  }

  tokio::time::sleep(Duration::from_millis(20)).await;

  let before = manager
    .statistics_snapshot("endpoint-drain")
    .expect("statistics should exist");
  assert!(before.queue_size >= 3);

  let handle = mailbox.to_handle().await;
  handle.process_messages().await;

  tokio::time::sleep(Duration::from_millis(20)).await;

  let after = manager
    .statistics_snapshot("endpoint-drain")
    .expect("statistics should exist");
  assert!(after.queue_size <= 1);

  Ok(())
}

#[tokio::test]
async fn endpoint_writer_queue_snapshot_interval_samples_updates() -> TestResult<()> {
  let (remote_arc, _endpoint_reader) = setup_remote_with_options(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19130),
    ConfigOption::with_endpoint_writer_queue_size(32),
    ConfigOption::with_endpoint_writer_queue_snapshot_interval(4),
  ])
  .await?;

  let manager = remote_arc.get_endpoint_manager().await;
  let snapshot_interval = remote_arc
    .get_config()
    .get_endpoint_writer_queue_snapshot_interval()
    .await;
  let mut mailbox = EndpointWriterMailbox::new(Arc::downgrade(&remote_arc), 4, 32, snapshot_interval);
  mailbox
    .register_handlers(
      Some(MessageInvokerHandle::new(Arc::new(RwLock::new(NoopInvoker)))),
      Some(DispatcherHandle::new(NoopDispatcher)),
    )
    .await;

  let target_pid = Pid {
    address: "endpoint-snapshot".to_string(),
    id: "target".to_string(),
    request_id: 0,
  };

  let enqueue_message = |value: &'static str| {
    let deliver = RemoteDeliver {
      header: None,
      message: MessageHandle::new(DummyPayload { value }),
      target: target_pid.clone(),
      sender: None,
      serializer_id: 0,
    };
    mailbox.post_user_message(MessageHandle::new(deliver))
  };

  enqueue_message("msg-0").await;
  tokio::time::sleep(Duration::from_millis(20)).await;

  let snapshot_first = manager
    .statistics_snapshot(&target_pid.address)
    .expect("stats after first enqueue");
  assert_eq!(snapshot_first.queue_size, 1);

  enqueue_message("msg-1").await;
  enqueue_message("msg-2").await;
  tokio::time::sleep(Duration::from_millis(20)).await;

  let snapshot_mid = manager
    .statistics_snapshot(&target_pid.address)
    .expect("stats after third enqueue");
  assert_eq!(snapshot_mid.queue_size, 1);

  enqueue_message("msg-3").await;
  tokio::time::sleep(Duration::from_millis(20)).await;

  let snapshot_after_interval = manager
    .statistics_snapshot(&target_pid.address)
    .expect("stats after hitting snapshot interval");
  assert!(snapshot_after_interval.queue_size >= 4);

  let handle = mailbox.to_handle().await;
  handle.process_messages().await;
  tokio::time::sleep(Duration::from_millis(20)).await;

  let snapshot_final = manager
    .statistics_snapshot(&target_pid.address)
    .expect("stats after drain");
  assert_eq!(snapshot_final.queue_size, 0);

  Ok(())
}

#[tokio::test]
async fn remote_await_reconnect_resolves_closed_after_schedule() -> TestResult<()> {
  let (remote_arc, _endpoint_reader) = setup_remote_with_options(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(19082),
    ConfigOption::with_endpoint_reconnect_max_retries(1),
    ConfigOption::with_endpoint_reconnect_initial_backoff(Duration::from_millis(0)),
    ConfigOption::with_endpoint_reconnect_max_backoff(Duration::from_millis(0)),
  ])
  .await?;

  let manager = remote_arc.get_endpoint_manager().await;
  let waiter = {
    let remote = remote_arc.clone();
    async move { remote.await_reconnect("endpoint-remote-await-closed").await }
  };

  let scheduler = async {
    manager
      .schedule_reconnect("endpoint-remote-await-closed".to_string())
      .await;
  };

  let (_, state) = tokio::join!(scheduler, waiter);
  assert_eq!(state, Some(ConnectionState::Closed));

  Ok(())
}
