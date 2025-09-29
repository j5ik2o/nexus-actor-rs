/// Virtual Actor の一意な識別子。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClusterIdentity {
  kind: String,
  id: String,
}

impl ClusterIdentity {
  pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
    Self {
      kind: kind.into(),
      id: id.into(),
    }
  }

  pub fn kind(&self) -> &str {
    &self.kind
  }

  pub fn id(&self) -> &str {
    &self.id
  }
}
