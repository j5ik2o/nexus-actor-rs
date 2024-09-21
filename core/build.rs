extern crate prost_build;

fn main() {
  //  prost_build::compile_protos(&["proto/actor.proto"], &["proto/"]).unwrap();
  tonic_build::configure()
    .out_dir("src/generated")
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
      ],
      &["proto/"],
    )
    .unwrap();
}
