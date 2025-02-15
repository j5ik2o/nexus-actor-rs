//! Continuation message implementation.

use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct Continuation {
  pub(crate) id: u64,
}

impl Continuation {
  pub fn new(id: u64) -> Self {
    Self { id }
  }
}

// Remove Message implementation to avoid conflict with blanket impl
