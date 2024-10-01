use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum ResponseStatusCode {
  Ok = 0,
  Unavailable,
  Timeout,
  ProcessNameAlreadyExists,
  Error,
  DeadLetter,
  Max,
}
