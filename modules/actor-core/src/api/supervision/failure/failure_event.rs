use super::FailureInfo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureEvent {
  RootEscalated(FailureInfo),
}
