extern crate prost_build;

fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile(
      &[
        "proto/google/protobuf/duration.proto",
        "proto/google/protobuf/any.proto",
        "proto/actor.proto",
        "proto/remote.proto",
        "proto/cluster.proto",
        "proto/grain.proto",
        "proto/pubsub.proto",
        "proto/gossip.proto",
        "proto/examples/remote-activate/messages.proto",
      ],
      &["proto/"],
    )
    .unwrap();
}
