fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile_protos(
      &[
        "proto/google/protobuf/duration.proto",
        "proto/google/protobuf/any.proto",
        "proto/actor.proto",
      ],
      &["proto/"],
    )
    .unwrap();
}
