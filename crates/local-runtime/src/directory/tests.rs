use super::posix_reported_directory;

fn unnamed() -> Option<String> {
    None
}

#[test]
fn posix_reports_keep_absolute_paths_and_decode_local_uris_once() {
    for (raw, expected) in [
        ("/home/a b/项目", "/home/a b/项目"),
        ("/", "/"),
        ("file:///home/one%20two", "/home/one two"),
        ("file://localhost/srv/a+b", "/srv/a+b"),
        ("file://LocalHost/srv", "/srv"),
        ("file:///tmp/a%2520b", "/tmp/a%20b"),
        ("file:///%E9%A1%B9%E7%9B%AE", "/项目"),
    ] {
        assert_eq!(
            posix_reported_directory(raw, unnamed).as_deref(),
            Some(expected),
            "{raw}"
        );
    }
    let bench = || Some("bench".to_owned());
    assert_eq!(
        posix_reported_directory("file://Bench/home/work", bench).as_deref(),
        Some("/home/work")
    );
}

#[test]
fn posix_reports_refuse_other_machines_and_malformed_values() {
    let bench = || Some("bench".to_owned());
    for raw in [
        "",
        "relative",
        "C:\\work",
        "file:relative",
        "file://localhost",
        "file://remote/home/work",
        "file://bench.example/home/work",
        "file:///tmp/bad%",
        "file:///tmp/%ff",
        "file:///tmp/%00",
        "file:///tmp/%0a",
        "file:///tmp/a?x",
        "file:///tmp/a#b",
        "/tmp/\u{1b}]0;title",
    ] {
        assert!(posix_reported_directory(raw, bench).is_none(), "{raw}");
    }
    assert!(posix_reported_directory("file://remote/home/work", unnamed).is_none());
    assert!(
        posix_reported_directory(
            &format!("/{}", "a".repeat(super::OBSERVATION_LIMIT)),
            unnamed
        )
        .is_none()
    );
}

// The shell's OSC 7 report and the Host's procfs directory reach every caller through this
// machine's rules.
#[cfg(unix)]
#[test]
fn this_machine_reads_posix_reports() {
    assert_eq!(
        super::reported_directory("file://localhost/home/work").as_deref(),
        Some("/home/work")
    );
    assert_eq!(
        super::reported_directory("/proc/self/cwd").as_deref(),
        Some("/proc/self/cwd")
    );
    assert!(super::reported_directory("C:\\work").is_none());
}
