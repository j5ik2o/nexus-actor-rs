use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::endpoint_manager::{EndpointManager, RequestKeyWrapper};
use crate::endpoint_reader::EndpointReader;
use crate::generated::remote::{
  self, connect_request::ConnectionType, ClientConnection, ConnectRequest, RemoteMessage,
};
use crate::remote::Remote;
use bytes::Bytes;
use http_body_util::Empty;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, OnceCell};
use tokio::time::{timeout, Duration};
use tonic::codec::{Codec, ProstCodec, Streaming};
use tonic::{Request, Status};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

async fn setup_remote_with_manager() -> TestResult<(Arc<Remote>, EndpointReader)> {
  static INIT: OnceCell<()> = OnceCell::const_new();
  let _ = INIT
    .get_or_init(|| async {
      let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    })
    .await;

  let system = ActorSystem::new().await.expect("actor system init");
  let config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(19080)]).await;
  let remote = Remote::new(system, config).await;
  let remote_arc = Arc::new(remote);
  let endpoint_manager = EndpointManager::new(Arc::downgrade(&remote_arc));
  remote_arc.set_endpoint_manager_for_test(endpoint_manager).await;
  let endpoint_reader = EndpointReader::new(Arc::downgrade(&remote_arc));
  Ok((remote_arc, endpoint_reader))
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
