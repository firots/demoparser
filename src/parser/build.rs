use std::{io::Result, process::Command};

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=../csgoproto/src/protobuf.rs");
    println!("cargo::rerun-if-changed=../csgoproto/src/maps.rs");
    println!("cargo::rerun-if-env-changed=CS2PARSER_REGENERATE_MAPS");
    // Use the checked-in maps unless regeneration is explicitly requested.
    if std::env::var_os("CS2PARSER_REGENERATE_MAPS").is_none() {
        return Ok(());
    }
    println!("cargo::rerun-if-changed=../csgoproto/GameTracking-CS2/game/csgo/pak01_dir/resource/csgo_english.txt");

    let profile = std::env::var("PROFILE").unwrap_or("debug".to_string());
    let mut command = Command::new("cargo");
    command.current_dir("../csgoproto").arg("run");
    if profile == "release" { command.arg("--release"); }
    if !command.status()?.success() {
        return Err(std::io::Error::other("Explicit map regeneration failed"));
    }

    Ok(())
}
