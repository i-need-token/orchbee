//! How a reported directory compares with the one a line expects.
use super::same_directory;

#[cfg(windows)]
#[test]
fn directories_compare_case_insensitively_with_one_trailing_separator() {
    assert!(same_directory("C:\\Work\\Project", "c:\\work\\project\\"));
    assert!(same_directory("C:\\work\\project/", "C:\\work\\project"));
    assert!(same_directory("C:\\", "c:\\"));
    // A drive root keeps its separator: `C:` alone is the drive's current directory.
    assert!(!same_directory("C:\\", "C:"));
    assert!(!same_directory(
        "C:\\work\\project\\\\",
        "C:\\work\\project"
    ));
    assert!(!same_directory("C:\\work\\project", "C:\\work\\projects"));
    assert!(same_directory("\\\\server\\share\\", "\\\\SERVER\\share"));
}

#[cfg(unix)]
#[test]
fn posix_directories_match_by_spelling_or_by_the_directory_they_name() {
    use std::{fs, os::unix::fs::symlink};
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = std::env::temp_dir().join(format!(
        "orchbee-prompt-directory-{:032x}",
        u128::from_le_bytes(nonce)
    ));
    let project = root.join("project");
    let sibling = root.join("projects");
    let link = root.join("link");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir(&sibling).unwrap();
    symlink(&project, &link).unwrap();
    let text = |path: &std::path::Path| path.to_str().unwrap().to_owned();
    let canonical = project.canonicalize().unwrap();
    let checks = [
        // One trailing separator is trimmed, but never the root's.
        same_directory("/work/project/", "/work/project"),
        same_directory("/", "/"),
        // A link and its target, and the canonical spelling, name the same directory.
        same_directory(&text(&link), &text(&project)),
        same_directory(&text(&canonical), &text(&project)),
        !same_directory(&text(&sibling), &text(&project)),
        // Paths that name nothing match only by spelling.
        !same_directory("/work/project", "/work/projects"),
        !same_directory("/work/project//", "/work/project"),
        // A relative path is never resolved against the Server's own directory.
        !same_directory(".", &text(&std::env::current_dir().unwrap())),
    ];
    fs::remove_file(&link).unwrap();
    fs::remove_dir(&sibling).unwrap();
    fs::remove_dir(&project).unwrap();
    fs::remove_dir(&root).unwrap();
    assert_eq!(checks, [true; 8]);
}
