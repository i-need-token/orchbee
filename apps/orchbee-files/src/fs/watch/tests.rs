//! The folders a recursive watch leaves out.

use orchbee_files_protocol::Separator;

use super::*;
use crate::scope::path;

fn native(relative: &str) -> NativePath {
    let (base, separator) = match path::SEPARATOR {
        Separator::Backslash => (r"C:\w", "\\"),
        Separator::Slash => ("/w", "/"),
    };
    let mut native = base.to_owned();
    for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
        native.push_str(separator);
        native.push_str(segment);
    }
    NativePath::parse(&native).unwrap()
}

#[test]
fn a_recursive_watch_leaves_out_vs_codes_folders_at_its_top_and_one_level_down() {
    let tree = native("");
    let left_out = |relative| excluded(&tree, &native(relative));
    assert!(left_out(".git/objects"));
    assert!(left_out(".git/objects/ab/cdef"));
    assert!(left_out("sub/.git/subtree-cache/x"));
    assert!(left_out("sub/.hg/store"));
    assert!(!left_out(".git"));
    assert!(!left_out(".git/refs/heads"));
    assert!(!left_out("a/b/.git/objects"));
    assert!(!left_out("src/objects"));
}
