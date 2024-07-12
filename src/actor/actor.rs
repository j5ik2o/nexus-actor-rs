pub mod actor;
pub mod actor_error;
pub mod actor_handle;
pub mod actor_inner_error;
pub mod actor_process;
pub mod actor_producer;
pub mod actor_receiver;
pub mod behavior;
pub mod context_decorator;
pub mod context_decorator_chain;
pub mod context_handler;
pub mod continuer;
pub mod middleware_chain;
pub mod pid;
pub mod pid_set;
mod pid_set_test;
pub mod props;
pub mod receiver_middleware;
pub mod receiver_middleware_chain;
pub mod restart_statistics;
pub mod sender_middleware;
pub mod sender_middleware_chain;
pub mod spawn_middleware;
pub mod spawner;
pub mod taks;

// include!(concat!(env!("OUT_DIR"), "/actor.rs"));

#[derive(Debug, Clone, PartialEq)]
pub struct Pid {
  pub address: String,
  pub id: String,
  pub request_id: u32,
}
