//! Feed admission, version 1: the shared fixture the release tooling's tests read too, the feed
//! signature domain against the manifest's, the document bound, and the reader rules of each
//! member. Every case is signed with the test key, so a refusal comes from the rule under test.
use super::super::{
    feed::{ARCHIVE_LIMIT, Feed, WindowOffer},
    install::COMPLETION_HELPER,
    signature::{FEED_LIMIT, Refusal, TEST_TABLE, verify, verify_feed},
    window_files::{Rules, WindowFiles},
};
use super::{sign, sign_feed};
use crate::invocation::{OutputFormat, Request, parse_selected_request};
use orchbee_local_runtime::{
    AUTOMATIC_UPDATE_HELPER, CLIENT_FAILURE_HELPER, UpdateChannel, WINDOW_INSTALLER_HELPER,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::ffi::OsString;

/// The release tooling writes this pair and its tests rebuild it byte for byte; it names the
/// manifest fixture beside it. `.signed` keeps the formatter from rewriting the signed bytes.
const FEED: &[u8] = include_bytes!("fixtures/feed.json.signed");
const FEED_SIGNATURE: &[u8] = include_bytes!("fixtures/feed.json.sig");
const MANIFEST: &[u8] = include_bytes!("fixtures/release.json");
const MANIFEST_SIGNATURE: &[u8] = include_bytes!("fixtures/release.json.sig");
const PLATFORM: &str = "windows-x86_64";

fn admit(feed: &[u8]) -> Option<Feed> {
    let trusted = verify_feed(&sign_feed(feed), feed, TEST_TABLE).ok()?;
    Feed::parse(trusted, PLATFORM)
}

#[test]
fn the_shared_fixture_verifies_and_neither_domain_admits_the_other() {
    let feed = Feed::parse(
        verify_feed(FEED_SIGNATURE, FEED, TEST_TABLE).unwrap(),
        PLATFORM,
    )
    .unwrap();
    assert_eq!(
        (feed.channel, feed.version.as_str(), feed.build_id.as_str()),
        (UpdateChannel::Stable, "0.1.0", "fixture.1")
    );
    assert_eq!(feed.manifest, crate::hex::encode(&Sha256::digest(MANIFEST)));
    assert_eq!(
        (feed.archive.url.as_str(), feed.archive.size),
        (
            "https://releases.example.test/orchbee/v0.1.0/orchbee-0.1.0-windows-x86_64.zip",
            27_000_000
        )
    );

    // A feed signature never verifies a manifest, and a manifest signature never verifies a feed.
    assert_eq!(
        verify(FEED_SIGNATURE, FEED, TEST_TABLE).err(),
        Some(Refusal::Invalid)
    );
    assert_eq!(
        verify_feed(MANIFEST_SIGNATURE, MANIFEST, TEST_TABLE).err(),
        Some(Refusal::Invalid)
    );
    assert_eq!(
        verify_feed(&sign(FEED), FEED, TEST_TABLE).err(),
        Some(Refusal::Invalid)
    );
    assert_eq!(
        verify_feed(&sign_feed(FEED), FEED, &[]).err(),
        Some(Refusal::NoTrustedKeys)
    );
}

#[test]
fn a_feed_is_read_only_within_its_bound() {
    let padded = |length: usize| {
        let base = json!({"extensions": {"pad": ""}}).to_string();
        let base = base.replace(
            r#""pad":"""#,
            &format!(r#""pad":"{}""#, "x".repeat(length - base.len())),
        );
        assert_eq!(base.len(), length);
        base.into_bytes()
    };
    // The verifier never parses, so any bytes within the bound verify.
    let largest = padded(FEED_LIMIT);
    assert!(verify_feed(&sign_feed(&largest), &largest, TEST_TABLE).is_ok());
    let oversized = padded(FEED_LIMIT + 1);
    assert_eq!(
        verify_feed(&sign_feed(&oversized), &oversized, TEST_TABLE).err(),
        Some(Refusal::Oversized)
    );
}

#[test]
fn feed_members_follow_the_version_1_reader_rules() {
    let base = json!({
        "schemaVersion": 1, "product": "orchbee", "channel": "stable", "platform": PLATFORM,
        "version": "0.2.0", "buildId": "build.7", "manifest": {"sha256": "a".repeat(64)},
        "archive": {"format": "zip", "url": "https://releases.example.test/stable/orchbee.zip",
            "size": 27_000_000, "sha256": "b".repeat(64)},
    });
    // What the case is, how it changes the admitted feed, and whether the result is admitted.
    type Case = (&'static str, fn(&mut Value), bool);
    let cases: [Case; 31] = [
        ("as written", |_| {}, true),
        ("preview", |v| v["channel"] = json!("preview"), true),
        (
            "pre-release",
            |v| v["version"] = json!("0.2.0-beta.1"),
            true,
        ),
        (
            "relative archive",
            |v| v["archive"]["url"] = json!("orchbee-0.2.0-windows-x86_64.zip"),
            true,
        ),
        (
            "largest archive",
            |v| v["archive"]["size"] = json!(ARCHIVE_LIMIT),
            true,
        ),
        (
            "smallest archive",
            |v| v["archive"]["size"] = json!(1),
            true,
        ),
        (
            "extensions",
            |v| v["extensions"] = json!({"later": {"any": [1, "two"]}}),
            true,
        ),
        ("unknown member", |v| v["comment"] = json!("x"), false),
        (
            "unknown archive member",
            |v| v["archive"]["name"] = json!("x"),
            false,
        ),
        (
            "unknown manifest member",
            |v| v["manifest"]["size"] = json!(1),
            false,
        ),
        (
            "extensions not an object",
            |v| v["extensions"] = json!([]),
            false,
        ),
        ("schema version 2", |v| v["schemaVersion"] = json!(2), false),
        ("another product", |v| v["product"] = json!("other"), false),
        ("unknown channel", |v| v["channel"] = json!("beta"), false),
        (
            "another platform",
            |v| v["platform"] = json!("windows-aarch64"),
            false,
        ),
        ("build metadata", |v| v["version"] = json!("0.2.0+7"), false),
        ("empty build id", |v| v["buildId"] = json!(""), false),
        (
            "another format",
            |v| v["archive"]["format"] = json!("tar.gz"),
            false,
        ),
        (
            "query",
            |v| v["archive"]["url"] = json!("https://releases.example.test/a.zip?x=1"),
            false,
        ),
        (
            "plain http",
            |v| v["archive"]["url"] = json!("http://releases.example.test/a.zip"),
            false,
        ),
        (
            "parent path",
            |v| v["archive"]["url"] = json!("../a.zip"),
            false,
        ),
        (
            "drive path",
            |v| v["archive"]["url"] = json!(r"C:\a.zip"),
            false,
        ),
        ("empty archive", |v| v["archive"]["size"] = json!(0), false),
        (
            "archive above the bound",
            |v| v["archive"]["size"] = json!(ARCHIVE_LIMIT + 1),
            false,
        ),
        (
            "fractional size",
            |v| v["archive"]["size"] = json!(1.5),
            false,
        ),
        (
            "upper-case digest",
            |v| v["archive"]["sha256"] = json!("B".repeat(64)),
            false,
        ),
        (
            "short digest",
            |v| v["archive"]["sha256"] = json!("b".repeat(63)),
            false,
        ),
        (
            "upper-case manifest",
            |v| v["manifest"]["sha256"] = json!("A".repeat(64)),
            false,
        ),
        ("null manifest", |v| v["manifest"] = Value::Null, false),
        ("null archive", |v| v["archive"] = Value::Null, false),
        ("not an object", |v| *v = json!([1]), false),
    ];
    for (name, change, admitted) in cases {
        let mut value = base.clone();
        change(&mut value);
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(admit(&bytes).is_some(), admitted, "{name}: {value}");
    }
    // The same members spelled with other white space are the same feed.
    let spelled = serde_json::to_vec_pretty(&base).unwrap();
    assert_eq!(admit(&spelled).unwrap().version, "0.2.0");
}

#[test]
fn forward_compatible_extensions_in_feed_and_window_file_lists_are_admitted() {
    let mut feed_value = json!({
        "schemaVersion": 1,
        "product": "orchbee",
        "channel": "stable",
        "platform": PLATFORM,
        "version": "0.3.0",
        "buildId": "build.9",
        "manifest": {"sha256": "a".repeat(64)},
        "archive": {
            "format": "zip",
            "url": "https://releases.example.test/stable/orchbee.zip",
            "size": 27_000_000,
            "sha256": "b".repeat(64),
        },
        "extensions": {
            "futureField": {"nested": [1, "two", {"enabled": true}]},
            "desktop": {
                "version": "0.3.0",
                "manifest": {"sha256": "c".repeat(64)},
                "archive": {
                    "format": "zip",
                    "url": "https://releases.example.test/stable/orchbee-window.zip",
                    "size": 45_000_000,
                    "sha256": "d".repeat(64),
                },
                "extensions": {
                    "futureField": {"minOs": "11.0", "notes": ["bridge-safe"]},
                },
            },
        },
    });
    let parsed = admit(&serde_json::to_vec(&feed_value).unwrap()).unwrap();
    let WindowOffer::Offered(window) = parsed.window else {
        panic!("expected extensions.desktop with nested extensions.futureField to be offered");
    };
    assert_eq!(window.version, "0.3.0");
    assert_eq!(window.list, "c".repeat(64));
    assert_eq!(window.archive.size, 45_000_000);

    // An unknown field directly inside `extensions.desktop` outside `extensions` invalidates only
    // the window offer and never breaks runtime feed admission.
    feed_value["extensions"]["desktop"]["unexpectedField"] = json!(true);
    let runtime_only = admit(&serde_json::to_vec(&feed_value).unwrap()).unwrap();
    assert!(matches!(runtime_only.window, WindowOffer::Invalid));

    let windows_list = json!({
        "schemaVersion": 1,
        "version": "0.3.0",
        "files": [{"path": "orchbee-desktop.exe", "size": 10, "sha256": "e".repeat(64)}],
        "extensions": {"futureField": {"channel": "stable", "flags": [1, 2]}},
    });
    assert!(
        WindowFiles::parse(&serde_json::to_vec(&windows_list).unwrap(), Rules::Windows).is_some()
    );

    let macos_list = json!({
        "schemaVersion": 1,
        "version": "0.3.0",
        "files": [{
            "path": "Orchbee.app/Contents/MacOS/Orchbee",
            "size": 10,
            "sha256": "f".repeat(64),
            "executable": true,
        }],
        "links": [],
        "extensions": {"futureField": {"notarized": true}},
    });
    assert!(WindowFiles::parse(&serde_json::to_vec(&macos_list).unwrap(), Rules::Macos).is_some());

    let mut invalid_extensions = windows_list;
    invalid_extensions["extensions"] = json!([]);
    assert!(
        WindowFiles::parse(
            &serde_json::to_vec(&invalid_extensions).unwrap(),
            Rules::Windows,
        )
        .is_none()
    );
}

#[test]
fn cross_build_helper_flags_keep_their_frozen_spelling() {
    const FROZEN_MESSAGE: &str = "cross-build helper CLI flags are permanently frozen contracts invoked across older and newer Orchbee binaries and installers; keep accepting every released spelling and introduce a new name instead of renaming";
    for (helper, current) in [
        (COMPLETION_HELPER, "--complete-install"),
        (AUTOMATIC_UPDATE_HELPER, "--automatic-update"),
        (CLIENT_FAILURE_HELPER, "--record-client-failure"),
        (WINDOW_INSTALLER_HELPER, "--installer-window"),
    ] {
        assert_eq!(
            (helper.current, helper.earlier),
            (current, format!("{current}-v1").as_str()),
            "{FROZEN_MESSAGE}"
        );
    }

    let cases: [(&[&str], Request, Option<OutputFormat>); 13] = [
        (
            &["--complete-install"],
            Request::CompleteInstall {
                no_path: false,
                allow_downgrade: false,
            },
            None,
        ),
        (
            &["--complete-install", "--no-path", "--allow-downgrade"],
            Request::CompleteInstall {
                no_path: true,
                allow_downgrade: true,
            },
            None,
        ),
        (
            &["--installer-uninstall"],
            Request::InstallerUninstall { delete_data: false },
            None,
        ),
        (
            &["--installer-uninstall", "--delete-data"],
            Request::InstallerUninstall { delete_data: true },
            None,
        ),
        (
            &["--automatic-update"],
            Request::AutomaticUpdate,
            Some(OutputFormat::Json),
        ),
        (
            &["--record-client-failure", "0.2.0"],
            Request::RecordClientFailure {
                version: "0.2.0".into(),
            },
            None,
        ),
        (
            &["--installer-window"],
            Request::InstallerWindow,
            Some(OutputFormat::Json),
        ),
        (&["--tend-window-store"], Request::TendWindowStore, None),
        (
            &["--create-window-folder"],
            Request::CreateWindowFolder,
            Some(OutputFormat::Json),
        ),
        (&["--machine-link"], Request::MachineLink, None),
        (&["--machine-hub"], Request::MachineHub, None),
        (&["--machine-hub-state"], Request::MachineHubState, None),
        (&["--machine-hub-hold"], Request::MachineHubHold, None),
    ];
    // Releases up to 0.1.0-beta.24 send each of these entries only with a `-v1` suffix, so the
    // earlier spelling selects the same request.
    for (argv, expected_request, expected_format) in cases {
        let Some((entry, rest)) = argv.split_first() else {
            continue;
        };
        for entry in [(*entry).to_owned(), format!("{entry}-v1")] {
            let argv: Vec<OsString> = std::iter::once(entry.as_str())
                .chain(rest.iter().copied())
                .map(OsString::from)
                .collect();
            let selected = parse_selected_request(argv.clone().into_iter())
                .unwrap_or_else(|_| panic!("{FROZEN_MESSAGE}: {argv:?}"));
            assert_eq!(
                (selected.request, selected.format),
                (expected_request.clone(), expected_format),
                "{FROZEN_MESSAGE}: {argv:?}"
            );
        }
    }
}
