fn main() {
  tonic_build::configure()
    .out_dir("generated")
    .compile_protos(&["../proto/actor.proto"], &["../proto"])
    .unwrap();
}
