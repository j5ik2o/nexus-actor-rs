use num_enum::{IntoPrimitive, TryFromPrimitive};

// Directive enum to replace the constants
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Directive {
  Resume = 0,
  Restart = 1,
  Stop = 2,
  Escalate = 3,
}

impl Directive {
  pub fn as_str(&self) -> &'static str {
    match self {
      Directive::Resume => "ResumeDirective",
      Directive::Restart => "RestartDirective",
      Directive::Stop => "StopDirective",
      Directive::Escalate => "EscalateDirective",
    }
  }
}

impl std::fmt::Display for Directive {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}
