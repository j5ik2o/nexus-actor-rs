#![allow(unused_imports)]

pub mod actor {
  include!("../generated/actor.rs");
}
mod actor_impl;

pub use self::{actor_impl::*};
