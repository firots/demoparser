use std::{io::{Error, ErrorKind, Result}, path::Path};

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=src/protobuf.rs");
    println!("cargo::rerun-if-env-changed=CS2PARSER_REGENERATE_PROTOS");
    // Use the checked-in bindings unless regeneration is explicitly requested.
    if std::env::var_os("CS2PARSER_REGENERATE_PROTOS").is_none() {
        return Ok(());
    }
    println!("cargo::rerun-if-changed=GameTracking-CS2/Protobufs/demo.proto");

    // Regeneration must use a deliberately prepared source revision, never a
    // network checkout that silently follows the latest game update.
    if !Path::new("GameTracking-CS2/Protobufs/demo.proto").is_file() {
        return Err(Error::new(ErrorKind::NotFound,
            "Prepare and record a pinned GameTracking-CS2 checkout before regenerating protobufs"));
    }

    let protos = vec![
        "GameTracking-CS2/Protobufs/steammessages.proto",
        "GameTracking-CS2/Protobufs/gcsdk_gcmessages.proto",
        "GameTracking-CS2/Protobufs/demo.proto",
        "GameTracking-CS2/Protobufs/cstrike15_gcmessages.proto",
        "GameTracking-CS2/Protobufs/cstrike15_usermessages.proto",
        "GameTracking-CS2/Protobufs/usermessages.proto",
        "GameTracking-CS2/Protobufs/networkbasetypes.proto",
        "GameTracking-CS2/Protobufs/engine_gcmessages.proto",
        "GameTracking-CS2/Protobufs/netmessages.proto",
        "GameTracking-CS2/Protobufs/network_connection.proto",
        "GameTracking-CS2/Protobufs/cs_usercmd.proto",
        "GameTracking-CS2/Protobufs/usercmd.proto",
        "GameTracking-CS2/Protobufs/gameevents.proto",
        "GameTracking-CS2/Protobufs/cs_gameevents.proto",
    ];

    prost_build::Config::new()
        .format(false)
        .out_dir("src")
        .default_package_filename("protobuf")
        .bytes(["."])
        .enum_attribute(".", "#[derive(::strum::EnumIter)]")
        .compile_protos(&protos, &["GameTracking-CS2/Protobufs/"])
}
