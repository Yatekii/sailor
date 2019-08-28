fn main() {
    prost_build::compile_protos(&["src/vector_tile/vector-tile.proto"],
                                &["src/"]).unwrap();
}