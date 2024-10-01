fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile(
      &[
        "proto/google/protobuf/duration.proto",
        "proto/google/protobuf/any.proto",
        "proto/actor.proto",
      ],
      &["proto/"],
    )
    .unwrap();
}
