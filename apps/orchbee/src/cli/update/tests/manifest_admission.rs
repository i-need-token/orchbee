//! Version 2 manifest admission after verification: valid, boundary and invalid documents.
use super::super::{manifest::Release, signature::TrustedManifest};
use orchbee_local_runtime::UpdateChannel;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PLATFORM: &str = "windows-x86_64";
const FILE_LIMIT: u64 = 512 * 1024 * 1024;

fn file(path: &str) -> Value {
    json!({"path": path, "url": path, "sha256": "a".repeat(64), "size": 100})
}

fn sample() -> Value {
    let generations = json!({"generations": {"lowest": 1, "highest": 1}});
    let files = [
        "orchbee.exe",
        "orchbee-server.exe",
        "orchbee-terminal-host.exe",
        "cohort.json",
        "agent-observation-adapters/capsule.json",
        "conpty/conpty.dll",
    ]
    .map(file);
    json!({
        "schemaVersion": 2, "product": "orchbee", "channel": "stable", "platform": PLATFORM,
        "version": "0.1.0", "buildId": "test-1", "files": files,
        "components": [
            {"id": "client", "version": "0.1.0", "files": ["orchbee.exe"],
                "connections": {"client-server": generations}},
            {"id": "terminal-host", "version": "0.1.0", "securityFloor": "0.1.0",
                "recommendedFloor": "0.1.0-rc.1",
                "files": ["orchbee-terminal-host.exe", "conpty/conpty.dll"],
                "connections": {"worker-host": generations, "obtf": generations}},
        ],
    })
}

fn parse_bytes(bytes: &[u8]) -> Option<Release> {
    Release::parse(TrustedManifest::installed(bytes), PLATFORM)
}

fn parse(value: &Value) -> Option<Release> {
    parse_bytes(&serde_json::to_vec(value).unwrap())
}

/// `sample` with `change` applied.
fn with(change: impl FnOnce(&mut Value)) -> Value {
    let mut value = sample();
    change(&mut value);
    value
}

#[test]
fn a_complete_manifest_is_admitted_with_its_identity_and_every_listed_file() {
    let bytes = serde_json::to_vec(&sample()).unwrap();
    let release = parse_bytes(&bytes).unwrap();
    assert_eq!(release.channel, UpdateChannel::Stable);
    assert_eq!(release.identity, format!("{:x}", Sha256::digest(&bytes)));
    assert_eq!(release.encoded, bytes);
    assert_eq!(
        (release.version.as_str(), release.build_id.as_str()),
        ("0.1.0", "test-1")
    );
    assert_eq!(release.files.len(), 6);
    let preview = parse(&with(|value| value["channel"] = json!("preview"))).unwrap();
    assert_eq!(preview.channel, UpdateChannel::Preview);
    // The checked-in fixture, signed by the release tooling, is itself a complete manifest.
    assert!(parse_bytes(include_bytes!("fixtures/release.json")).is_some());
}

#[test]
fn extensions_unknown_components_and_unknown_connections_are_ignored() {
    for value in [
        with(|value| value["extensions"] = json!({"later": {"nested": [1, {"deep": null}]}})),
        with(|value| value["components"][0]["extensions"] = json!({"later": true})),
        with(|value| {
            let components = value["components"].as_array_mut().unwrap();
            components.push(json!({"id": "gui", "anything": {"unread": true}}));
            components.push(json!({"id": "future-2", "files": ["not-listed"]}));
        }),
        with(|value| {
            value["components"][0]["connections"]["later-link"] =
                json!({"generations": {"lowest": 3, "highest": 9}});
        }),
        with(|value| value["components"] = json!([])),
    ] {
        assert!(parse(&value).is_some(), "{value}");
    }
}

#[test]
fn every_bound_admits_its_limit_and_refuses_one_more() {
    let longest_version = format!("1.2.3-{}", "a".repeat(58));
    assert_eq!(longest_version.len(), 64);
    // The root object is depth 1 and its extensions depth 2, so 30 nested arrays reach depth 32.
    let nested = |depth: usize| {
        let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
        serde_json::from_str::<Value>(&text).unwrap()
    };
    let pairs: [(Value, Value); 7] = [
        (
            with(|value| value["extensions"] = json!({"deep": nested(30)})),
            with(|value| value["extensions"] = json!({"deep": nested(31)})),
        ),
        (
            with(|value| value["version"] = json!(longest_version)),
            with(|value| value["version"] = json!(format!("{longest_version}a"))),
        ),
        (
            with(|value| value["buildId"] = json!("b".repeat(96))),
            with(|value| value["buildId"] = json!("b".repeat(97))),
        ),
        (
            with(|value| value["files"][0]["size"] = json!(FILE_LIMIT)),
            with(|value| value["files"][0]["size"] = json!(FILE_LIMIT + 1)),
        ),
        (
            with(|value| {
                for (index, size) in [FILE_LIMIT, FILE_LIMIT, FILE_LIMIT, FILE_LIMIT, 0, 0]
                    .into_iter()
                    .enumerate()
                {
                    value["files"][index]["size"] = json!(size);
                }
            }),
            with(|value| {
                for (index, size) in [FILE_LIMIT, FILE_LIMIT, FILE_LIMIT, FILE_LIMIT, 1, 0]
                    .into_iter()
                    .enumerate()
                {
                    value["files"][index]["size"] = json!(size);
                }
            }),
        ),
        (
            with(|value| {
                let files = value["files"].as_array_mut().unwrap();
                files.extend((files.len()..4096).map(|index| file(&format!("extra/{index}"))));
            }),
            with(|value| {
                let files = value["files"].as_array_mut().unwrap();
                files.extend((files.len()..4097).map(|index| file(&format!("extra/{index}"))));
            }),
        ),
        (
            with(|value| {
                value["components"][0]["connections"]["client-server"]["generations"] =
                    json!({"lowest": 1, "highest": 65_535});
            }),
            with(|value| {
                value["components"][0]["connections"]["client-server"]["generations"] =
                    json!({"lowest": 1, "highest": 65_536});
            }),
        ),
    ];
    for (limit, beyond) in pairs {
        assert!(parse(&limit).is_some());
        assert!(parse(&beyond).is_none());
    }
}

#[test]
fn manifests_outside_version_2_or_for_another_release_are_refused() {
    let root = |member: &str, replacement: Value| with(|value| value[member] = replacement);
    let removed = |member: &str| {
        with(|value| {
            value.as_object_mut().unwrap().remove(member);
        })
    };
    for value in [
        root("schemaVersion", json!(1)),
        root("schemaVersion", json!("2")),
        root("product", json!("another-product")),
        root("platform", json!("linux-x86_64")),
        root("platform", json!("Windows-x86_64")),
        root("channel", json!("nightly")),
        root("channel", json!("")),
        root("version", json!("0.1.0+build.1")),
        root("version", json!("01.0.0")),
        root("version", json!("0.1")),
        root("version", json!("0.1.0-")),
        root("version", json!("0.1.0-01")),
        root("version", json!("0.1.0-a..b")),
        root("buildId", json!("a b")),
        root("extensions", json!([])),
        root("unknown", json!(true)),
        removed("components"),
        removed("files"),
        removed("buildId"),
        // A version 1 manifest has neither components nor a reader any longer.
        with(|value| {
            value["schemaVersion"] = json!(1);
            value.as_object_mut().unwrap().remove("components");
        }),
    ] {
        assert!(parse(&value).is_none(), "{value}");
    }
}

#[test]
fn files_that_could_escape_alias_or_go_unverified_are_refused() {
    let added = |path: &str| with(|value| value["files"].as_array_mut().unwrap().push(file(path)));
    let changed =
        |member: &str, replacement: Value| with(|value| value["files"][0][member] = replacement);
    for value in [
        added("release.json"),
        added("RELEASE.JSON"),
        added("release.json.sig"),
        added("../evil"),
        added("C:/evil"),
        added("a:stream"),
        added("a/NUL.exe"),
        added("a/COM1.txt"),
        added("a."),
        added("orchbee.EXE"),
        added("agent-observation-adapters"),
        changed("size", json!(-1)),
        changed("size", json!(1.5)),
        changed("sha256", json!("a".repeat(63))),
        changed("sha256", json!("A".repeat(64))),
        changed("url", json!("http://example.test/file")),
        changed("url", json!("https://user:secret@example.test/file")),
        changed("url", json!("https://example.test/file#fragment")),
        changed("extra", json!(true)),
        with(|value| value["files"] = json!([])),
        with(|value| {
            value["files"]
                .as_array_mut()
                .unwrap()
                .retain(|f| f["path"] != "cohort.json")
        }),
    ] {
        assert!(parse(&value).is_none(), "{value}");
    }
}

#[test]
fn known_components_are_closed_and_bound_to_listed_files() {
    let component = |member: &str, replacement: Value| {
        with(|value| value["components"][0][member] = replacement)
    };
    let range = |lowest: Value, highest: Value| {
        with(|value| {
            value["components"][0]["connections"]["client-server"]["generations"] =
                json!({"lowest": lowest, "highest": highest});
        })
    };
    for value in [
        component("unknown", json!(true)),
        component("files", json!(["not-listed.exe"])),
        component("files", json!(["orchbee.exe", "orchbee.exe"])),
        component("files", json!([])),
        component("version", json!("1.0")),
        component("securityFloor", json!("1")),
        component("recommendedFloor", json!(null)),
        component("extensions", json!("text")),
        component("connections", json!([])),
        component(
            "connections",
            json!({"Client-Server": {"generations": {"lowest": 1, "highest": 1}}}),
        ),
        component(
            "connections",
            json!({"client-server": {"generations": {"lowest": 1, "highest": 1}, "extra": 1}}),
        ),
        component(
            "connections",
            json!({"client-server": {"generations": {"lowest": 1, "highest": 1, "extra": 1}}}),
        ),
        range(json!(2), json!(1)),
        range(json!(0), json!(1)),
        range(json!(1), json!("1")),
        with(|value| {
            let duplicate = value["components"][0].clone();
            value["components"].as_array_mut().unwrap().push(duplicate);
        }),
        with(|value| value["components"][0]["id"] = json!("Client")),
        with(|value| value["components"] = json!({})),
        with(|value| {
            value["components"] = json!(
                (0..65)
                    .map(|n| json!({"id": format!("later-{n}")}))
                    .collect::<Vec<_>>()
            );
        }),
    ] {
        assert!(parse(&value).is_none(), "{value}");
    }
}

#[test]
fn only_strict_json_reaches_the_member_rules() {
    let text = serde_json::to_string(&sample()).unwrap();
    for bytes in [
        text.replacen(
            r#""product":"orchbee""#,
            r#""product":"orchbee","product":"orchbee""#,
            1,
        )
        .into_bytes(),
        text.replacen(r#""id":"client""#, r#""id":"client","id":"client""#, 1)
            .into_bytes(),
        format!("{text} {{}}").into_bytes(),
        format!("[{text}]").into_bytes(),
        [text.as_bytes(), &[0xff]].concat(),
    ] {
        assert!(
            parse_bytes(&bytes).is_none(),
            "{}",
            String::from_utf8_lossy(&bytes)
        );
    }
}
