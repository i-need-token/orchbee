//! The fingerprint algorithm against reference values computed without it.

use std::{fs, io, path::PathBuf};

use crate::{algorithm, support::TestResult};

/// Files of the reference tree with their exact bytes. The tree exercises bytewise path order
/// (`a-b.rs` before `a/b.rs`), the extension filter, both excluded components, and the excluded
/// file name.
const TREE: [(&str, &str); 11] = [
    ("schemas/demo/a.json", "{\"a\":1}\n"),
    ("schemas/demo/README.md", "ignored\n"),
    ("schemas/demo/tests/x.json", "[]\n"),
    ("crates/demo/src/lib.rs", "pub fn f() {}\n"),
    ("crates/demo/src/a-b.rs", "// a-b\n"),
    ("crates/demo/src/a/b.rs", "// a/b\n"),
    ("crates/demo/src/codec.rs", "mod tests;\n"),
    ("crates/demo/src/codec/tests.rs", "#[test]\nfn t() {}\n"),
    ("apps/host/src/__tests__/t.zig", "test {}\n"),
    ("apps/host/src/main.zig", "pub fn main() void {}\n"),
    ("apps/host/src/build.zon", ".{}\n"),
];

/// Computed in Git Bash with coreutils, independently of this crate: `printf` wrote `TREE`, then
///
/// ```sh
/// { printf 'orchbee-protocol-fingerprint-v1\n'
///   for p in apps/host/src/main.zig crates/demo/src/a-b.rs crates/demo/src/a/b.rs \
///            crates/demo/src/codec.rs crates/demo/src/lib.rs schemas/demo/a.json; do
///     printf '%s\0%s\n' "$p" "$(sha256sum "$p" | cut -d' ' -f1)"
///   done; } | sha256sum
/// ```
const TREE_FINGERPRINT: &str = "050bbc0a75d1b0986dbdc39ed05397931626c18877b81cd35ab4ca792a2a28c0";
/// `printf 'orchbee-protocol-fingerprint-v1\n' | sha256sum`: the fingerprint of no input file.
const EMPTY_FINGERPRINT: &str = "a045c32a35d1cd926e262bd91bba443c4466bc54c1e387ce3b833831de2e019c";

#[test]
fn fingerprint_matches_independent_reference_values() -> TestResult {
    let root = ScratchTree::new()?;
    for (path, contents) in TREE {
        let native = path
            .split('/')
            .fold(root.0.clone(), |native, component| native.join(component));
        fs::create_dir_all(native.parent().ok_or("a tree file has a parent")?)?;
        fs::write(native, contents)?;
    }
    let directories = ["apps/host/src", "crates/demo/src", "schemas/demo"];
    assert_eq!(
        hex(algorithm::compute(&root.0, &directories)?),
        TREE_FINGERPRINT
    );
    assert_eq!(hex(algorithm::compute(&root.0, &[])?), EMPTY_FINGERPRINT);
    Ok(())
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A fresh directory under the system temporary directory, removed when dropped.
struct ScratchTree(PathBuf);

impl ScratchTree {
    fn new() -> io::Result<Self> {
        let base = std::env::temp_dir();
        for attempt in 0..100_u32 {
            let path = base.join(format!(
                "orchbee-connection-handshake-{}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no free fingerprint scratch directory",
        ))
    }
}

impl Drop for ScratchTree {
    fn drop(&mut self) {
        // Best effort: a leftover scratch tree under the temporary directory harms nothing.
        let _ = fs::remove_dir_all(&self.0);
    }
}
