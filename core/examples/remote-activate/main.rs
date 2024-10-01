mod examples {
  mod remote_activate {
    pub mod messages {
      include!("../../generated/examples.remote_activate.messages.rs");
    }
  }
}

#[tokio::main]
async fn main() {}
