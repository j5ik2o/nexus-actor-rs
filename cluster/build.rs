fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile_protos(
      &[
        "../proto/cluster.proto",
        "../proto/grain.proto",
        "../proto/pubsub.proto",
        "../proto/gossip.proto",
        "../proto/registry.proto",
      ],
      &["../proto"],
    )
    .unwrap();
}
