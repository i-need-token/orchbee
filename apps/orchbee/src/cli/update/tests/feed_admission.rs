//! Feed admission, version 1: the shared fixture the release tooling's tests read too, the feed
//! signature domain against the manifest's, the document bound, and the reader rules of each
//! member. Every case is signed with the test key, so a refusal comes from the rule under test.
use super::super::{
    feed::{ARCHIVE_LIMIT, Feed},
    signature::{FEED_LIMIT, Refusal, TEST_TABLE, verify, verify_feed},
};
use super::{sign, sign_feed};
use orchbee_local_runtime::UpdateChannel;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

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
    assert_eq!(feed.manifest, format!("{:x}", Sha256::digest(MANIFEST)));
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
