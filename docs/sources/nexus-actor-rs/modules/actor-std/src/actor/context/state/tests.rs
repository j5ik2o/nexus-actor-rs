use crate::actor::context::state::State;
use num_enum::TryFromPrimitive;

#[test]
fn test_state_primitive_conversions() {
  // Test conversion from primitive to State
  assert_eq!(State::try_from_primitive(0).unwrap(), State::Alive);
  assert_eq!(State::try_from_primitive(1).unwrap(), State::Restarting);
  assert_eq!(State::try_from_primitive(2).unwrap(), State::Stopping);
  assert_eq!(State::try_from_primitive(3).unwrap(), State::Stopped);

  // Test invalid primitive conversion
  assert!(State::try_from_primitive(4).is_err());
}

#[test]
fn test_state_to_primitive() {
  // Test conversion from State to primitive
  assert_eq!(State::Alive as u8, 0);
  assert_eq!(State::Restarting as u8, 1);
  assert_eq!(State::Stopping as u8, 2);
  assert_eq!(State::Stopped as u8, 3);
}

#[test]
fn test_state_debug_format() {
  assert_eq!(format!("{:?}", State::Alive), "Alive");
  assert_eq!(format!("{:?}", State::Restarting), "Restarting");
  assert_eq!(format!("{:?}", State::Stopping), "Stopping");
  assert_eq!(format!("{:?}", State::Stopped), "Stopped");
}

#[test]
fn test_state_equality() {
  assert_eq!(State::Alive, State::Alive);
  assert_ne!(State::Alive, State::Stopped);

  let state1 = State::Restarting;
  let state2 = State::Restarting;
  assert_eq!(state1, state2);
}

#[test]
fn test_state_copy_clone() {
  let state = State::Alive;
  let copied = state;
  assert_eq!(state, copied);

  let cloned = state;
  assert_eq!(state, cloned);
}
