use super::*;
#[test]
fn curl_stream_limit_and_https_resolution_do_not_silently_degrade() {
    for value in [
        "curl 8.4.0 Windows",
        "curl 8.14.1 Windows",
        "curl 9.0.0 Windows",
    ] {
        assert!(bounded_curl(value.as_bytes()));
    }
    for value in [
        "curl 8.3.0 Windows",
        "curl 7.88.0 Windows",
        "curl unknown",
        "not curl 9.0.0",
    ] {
        assert!(!bounded_curl(value.as_bytes()));
    }
    assert!(Source::parse("https://example.test").is_err());
    assert!(Source::parse("http://example.test/release.json").is_err());
    let source = Source::parse("https://example.test/stable/release.json")
        .unwrap_or_else(|_| panic!("valid source"));
    assert!(
        matches!(source.artifact("orchbee.exe"), Ok(Source::Https(url)) if url == "https://example.test/stable/orchbee.exe")
    );

    for (value, expected) in [
        ("C:/release.json", Some("C:/bin/tool.exe")),
        (r"C:\release.json", Some(r"C:\bin/tool.exe")),
        ("C:/feed//./release.json", Some(r"C:/feed\bin/tool.exe")),
        (r"C:\feed\\.\release.json\.", Some(r"C:\feed\bin/tool.exe")),
        (r"C:\\feed\\release.json", Some(r"C:\\feed\bin/tool.exe")),
        ("C:/././release.json", Some("C:/bin/tool.exe")),
        ("D:/资料/发布.json", Some(r"D:/资料\bin/tool.exe")),
        ("C:/feed/...", Some(r"C:/feed\bin/tool.exe")),
        ("C:/feed/ release.json", Some(r"C:/feed\bin/tool.exe")),
        ("C:/", None),
        (r"C:\", None),
        ("C:////.", None),
    ] {
        let source = Source::parse(value).unwrap_or_else(|_| panic!("admitted source: {value}"));
        let Source::Local(path) = source else {
            panic!("local source: {value}");
        };
        assert_eq!(
            path.artifact("bin/tool.exe")
                .as_ref()
                .and_then(|path| path.as_path().to_str()),
            expected,
            "{value}",
        );
        #[cfg(windows)]
        assert_windows_source_compatibility(value, Some(&path));
    }
    for value in [
        "/release.json",
        r"\release.json",
        "C:release.json",
        "C:",
        "1:/release.json",
        "é:/release.json",
        r"\\server\share\release.json",
        r"\\?\C:\release.json",
        r"\\?\C:/release.json",
        r"\\.\C:\release.json",
        "//?/C:/release.json",
        "C:/../release.json",
        "C:/feed/../release.json",
        "C:/release\0.json",
        "C:/release\r.json",
        "C:/release\n.json",
    ] {
        assert!(Source::parse(value).is_err(), "{value}");
        #[cfg(windows)]
        assert_windows_source_compatibility(value, None);
    }

    let longest = format!("C:/{}/release.json", "d".repeat(4096 - 3 - 1 - 12));
    let source = Source::parse(&longest).unwrap_or_else(|_| panic!("source length boundary"));
    assert!(Source::parse(&(longest.clone() + "x")).is_err());
    let Source::Local(path) = source else {
        panic!("local source");
    };
    assert!(
        path.artifact("nested/bin/tool.exe")
            .unwrap()
            .as_path()
            .to_str()
            .unwrap()
            .len()
            > 4096
    );
    #[cfg(windows)]
    assert_windows_source_compatibility(&longest, Some(&path));
}

/// The previous native implementation is an independent witness for the shared lexical model.
#[cfg(windows)]
fn assert_windows_source_compatibility(value: &str, actual: Option<&LocalPath>) {
    use std::path::{Component, PathBuf, Prefix};
    let original = PathBuf::from(value);
    let admitted = value.len() <= 4096
        && !value.contains(['\0', '\r', '\n'])
        && original.is_absolute()
        && matches!(original.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
        && !original
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir));
    assert_eq!(actual.is_some(), admitted, "{value}");
    if let Some(actual) = actual {
        assert_eq!(
            actual
                .artifact("bin/tool.exe")
                .map(|path| path.as_path().as_os_str().to_owned()),
            original
                .parent()
                .map(|parent| parent.join("bin/tool.exe").into_os_string()),
            "{value}",
        );
        assert_eq!(
            actual.parent_and_name(),
            original
                .parent()
                .and_then(Path::to_str)
                .zip(original.file_name().and_then(|name| name.to_str())),
            "{value}",
        );
    }
}
