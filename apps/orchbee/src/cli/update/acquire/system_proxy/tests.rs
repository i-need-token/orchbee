use super::{Settings, Value, arguments};
use std::{cell::Cell, ffi::OsString};

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn on(server: Value, bypass: Value) -> Settings {
    Settings {
        enabled: Value::Number(1),
        server,
        bypass,
    }
}

fn no_environment(_: &str) -> Option<OsString> {
    None
}

fn expected(server: Option<&str>, bypass: Option<&str>) -> Vec<String> {
    let mut arguments = Vec::new();
    if let Some(server) = server {
        arguments.extend(["--proxy".to_owned(), server.to_owned()]);
    }
    if let Some(bypass) = bypass {
        arguments.extend(["--noproxy".to_owned(), bypass.to_owned()]);
    }
    arguments
}

/// The mapping from the per-user settings to curl's arguments, where reading Windows' formats can
/// break: the on flag, one server or a per-scheme list, the bypass list, and values that are not
/// understood, which must give no argument rather than a failing download.
#[test]
fn system_proxy_settings_map_to_curl_arguments() {
    let cases: Vec<(&str, Settings, Vec<String>)> = vec![
        // The on flag.
        (
            "off",
            Settings {
                enabled: Value::Number(0),
                ..on(text("proxy.corp:8080"), Value::Absent)
            },
            expected(None, None),
        ),
        (
            "flag absent",
            Settings {
                enabled: Value::Absent,
                ..on(text("proxy.corp:8080"), Value::Absent)
            },
            expected(None, None),
        ),
        (
            "flag as text",
            Settings {
                enabled: text("1"),
                ..on(text("proxy.corp:8080"), Value::Absent)
            },
            expected(None, None),
        ),
        (
            "flag of another type",
            Settings {
                enabled: Value::Other,
                ..on(text("proxy.corp:8080"), Value::Absent)
            },
            expected(None, None),
        ),
        (
            "any nonzero flag",
            Settings {
                enabled: Value::Number(7),
                ..on(text("proxy.corp:8080"), Value::Absent)
            },
            expected(Some("http://proxy.corp:8080"), None),
        ),
        // One server for every scheme.
        (
            "single server",
            on(text(" proxy.corp:8080 "), Value::Absent),
            expected(Some("http://proxy.corp:8080"), None),
        ),
        (
            "single server, Windows' default port",
            on(text("proxy.corp"), Value::Absent),
            expected(Some("http://proxy.corp:80"), None),
        ),
        (
            "single server with an http prefix",
            on(text("HTTP://127.0.0.1:7890"), Value::Absent),
            expected(Some("http://127.0.0.1:7890"), None),
        ),
        (
            "single IPv6 server",
            on(text("[::1]:3128"), Value::Absent),
            expected(Some("http://[::1]:3128"), None),
        ),
        // Per-scheme lists.
        (
            "per-scheme list takes https",
            on(
                text("http=a.corp:1;https=b.corp:2;ftp=c.corp:3;socks=d.corp:4"),
                Value::Absent,
            ),
            expected(Some("http://b.corp:2"), None),
        ),
        (
            "per-scheme names ignore case and spaces",
            on(text(" HTTPS = b.corp:2 ; "), Value::Absent),
            expected(Some("http://b.corp:2"), None),
        ),
        (
            "per-scheme list without https takes socks",
            on(text("http=a.corp:1;socks=d.corp"), Value::Absent),
            expected(Some("socks4://d.corp:1080"), None),
        ),
        (
            "per-scheme list for other schemes only connects directly",
            on(text("http=a.corp:1;ftp=c.corp:3"), Value::Absent),
            expected(None, None),
        ),
        // The bypass list.
        (
            "bypass list",
            on(
                text("proxy.corp:8080"),
                text(
                    "<local>;*.corp.example;10.*;172.16.*.*;192.168.1.*;intranet;example.org;\
                     10.0.0.7;;bad entry;fe80::1;*foo*;10.*.1.*;999.*;http://x.example;a.b:8080",
                ),
            ),
            expected(
                Some("http://proxy.corp:8080"),
                Some(
                    "corp.example,10.0.0.0/8,172.16.0.0/16,192.168.1.0/24,intranet,example.org,\
                     10.0.0.7",
                ),
            ),
        ),
        (
            "bypass of <local> alone passes no list",
            on(text("proxy.corp:8080"), text("<LOCAL>")),
            expected(Some("http://proxy.corp:8080"), None),
        ),
        (
            "bypass of every host",
            on(text("proxy.corp:8080"), text("*")),
            expected(Some("http://proxy.corp:8080"), Some("*")),
        ),
        (
            "bypass of another type",
            on(text("proxy.corp:8080"), Value::Other),
            expected(None, None),
        ),
        // Servers that are not understood.
        (
            "server absent",
            on(Value::Absent, Value::Absent),
            expected(None, None),
        ),
        (
            "server as a number",
            on(Value::Number(8080), Value::Absent),
            expected(None, None),
        ),
        (
            "server of another type",
            on(Value::Other, Value::Absent),
            expected(None, None),
        ),
    ];
    for (name, settings, wanted) in cases {
        assert_eq!(
            arguments(&no_environment, || Some(settings)),
            wanted,
            "{name}"
        );
    }

    for server in [
        "",
        "   ",
        "proxy.corp:",
        "proxy.corp:0",
        "proxy.corp:65536",
        "proxy.corp:port",
        "proxy.corp:80:81",
        "user:secret@proxy.corp:8080",
        "socks5://proxy.corp:1080",
        "https://proxy.corp:443",
        "proxy.corp:8080/path",
        "pro xy.corp:8080",
        ".corp:8080",
        "[]:8080",
        "[::1",
        "[::1]8080",
        "a.corp:1;b.corp:2",
        "http=a.corp:1;b.corp:2",
        "https=bad entry;socks=d.corp:4",
        "https=",
    ] {
        assert_eq!(
            arguments(&no_environment, || Some(on(text(server), Value::Absent))),
            expected(None, None),
            "{server:?}",
        );
    }
    assert_eq!(arguments(&no_environment, || None), expected(None, None));
}

/// `HTTPS_PROXY` and `ALL_PROXY` choose the proxy themselves, and the settings are then not even
/// read; `NO_PROXY` stays curl's bypass list, so the system list is not passed over it.
#[test]
fn the_environment_wins_over_the_system_proxy() {
    let settings = || on(text("proxy.corp:8080"), text("example.org"));
    for variable in ["HTTPS_PROXY", "ALL_PROXY"] {
        let read = Cell::new(false);
        let environment = |name: &str| (name == variable).then(OsString::new);
        let found = arguments(&environment, || {
            read.set(true);
            Some(settings())
        });
        assert_eq!(found, expected(None, None), "{variable}");
        assert!(!read.get(), "{variable}");
    }
    let environment = |name: &str| (name == "NO_PROXY").then(|| OsString::from("localhost"));
    assert_eq!(
        arguments(&environment, || Some(settings())),
        expected(Some("http://proxy.corp:8080"), None),
    );
    assert_eq!(
        arguments(&no_environment, || Some(settings())),
        expected(Some("http://proxy.corp:8080"), Some("example.org")),
    );
}
