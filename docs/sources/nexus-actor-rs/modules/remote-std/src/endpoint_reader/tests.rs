use super::*;
use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::endpoint_manager::EndpointManager;
use crate::generated::remote::{self, connect_request::ConnectionType, ClientConnection, ConnectRequest};
use crate::remote::Remote;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::actor::process::{Process, ProcessHandle};
use std::any::Any;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};
use tonic::{Request, Status};

#[derive(Debug, Clone)]
struct DummyProcess;

#[async_trait::async_trait]
impl Process for DummyProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, _: MessageHandle) {}

  async fn send_system_message(&self, _: &ExtendedPid, _: MessageHandle) {}

  async fn stop(&self, _: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[derive(Debug, Clone)]
struct NoopActor;

#[async_trait::async_trait]
impl Actor for NoopActor {
  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }
}

async fn setup_remote() -> (Arc<Remote>, EndpointReader) {
  static INIT: OnceCell<()> = OnceCell::const_new();
  // 一部のテストではトレース初期化が複数回行われると panic するため once でガード
  let _ = INIT
    .get_or_init(|| async {
      let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    })
    .await;

  let system = ActorSystem::new().await.expect("actor system init");
  let config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(19080)]).await;
  let remote = Remote::new(system.clone(), config).await;
  let remote_arc = Arc::new(remote);
  let endpoint_manager = EndpointManager::new(Arc::downgrade(&remote_arc));
  remote_arc.set_endpoint_manager_for_test(endpoint_manager).await;
  let endpoint_reader = EndpointReader::new(Arc::downgrade(&remote_arc));
  (remote_arc, endpoint_reader)
}

#[tokio::test]
async fn list_processes_filters_named_processes() {
  let (remote_arc, endpoint_reader) = setup_remote().await;
  let actor_system = remote_arc.get_actor_system().clone();
  let registry = actor_system.get_process_registry().await;

  let _ = registry
    .add_process(ProcessHandle::new(DummyProcess), "proc_alpha")
    .await;
  let _ = registry.add_process(ProcessHandle::new(DummyProcess), "other").await;

  let request = ListProcessesRequest {
    pattern: "proc".to_string(),
    r#type: ListProcessesMatchType::MatchPartOfString as i32,
  };

  let response = endpoint_reader
    .list_processes(Request::new(request))
    .await
    .expect("list_processes should succeed");
  let mut ids: Vec<_> = response.into_inner().pids.into_iter().map(|pid| pid.id).collect();
  ids.sort();

  assert_eq!(ids, vec!["proc_alpha".to_string()]);
}

#[tokio::test]
async fn get_process_diagnostics_returns_actor_process_info() {
  let (remote_arc, endpoint_reader) = setup_remote().await;
  let actor_system = remote_arc.get_actor_system().clone();
  let mut root = actor_system.get_root_context().await;

  let props = Props::from_async_actor_producer(|_| async { NoopActor }).await;
  let pid = root.spawn_named(props, "diag_actor").await.expect("spawn actor");

  let request = GetProcessDiagnosticsRequest {
    pid: Some(pid.inner_pid.clone()),
  };

  let response = endpoint_reader
    .get_process_diagnostics(Request::new(request))
    .await
    .expect("diagnostics should succeed");
  let diagnostics = response.into_inner().diagnostics_string;

  assert!(diagnostics.contains("ActorProcess"));
}

#[tokio::test]
async fn client_connection_respects_block_list() {
  let (remote_arc, endpoint_reader) = setup_remote().await;
  let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<remote::RemoteMessage, Status>>(1);

  let request = ConnectRequest {
    connection_type: Some(ConnectionType::ClientConnection(ClientConnection {
      system_id: "client-1".to_string(),
    })),
  };

  endpoint_reader
    .on_connect_request(&tx, &request, None)
    .await
    .expect("client connection should succeed");

  let message = rx.recv().await.expect("response available").expect("ok result");
  match message.message_type {
    Some(remote::remote_message::MessageType::ConnectResponse(response)) => {
      assert!(!response.blocked);
      assert_eq!(response.member_id, "client-1");
    }
    other => panic!("unexpected response: {other:?}"),
  }

  remote_arc.get_block_list().block("client-2".to_string()).await;

  let blocked_request = ConnectRequest {
    connection_type: Some(ConnectionType::ClientConnection(ClientConnection {
      system_id: "client-2".to_string(),
    })),
  };

  endpoint_reader
    .on_connect_request(&tx, &blocked_request, None)
    .await
    .expect("blocked client connection should return response");

  let blocked = rx.recv().await.expect("blocked response").expect("ok result");
  match blocked.message_type {
    Some(remote::remote_message::MessageType::ConnectResponse(response)) => {
      assert!(response.blocked);
      assert_eq!(response.member_id, "client-2");
    }
    other => panic!("unexpected blocked response: {other:?}"),
  }
}

#[tokio::test]
async fn get_suspend_reflects_shared_state() {
  let flag = Arc::new(Mutex::new(true));
  assert!(EndpointReader::get_suspend(flag.clone()).await);
  {
    let mut mg = flag.lock().await;
    *mg = false;
  }
  assert!(!EndpointReader::get_suspend(flag).await);
}
