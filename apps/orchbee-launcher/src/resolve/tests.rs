//! The launcher's pointer-to-command resolution: which Client a pointer names, which pointer bytes
//! name none, which part of a command line passes on unchanged, and which failed Client is recorded.
use super::{Failure, client, command_tail, failure_recorder};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn pointer(product: &str) -> String {
    format!(r#"{{"version":1,"active":{{"product":"{product}","manifest":"{DIGEST}"}}}}"#)
}

/// Resolves `launcher` with `bytes` as the pointer, recording which file the launcher asked for.
fn resolve(launcher: &str, bytes: Option<&str>) -> (Result<PathBuf, Failure>, Vec<PathBuf>) {
    let asked = RefCell::new(Vec::new());
    let result = client(Path::new(launcher), |path| {
        asked.borrow_mut().push(path.to_path_buf());
        bytes.map(|text| text.as_bytes().to_vec())
    });
    (result, asked.into_inner())
}

#[test]
fn the_active_entry_names_the_client_of_the_launchers_own_name() {
    let previous = format!(
        r#"{{"version":1,"active":{{"product":"0.2.0-beta.1","manifest":"{DIGEST}"}},"previous":{{"product":"0.1.0","manifest":"{DIGEST}"}},"extensions":{{"later":true}}}}"#
    );
    for (launcher, bytes, client) in [
        (
            r"C:\Store\bin\orchbee.exe",
            pointer("0.1.0"),
            r"C:\Store\versions\0.1.0\orchbee.exe",
        ),
        (
            r"C:\Store\bin\ob-test-0123456789abcdef.exe",
            pointer("1.20.3"),
            r"C:\Store\versions\1.20.3\ob-test-0123456789abcdef.exe",
        ),
        (
            r"D:\with space\bin\orchbee.exe",
            previous,
            r"D:\with space\versions\0.2.0-beta.1\orchbee.exe",
        ),
    ] {
        let (result, asked) = resolve(launcher, Some(&bytes));
        assert_eq!(result, Ok(PathBuf::from(client)), "{launcher}");
        let root = Path::new(launcher).parent().unwrap().parent().unwrap();
        assert_eq!(asked, [root.join("active.json")], "{launcher}");
    }
}

#[test]
fn a_missing_or_invalid_pointer_names_no_active_version() {
    let missing = resolve(r"C:\Store\bin\orchbee.exe", None).0;
    assert_eq!(
        missing,
        Err(Failure::NoActiveVersion(PathBuf::from(
            r"C:\Store\active.json"
        )))
    );
    for bytes in [
        String::new(),
        pointer("0.1.0")[..30].to_owned(),
        pointer("0.1.0").replace(r#""version":1"#, r#""version":2"#),
        pointer(r"..\\..\\Windows"),
        pointer("0.1"),
        pointer("0.1.0+build.7"),
        pointer("0.1.0").replace(DIGEST, &DIGEST.to_uppercase()),
        pointer("0.1.0").replace('}', r#","unknown":1}"#),
        format!(
            r#"{{"version":1,"active":{{"product":"0.1.0","manifest":"{DIGEST}"}},"active":{{"product":"0.2.0","manifest":"{DIGEST}"}}}}"#
        ),
    ] {
        assert_eq!(
            resolve(r"C:\Store\bin\orchbee.exe", Some(&bytes)).0,
            Err(Failure::NoActiveVersion(PathBuf::from(
                r"C:\Store\active.json"
            ))),
            "{bytes}"
        );
    }
}

// A failed Client is recorded through the previous version only while the pointer still makes the
// failed version active through a counting move that may fall back.
#[test]
fn a_failed_client_is_recorded_through_the_previous_version_only_while_its_move_counts() {
    let entry = |product: &str| format!(r#"{{"product":"{product}","manifest":"{DIGEST}"}}"#);
    let pointer = |active: &str, previous: Option<&str>, moved: &str, back: bool| {
        let previous = previous.map_or_else(String::new, |previous| {
            format!(r#","previous":{}"#, entry(previous))
        });
        format!(
            r#"{{"version":1,"active":{}{previous},"extensions":{{"fallback":{{"held":[],"move":{{"entry":{},"back":{back}}}}}}}}}"#,
            entry(active),
            entry(moved)
        )
    };
    let client = Path::new(r"C:\Store\versions\0.2.0\orchbee.exe");
    let counting = pointer("0.2.0", Some("0.1.0"), "0.2.0", true);
    assert_eq!(
        failure_recorder(client, Some(counting.as_bytes())),
        Some((
            PathBuf::from(r"C:\Store\versions\0.1.0\orchbee.exe"),
            "0.2.0".to_owned()
        ))
    );
    for (case, bytes) in [
        (
            "may not fall back",
            pointer("0.2.0", Some("0.1.0"), "0.2.0", false),
        ),
        ("no previous", pointer("0.2.0", None, "0.2.0", true)),
        (
            "a move for another entry",
            pointer("0.2.0", Some("0.1.0"), "0.1.0", true),
        ),
        (
            "another version active",
            pointer("0.3.0", Some("0.2.0"), "0.3.0", true),
        ),
        (
            "no fallback record",
            pointer("0.2.0", Some("0.1.0"), "0.2.0", true).replace("fallback", "later"),
        ),
        ("invalid", counting[..40].to_owned()),
    ] {
        assert_eq!(
            failure_recorder(client, Some(bytes.as_bytes())),
            None,
            "{case}"
        );
    }
    assert_eq!(failure_recorder(client, None), None);
}

#[test]
fn the_command_tail_passes_on_exactly_as_written() {
    for (line, tail) in [
        ("orchbee", ""),
        ("orchbee ", ""),
        ("orchbee session list", "session list"),
        ("orchbee \t  --format json  ", "--format json  "),
        (
            r#""C:\Program Files\Orchbee\bin\orchbee.exe" "a b" "" c"#,
            r#""a b" "" c"#,
        ),
        (r#"orchbee "" """#, r#""" """#),
        (r#"orchbee a"b c"d \"e\" \\"#, r#"a"b c"d \"e\" \\"#),
        ("orchbee 中文 \"é ü\" 🐝", "中文 \"é ü\" 🐝"),
        (r#""C:\a b"c d"#, "d"),
        (r#"C:\x"y z"\orchbee.exe tail"#, "tail"),
        (r#""unterminated quote"#, ""),
        ("", ""),
    ] {
        assert_eq!(command_tail(line), tail, "{line}");
    }
}
