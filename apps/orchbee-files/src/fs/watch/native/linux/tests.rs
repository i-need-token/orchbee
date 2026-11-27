//! The walk of a recursive watch on a real tree.

use std::{fs, path::PathBuf};

use super::*;

struct Tree(PathBuf);

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_walk_leaves_out_excluded_folders_and_links_and_keeps_to_its_limit() {
    let base = std::env::temp_dir().join(format!("orchbee-files-walk-{}", std::process::id()));
    let tree = Tree(base.clone());
    for folder in [
        "src/deep",
        ".git/objects/ab",
        ".git/refs",
        "sub/.git/objects",
        "sub/inner/.git/objects",
        "sub/.hg/store",
    ] {
        fs::create_dir_all(base.join(folder)).unwrap();
    }
    std::os::unix::fs::symlink(base.join("src"), base.join("link")).unwrap();
    let root = NativePath::parse(base.to_str().unwrap()).unwrap();

    let mut walked: Vec<String> = directories(&root, 100)
        .unwrap()
        .iter()
        .map(|folder| folder.as_str()[root.as_str().len()..].to_owned())
        .collect();
    walked.sort();
    // Excluded at the top and one level down, not deeper; the link is not followed.
    assert_eq!(
        walked,
        [
            "",
            "/.git",
            "/.git/refs",
            "/src",
            "/src/deep",
            "/sub",
            "/sub/.git",
            "/sub/.hg",
            "/sub/inner",
            "/sub/inner/.git",
            "/sub/inner/.git/objects",
        ]
    );
    assert_eq!(directories(&root, 10), None);
    drop(tree);
}
