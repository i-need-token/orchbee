//! Release path admission is independent of the filesystem adapter.
use super::valid_release_path;

#[test]
fn release_paths_reject_windows_aliases_and_escape() {
    for value in [
        "../x", "/x", "x\\y", "C:/x", "x:stream", "nul.exe", "a/COM1", "x.", "a//b", ".", "a/../b",
    ] {
        assert!(!valid_release_path(value), "{value}");
    }
    assert!(valid_release_path(
        "agent-observation-adapters/bundle/package.json"
    ));
}
