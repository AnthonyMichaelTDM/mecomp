const PROTO_FILES: &[&str] = &[
    "proto/daemon.proto",
    "proto/apis/collection.proto",
    "proto/apis/dynamic.proto",
    "proto/apis/entities.proto",
    "proto/apis/library.proto",
    "proto/apis/misc.proto",
    "proto/apis/playback.proto",
    "proto/apis/playlist.proto",
    "proto/apis/queue.proto",
    "proto/apis/radio.proto",
    "proto/apis/rand.proto",
    "proto/apis/search.proto",
    "proto/apis/state.proto",
];

fn main() {
    tonic_prost_build::configure()
        .compile_well_known_types(false)
        .emit_rerun_if_changed(true)
        .build_transport(true)
        .out_dir("out")
        .compile_protos(PROTO_FILES, &["proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {e:?}"));
}
