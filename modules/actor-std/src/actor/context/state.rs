#[cfg(test)]
mod tests;

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum State {
  Alive = 0,
  Restarting = 1,
  Stopping = 2,
  Stopped = 3,
}
