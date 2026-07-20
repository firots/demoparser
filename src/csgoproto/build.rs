fn main() {
    // Generated protobuf bindings are committed to this crate. Ordinary
    // consumers must compile those pinned bytes rather than cloning the
    // moving GameTracking-CS2 tip and rewriting tracked source at build time.
    println!("cargo::rerun-if-changed=src/protobuf.rs");
}
