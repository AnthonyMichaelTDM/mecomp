use tonic_prost_build::Config;

const PROTO_FILES: &[&str] = &[
    "proto/daemon.proto",
    "proto/apis/collection.proto",
    "proto/apis/duration.proto",
    "proto/apis/dynamic.proto",
    "proto/apis/entities.proto",
    "proto/apis/library.proto",
    "proto/apis/misc.proto",
    "proto/apis/playback.proto",
    "proto/apis/playlist.proto",
    "proto/apis/queue.proto",
    "proto/apis/radio.proto",
    "proto/apis/search.proto",
    "proto/apis/state.proto",
];

fn main() {
    let mut config = Config::new();
    config.type_attribute(
        "mecomp.RepeatMode",
        "#[derive(::serde::Serialize, ::serde::Deserialize)]",
    );
    config.type_attribute(
        "mecomp.PlaybackStatus",
        "#[derive(::serde::Serialize, ::serde::Deserialize)]",
    );
    config.type_attribute(
        "mecomp.SongBrief",
        "#[derive(::serde::Serialize, ::serde::Deserialize)]",
    );
    config.type_attribute(
        "mecomp.RecordId",
        "#[derive(::serde::Serialize, ::serde::Deserialize)]",
    );

    tonic_prost_build::configure()
        .compile_well_known_types(false)
        .extern_path(".mecomp.Duration", "crate::Duration")
        .build_transport(true)
        .build_client(true)
        .build_server(true)
        // .out_dir("proto")
        .emit_package(true)
        .emit_rerun_if_changed(true)
        .use_arc_self(true)
        .protoc_arg("-I ./proto/google/protobuf")
        .compile_with_config(config, PROTO_FILES, &["proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {e:?}"));
}
