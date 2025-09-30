fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile_protos(
      &[
        "../proto/remote.proto",
        "../proto/cluster.proto",
        "../proto/grain.proto",
        "../proto/pubsub.proto",
        "../proto/gossip.proto",
        "../proto/examples/remote-activate/messages.proto",
      ],
      &["../proto"],
    )
    .unwrap();
}
