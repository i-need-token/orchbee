use super::{MAXIMUM_WORKING_DIRECTORY_BYTES, WorkingDirectory, WorkingDirectoryError};

#[test]
fn validates_absolute_posix_paths_by_utf8_byte_length_without_normalizing() {
    for valid in ["/", "/tmp/../work", "//node//path", "/tmp/工作"] {
        assert_eq!(
            WorkingDirectory::parse(valid).map(|path| path.to_string()),
            Ok(valid.to_owned())
        );
    }

    let maximum_ascii = format!("/{}", "a".repeat(MAXIMUM_WORKING_DIRECTORY_BYTES - 1));
    assert!(WorkingDirectory::parse(&maximum_ascii).is_ok());
    assert_eq!(
        WorkingDirectory::parse(&format!("{maximum_ascii}a")),
        Err(WorkingDirectoryError::TooLong {
            actual: MAXIMUM_WORKING_DIRECTORY_BYTES + 1,
            maximum: MAXIMUM_WORKING_DIRECTORY_BYTES,
        })
    );

    let maximum_multibyte = format!("/{}", "界".repeat(1365));
    assert_eq!(maximum_multibyte.len(), MAXIMUM_WORKING_DIRECTORY_BYTES);
    assert!(WorkingDirectory::parse(&maximum_multibyte).is_ok());
    let oversized_multibyte = format!("{maximum_multibyte}界");
    assert_eq!(
        WorkingDirectory::parse(&oversized_multibyte),
        Err(WorkingDirectoryError::TooLong {
            actual: MAXIMUM_WORKING_DIRECTORY_BYTES + 3,
            maximum: MAXIMUM_WORKING_DIRECTORY_BYTES,
        })
    );

    assert_eq!(
        WorkingDirectory::parse("relative/path"),
        Err(WorkingDirectoryError::NotAbsolute)
    );
    assert_eq!(
        WorkingDirectory::parse(""),
        Err(WorkingDirectoryError::NotAbsolute)
    );
    assert_eq!(
        WorkingDirectory::parse("/has\0nul"),
        Err(WorkingDirectoryError::ContainsNul)
    );
}

#[test]
fn windows_wire_paths_preserve_spelling_and_reject_ambiguous_roots() {
    for value in [
        r"C:\",
        r"c:/work/../工作",
        r"\\?\C:\work with spaces",
        "/work",
    ] {
        assert_eq!(WorkingDirectory::parse(value).unwrap().as_str(), value);
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
        assert!(WorkingDirectory::parse(value).is_err(), "{value}");
    }
}
