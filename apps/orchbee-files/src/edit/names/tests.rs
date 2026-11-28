//! Names by this machine's rules, and VS Code's copy names.

use super::*;

#[cfg(windows)]
const FOLDER: &str = r"C:\work";
#[cfg(not(windows))]
const FOLDER: &str = "/work";

fn code(result: Result<NativePath, FilesError>) -> Option<ErrorCode> {
    result.err().map(|error| error.code)
}

#[test]
fn a_name_this_machine_cannot_hold_is_invalid() {
    let folder = NativePath::parse(FOLDER).unwrap();
    let mut invalid = vec!["", ".", "..", "a/b"];
    // Windows refuses its reserved characters, control characters, a trailing dot or space and the
    // device names in every folder, with any extension; macOS and Linux hold each of them.
    let windows = [
        r"a\b",
        "a<b",
        "a>b",
        "a:b",
        "a\"b",
        "a|b",
        "a?b",
        "a*b",
        "a\tb",
        "name.",
        "name ",
        "CON",
        "con.txt",
        "Nul",
        "aux.tar.gz",
        "COM1",
        "lpt9.log",
        "COM\u{b9}",
        "conin$",
    ];
    if cfg!(windows) {
        invalid.extend(windows);
    } else {
        for name in windows {
            assert!(child(&folder, name).is_ok(), "{name:?}");
        }
    }
    for name in invalid {
        assert_eq!(
            code(child(&folder, name)),
            Some(ErrorCode::InvalidName),
            "{name:?}"
        );
    }
    for name in [
        "a.txt",
        ".gitignore",
        " leading",
        "con-sole",
        "COM10",
        "\u{4e2d}\u{6587}",
    ] {
        assert!(child(&folder, name).is_ok(), "{name:?}");
    }

    // A new path is judged by its final name, and its folder by the path rules.
    let created_at = |path: String| code(created(&FilePath::new(path).unwrap()));
    assert_eq!(
        created_at(format!("{FOLDER}{SEPARATOR}..")),
        Some(ErrorCode::InvalidName)
    );
    assert_eq!(created_at(format!("{FOLDER}{SEPARATOR}a.txt")), None);
    if cfg!(windows) {
        assert_eq!(
            created_at(r"C:\aux".to_owned()),
            Some(ErrorCode::InvalidName)
        );
        assert_eq!(
            created_at(r"C:\name.\a".to_owned()),
            Some(ErrorCode::InvalidRequest)
        );
        let unc = created_at(r"\\server\share\a".to_owned());
        assert_eq!(unc, Some(ErrorCode::UnsupportedLocation));
    } else {
        assert_eq!(
            created_at("relative/a".to_owned()),
            Some(ErrorCode::InvalidRequest)
        );
    }
}

#[test]
fn a_copy_takes_vs_code_next_name() {
    let files = [
        ("test.js", "test copy.js"),
        ("test copy.js", "test copy 2.js"),
        ("test copy 005.js", "test copy 6.js"),
        (
            "test copy 9007199254740992.js",
            "test copy 9007199254740992 copy.js",
        ),
        ("copy.js", "copy copy.js"),
        ("copy 2.js", "copy 2 copy.js"),
        ("test", "test copy"),
        ("test.", "test copy."),
        (".test", ".test copy"),
        ("..test", ". copy.test"),
        ("archive.tar.gz", "archive.tar copy.gz"),
        ("test copy 5", "test copy 6"),
        ("x copy 2 copy", "x copy 2 copy 2"),
        ("a copy  3", "a copy  3 copy"),
        (" copy", " copy copy"),
    ];
    for (name, next) in files {
        assert_eq!(increment(name, false), next, "{name:?}");
    }
    let folders = [
        ("test", "test copy"),
        ("test copy", "test copy 2"),
        ("test copy 005", "test copy 6"),
        ("test copy 1073741823", "test copy 1073741824"),
        ("test copy 1073741824", "test copy 1073741824 copy"),
        ("copy 2", "copy 2 copy"),
        ("test.js", "test.js copy"),
        ("test.js copy 5", "test.js copy 6"),
        ("test copy 0", "test copy"),
        ("test copy 1", "test copy 2"),
    ];
    for (name, next) in folders {
        assert_eq!(increment(name, true), next, "{name:?}");
    }
}
