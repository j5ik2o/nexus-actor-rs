extern crate prost_build;

fn main() {
  //  prost_build::compile_protos(&["proto/actor.proto"], &["proto/"]).unwrap();
  tonic_build::configure()
    .out_dir("src/generated")
    .compile(&["proto/actor.proto", "proto/remote.proto"], &["proto/"])
    .unwrap();
}
