use super::*;

#[test]
fn windows_wire_paths_preserve_spelling_and_reject_ambiguous_roots() {
    for value in [
        r"C:\",
        r"c:/work/../工作",
        r"\\?\C:\work with spaces",
        "/work",
    ] {
        assert_eq!(
            WorkbenchDirectoryPath::parse(value).unwrap().as_str(),
            value
        );
    }
    for value in [
        "",
        "relative",
        "C:",
        r"C:work",
        r"\work",
        r"\\server\share",
        r"\\.\pipe\x",
        r"\\?\UNC\server\share",
    ] {
        assert!(WorkbenchDirectoryPath::parse(value).is_err(), "{value}");
    }
}

#[test]
fn windows_checkout_spelling_matches_without_inventing_filesystem_identity() {
    assert!(same_windows_path_spelling(
        r"C:\Trees\Repo\Feature\",
        "c:/trees/repo/feature"
    ));
    for other in [
        "C:/trees/repo/other",
        "D:/trees/repo/feature",
        r"\\?\C:\trees\repo\feature",
        "C:/trees/repo/../repo/feature",
    ] {
        assert!(!same_windows_path_spelling("C:/trees/repo/feature", other));
    }
    assert!(!same_windows_path_spelling("C:/工作/Ä", "C:/工作/ä"));
    assert!(!same_windows_path_spelling("C:/repo/../other", "C:/other"));
    assert!(!same_windows_path_spelling(
        "C:/repo//other",
        "C:/repo/other"
    ));
}
