//! Admission of the signed update feed, version 1: the small document that names the newest release
//! of one channel and platform and the one archive that carries it, by address, exact size and
//! SHA-256. No download or installation effects. Only bytes a compiled key verified under the feed
//! domain reach this parser.
//!
//! Frozen from the first version that ships it: the members, their bounds, the `extensions` rule,
//! the signed bytes (see [`super::signature`]) and the archive's shape, a zip whose one top-level
//! folder is a signed release directory. Additions go into `extensions`, which this reader ignores.
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
        let archive = closed_object(
            object.get("archive")?,
            &["format", "url", "size", "sha256"],
            &[],
        )?;
        if Some(archive.get("format")?.as_str()?) != archive_format(platform) {
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
        Some(Self {
            channel,
            version,
            build_id,
            manifest,
            archive: Archive {
                url: url.to_owned(),
                size,
                sha256: sha256.to_owned(),
            },
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

/// The archive format a feed for `platform` names: `zip` for Windows. Other platforms have no
/// format yet, so their feeds are refused.
fn archive_format(platform: &str) -> Option<&'static str> {
    platform.starts_with("windows-").then_some("zip")
}
