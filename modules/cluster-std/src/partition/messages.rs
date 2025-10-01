use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::generated::actor::Pid;
use nexus_message_derive_rs::Message as MessageDerive;
use serde::{Deserialize, Serialize};

use crate::identity::ClusterIdentity;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, MessageDerive)]
pub struct ActivationRequest {
  pub identity: ClusterIdentity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, MessageDerive)]
pub struct ActivationResponse {
  #[serde(with = "extended_pid_option_serde")]
  pub pid: Option<nexus_actor_std_rs::actor::core::ExtendedPid>,
}

mod extended_pid_option_serde {
  use super::*;
  use serde::{Deserialize, Deserializer, Serialize, Serializer};

  #[derive(Serialize, Deserialize)]
  struct ExtendedPidSerde {
    address: String,
    id: String,
    request_id: u32,
  }

  pub fn serialize<S>(
    value: &Option<nexus_actor_std_rs::actor::core::ExtendedPid>,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: Serializer, {
    let opt = value.as_ref().map(|pid| ExtendedPidSerde {
      address: pid.inner_pid.address.clone(),
      id: pid.inner_pid.id.clone(),
      request_id: pid.inner_pid.request_id,
    });
    opt.serialize(serializer)
  }

  pub fn deserialize<'de, D>(
    deserializer: D,
  ) -> Result<Option<nexus_actor_std_rs::actor::core::ExtendedPid>, D::Error>
  where
    D: Deserializer<'de>, {
    let opt = Option::<ExtendedPidSerde>::deserialize(deserializer)?;
    Ok(opt.map(|data| {
      let pid = Pid {
        address: data.address,
        id: data.id,
        request_id: data.request_id,
      };
      nexus_actor_std_rs::actor::core::ExtendedPid::new(pid)
    }))
  }
}
