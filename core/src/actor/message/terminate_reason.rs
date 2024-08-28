#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TerminateReason {
  Stopped = 0,
  AddressTerminated = 1,
  NotFound = 2,
}

impl TerminateReason {
  /// String value of the enum field names used in the ProtoBuf definition.
  ///
  /// The values are not transformed in any way and thus are considered stable
  /// (if the ProtoBuf definition does not change) and safe for programmatic use.
  pub fn as_str_name(&self) -> &'static str {
    match self {
      TerminateReason::Stopped => "Stopped",
      TerminateReason::AddressTerminated => "AddressTerminated",
      TerminateReason::NotFound => "NotFound",
    }
  }

  /// Creates an enum from field names used in the ProtoBuf definition.
  pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
    match value {
      "Stopped" => Some(Self::Stopped),
      "AddressTerminated" => Some(Self::AddressTerminated),
      "NotFound" => Some(Self::NotFound),
      _ => None,
    }
  }
}
