//! Computes each negotiated connection's development protocol fingerprint from the generation
//! registry's input directories, so every build describes its own protocol source. The registry's
//! remaining content is mirrored in source and checked by the conformance tests, not generated.

#[path = "build/fingerprint.rs"]
mod fingerprint;

use std::{env, fmt::Write as _, fs, path::Path};

use serde_json::Value;

const REGISTRY: &str = "schemas/connection/v1/connections.json";
/// Registry connection names and the constant prefix `src/registry.rs` includes for each.
const CONNECTIONS: [(&str, &str); 2] = [
    ("client-server", "CLIENT_SERVER"),
    ("worker-host", "WORKER_HOST"),
];

fn main() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR");
    let repository = Path::new(&manifest)
        .ancestors()
        .nth(2)
        .expect("the crate lives at crates/connection-handshake");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/fingerprint.rs");
    let registry_path = repository.join(REGISTRY);
    println!("cargo:rerun-if-changed={}", registry_path.display());

    let registry: Value = serde_json::from_str(
        &fs::read_to_string(&registry_path)
            .unwrap_or_else(|error| panic!("read {REGISTRY}: {error}")),
    )
    .unwrap_or_else(|error| panic!("parse {REGISTRY}: {error}"));
    let connections = registry
        .get("negotiatedConnections")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{REGISTRY} must declare negotiatedConnections"));
    let declared = connections.keys().map(String::as_str).collect::<Vec<_>>();
    let expected = CONNECTIONS.map(|(name, _)| name);
    assert!(
        declared == expected,
        "{REGISTRY} declares negotiated connections {declared:?}; this binding knows {expected:?}"
    );

    let mut source = String::new();
    for (name, prefix) in CONNECTIONS {
        let inputs = connections
            .get(name)
            .and_then(|connection| connection.get("fingerprintInputs"))
            .and_then(Value::as_array)
            .and_then(|inputs| inputs.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
            .unwrap_or_else(|| panic!("{REGISTRY} {name} must list fingerprintInputs strings"));
        for input in &inputs {
            println!(
                "cargo:rerun-if-changed={}",
                repository.join(input).display()
            );
        }
        let digest = fingerprint::compute(repository, &inputs)
            .unwrap_or_else(|error| panic!("fingerprint {name}: {error}"));
        writeln!(source, "const {prefix}_FINGERPRINT: [u8; 32] = {digest:?};")
            .expect("writing to a String cannot fail");
    }
    let out = env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR");
    fs::write(Path::new(&out).join("fingerprints.rs"), source)
        .unwrap_or_else(|error| panic!("write fingerprints.rs: {error}"));
}
