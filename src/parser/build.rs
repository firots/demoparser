fn main() {
    // Maps and message types are committed generated inputs. Rebuilding the
    // parser must not execute the generator against a moving external clone
    // or rewrite the dependency checkout.
    println!("cargo::rerun-if-changed=../csgoproto/src/protobuf.rs");
    println!("cargo::rerun-if-changed=../csgoproto/src/maps.rs");
    println!("cargo::rerun-if-changed=../csgoproto/src/message_type.rs");
}
