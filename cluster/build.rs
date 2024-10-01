fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile(
      &[
        "proto/cluster.proto",
        "proto/grain.proto",
        "proto/pubsub.proto",
        "proto/gossip.proto",
      ],
      &["proto/", "../core/proto"],
    )
    .unwrap();
}
