use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Pid {
  pub id: Option<String>,
  pub address: String,
}

impl Debug for Pid {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Pid({}, {})", self.address, self.id.as_deref().unwrap_or("None"))
  }
}

impl Display for Pid {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}/{}", self.address, self.id.as_deref().unwrap_or("None"))
  }
}

impl Default for Pid {
  fn default() -> Self {
    Self {
      id: None,
      address: String::new(),
    }
  }
}
