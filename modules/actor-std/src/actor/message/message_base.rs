// Re-export Message trait from core_types
pub use crate::actor::core_types::message_types::Message;

// tests/tests
#[cfg(test)]
mod tests {
  use super::*;
  use nexus_message_derive_rs::Message;
  #[derive(Debug, Clone, PartialEq, Message)]
  pub struct Hello {
    pub who: String,
  }

  #[test]
  fn test_message_derive() {
    let msg1 = Hello {
      who: "World".to_string(),
    };
    let msg2 = Hello {
      who: "World".to_string(),
    };
    let msg3 = Hello {
      who: "Rust".to_string(),
    };

    assert!(msg1.eq_message(&msg2));
    assert!(!msg1.eq_message(&msg3));
  }
}
