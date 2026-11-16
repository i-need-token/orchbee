//! Admission of the signed update feed, version 1: the small document that names the newest release
//! of one channel and platform and the one archive that carries it, by address, exact size and
//! SHA-256. No download or installation effects. Only bytes a compiled key verified under the feed
//! domain reach this parser.
//!
//! Frozen from the first version that ships it: the members, their bounds, the `extensions` rule,
//! the signed bytes (see [`super::signature`]) and the archive's shape, a zip whose one top-level
//! folder is a signed release directory. Additions go into `extensions`, which this reader ignores
//! except for `extensions.desktop`, the window archive of the window self-update release: a closed
//! object of `version`, `manifest: {sha256}` (the SHA-256 of the archive's `desktop-files.json`),
//! `archive: {format: "zip", url, size, sha256}` and an optional `extensions`, each bounded as the
//! feed's own member of that name. A malformed `extensions.desktop` never refuses the feed: it only
//! makes the window part fail.
use super::{
    manifest::{Release, channel, label, semantic_version, token, valid_https},
    signature::TrustedFeed,
};
use crate::release_store::valid_release_path;
use orchbee_local_runtime::UpdateChannel;
use orchbee_release_records::{check_extensions, closed_object, lowercase_sha256, parse_object};
use serde_json::Value;

/// The largest archive a feed may name. This bound never shrinks.
pub(super) const ARCHIVE_LIMIT: u64 = 1024 * 1024 * 1024;

/// An admitted feed: the release it offers and the archive that carries it.
pub(super) struct Feed {
    pub channel: UpdateChannel,
    pub version: String,
    pub build_id: String,
    /// The lowercase SHA-256 of the `release.json` inside the archive: the identity the store's
    /// pointer records for this version.
    pub manifest: String,
    pub archive: Archive,
    /// What `extensions.desktop` offers.
    pub window: WindowOffer,
}

/// The window version a feed offers in `extensions.desktop`.
pub(super) enum WindowOffer {
    /// The feed names no window archive.
    Absent,
    /// `extensions.desktop` breaks a rule of its own; the runtime part is unaffected.
    Invalid,
    Offered(Window),
}

/// The window archive a feed names.
pub(super) struct Window {
    pub version: String,
    /// The lowercase SHA-256 of the archive's `desktop-files.json`: the identity the window pointer
    /// records for this version.
    pub list: String,
    pub archive: Archive,
}

/// The one archive a feed names. Its format is the platform's only archive format.
pub(super) struct Archive {
    /// An HTTPS address without a query, or a portable relative path resolved beside the feed.
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

impl Feed {
    /// Admits a trusted version 1 feed for `platform`. Unknown members are refused except inside
    /// `extensions`, which is checked and otherwise ignored.
    pub fn parse(feed: TrustedFeed<'_>, platform: &str) -> Option<Self> {
        let root = Value::Object(parse_object(feed.bytes())?);
        let object = closed_object(
            &root,
            &[
                "schemaVersion",
                "product",
                "channel",
                "platform",
                "version",
                "buildId",
                "manifest",
                "archive",
            ],
            &["extensions"],
        )?;
        check_extensions(object)?;
        if object.get("schemaVersion")?.as_u64()? != 1
            || object.get("product")?.as_str()? != "orchbee"
        {
            return None;
        }
        let channel = channel(object.get("channel")?)?;
        if token(object.get("platform")?, 32, true)? != platform {
            return None;
        }
        let version = semantic_version(object.get("version")?)?;
        let build_id = label(object.get("buildId")?)?;
        let manifest = closed_object(object.get("manifest")?, &["sha256"], &[])?
            .get("sha256")?
            .as_str()
            .filter(|digest| lowercase_sha256(digest))?
            .to_owned();
        let archive = archive(object.get("archive")?, archive_format(platform)?)?;
        let window = match object
            .get("extensions")
            .and_then(|extensions| extensions.get("desktop"))
        {
            None => WindowOffer::Absent,
            Some(value) => window(value).map_or(WindowOffer::Invalid, WindowOffer::Offered),
        };
        Some(Self {
            channel,
            version,
            build_id,
            manifest,
            archive,
            window,
        })
    }

    /// Whether `release`, admitted from the archive or the store, is the release this feed names.
    pub fn names(&self, release: &Release) -> bool {
        release.version == self.version
            && release.channel == self.channel
            && release.build_id == self.build_id
            && release.identity == self.manifest
    }
}

/// The closed archive member `value` in `format`: an HTTPS address without a query or a portable
/// relative path, a size from 1 byte to [`ARCHIVE_LIMIT`], and a lowercase SHA-256.
fn archive(value: &Value, format: &str) -> Option<Archive> {
    let archive = closed_object(value, &["format", "url", "size", "sha256"], &[])?;
    if archive.get("format")?.as_str()? != format {
        return None;
    }
    let url = archive.get("url")?.as_str()?;
    if !(valid_https(url) && !url.contains('?')) && !valid_release_path(url) {
        return None;
    }
    let size = archive
        .get("size")?
        .as_u64()
        .filter(|size| (1..=ARCHIVE_LIMIT).contains(size))?;
    let sha256 = archive
        .get("sha256")?
        .as_str()
        .filter(|digest| lowercase_sha256(digest))?;
    Some(Archive {
        url: url.to_owned(),
        size,
        sha256: sha256.to_owned(),
    })
}

/// The window archive `extensions.desktop` names; its archive is a zip on every platform.
fn window(value: &Value) -> Option<Window> {
    let object = closed_object(value, &["version", "manifest", "archive"], &["extensions"])?;
    check_extensions(object)?;
    let list = closed_object(object.get("manifest")?, &["sha256"], &[])?
        .get("sha256")?
        .as_str()
        .filter(|digest| lowercase_sha256(digest))?
        .to_owned();
    Some(Window {
        version: semantic_version(object.get("version")?)?,
        list,
        archive: archive(object.get("archive")?, "zip")?,
    })
}

/// The archive format a feed for `platform` names, as the release tool writes it
/// (`scripts/release-feed.mjs`): `zip` for Windows and `tar.gz` for macOS and Linux. Other
/// platforms have no format, so their feeds are refused.
fn archive_format(platform: &str) -> Option<&'static str> {
    if platform.starts_with("windows-") {
        Some("zip")
    } else if platform.starts_with("macos-") || platform.starts_with("linux-") {
        Some("tar.gz")
    } else {
        None
    }
}
