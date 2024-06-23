extern crate prost_build;

fn main() {
  prost_build::compile_protos(&["src/actor/actor.proto"], &["src/actor/"]).unwrap();
}
