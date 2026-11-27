use orchbee_files_protocol::ErrorCode;

use super::NativePath;

fn code(path: &str) -> Option<ErrorCode> {
    NativePath::parse(path).err().map(|error| error.code)
}

fn path(text: &str) -> NativePath {
    NativePath::parse(text).unwrap()
}

#[cfg(not(windows))]
#[test]
fn a_client_names_only_absolute_paths_without_dot_or_empty_segments() {
    for admitted in ["/", "/a", "/a/b c/.d", "/a/b..", "/~"] {
        assert_eq!(code(admitted), None, "{admitted:?}");
    }
    for refused in [
        "a", "./a", "/a/", "//a", "/a//b", "/a/./b", "/a/../b", "/a/..", "~/a",
    ] {
        assert_eq!(
            code(refused),
            Some(ErrorCode::InvalidRequest),
            "{refused:?}"
        );
    }
    assert_eq!(path("/a/b").name(), "b");
    assert_eq!(path("/").name(), "");
}

#[cfg(windows)]
#[test]
fn a_client_names_only_drive_paths_that_win32_opens_as_named() {
    for admitted in [
        r"C:\",
        r"c:\a",
        r"C:\a\b c\.d",
        r"C:\a\b..c",
        r"C:\COMA\console",
    ] {
        assert_eq!(code(admitted), None, "{admitted:?}");
    }
    for refused in [
        "a",
        r"C:",
        r"C:a",
        r"\a",
        "C:/a",
        r"C:\a\",
        r"C:\a\\b",
        r"C:\a\.\b",
        r"C:\a\..\b",
        r"C:\a.",
        r"C:\a ",
        r"C:\a:stream",
        r"C:\a*",
        r"C:\a\CON",
        r"C:\a\nul.txt",
        r"C:\a\COM0",
        r"C:\a\lpt9.log",
        r"C:\a\COM¹",
        r"C:\a\CONOUT$",
        "C:\\a\u{1}",
        r"\\?\C:\a",
        r"\\.\PhysicalDrive0",
    ] {
        assert_eq!(
            code(refused),
            Some(ErrorCode::InvalidRequest),
            "{refused:?}"
        );
    }
    assert_eq!(
        code(r"\\server\share\a"),
        Some(ErrorCode::UnsupportedLocation)
    );
    assert_eq!(path(r"C:\a\b").name(), "b");
    assert_eq!(path(r"C:\").name(), "");
}

#[test]
fn containment_compares_whole_segments_by_the_machine_rule() {
    #[cfg(not(windows))]
    let (root, below, sibling, prefix, other_case) = (
        "/w/project",
        "/w/project/src/a",
        "/w/projects/a",
        "/w",
        "/w/Project/a",
    );
    #[cfg(windows)]
    let (root, below, sibling, prefix, other_case) = (
        r"C:\w\project",
        r"C:\w\project\src\a",
        r"C:\w\projects\a",
        r"C:\w",
        r"c:\W\Project\a",
    );
    let root = path(root);
    assert!(path(below).is_within(&root));
    assert!(root.is_within(&root));
    // A longer name that starts with the member's last segment is not inside it.
    assert!(!path(sibling).is_within(&root));
    assert!(!path(prefix).is_within(&root));
    assert_eq!(path(other_case).is_within(&root), !super::CASE_SENSITIVE);
    assert_eq!(
        path(other_case).key()
            == path(&format!("{}{}a", root.as_str(), std::path::MAIN_SEPARATOR)).key(),
        !super::CASE_SENSITIVE
    );
}
