//! How a reported directory compares with the one a line expects.
use super::same_directory;

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
