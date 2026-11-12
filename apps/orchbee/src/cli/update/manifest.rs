//! Admission of signed release manifests, version 2; no download or installation effects. Only
//! bytes a compiled key verified, or an earlier verified installation stored, reach this parser.
use super::{signature::TrustedManifest, strict_json};
use crate::release_store::valid_release_path;
use orchbee_local_runtime::UpdateChannel;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) const FILE_LIMIT: u64 = 512 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const FILE_COUNT_LIMIT: usize = 4096;
const COMPONENT_LIMIT: usize = 64;
const CONNECTION_LIMIT: usize = 64;
/// Paths the installation stores beside the listed files, so no listed file may take them.
const RESERVED_PATHS: [&str; 2] = ["release.json", "release.json.sig"];
/// Components this updater reads; any other `id` is skipped unread.
const KNOWN_COMPONENTS: [&str; 5] = [
    "client",
    "server",
    "worker",
    "terminal-host",
    "agent-observation-capsule",
];

/// The release platform of this updater: its operating system and architecture, such as
/// `windows-x86_64`. A manifest for any other platform is refused.
pub(super) fn own_platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// A complete bounded release admitted before creating an installation directory.
pub(super) struct Release {
    pub encoded: Vec<u8>,
    pub channel: UpdateChannel,
    pub identity: String,
    pub version: String,
    pub build_id: String,
    pub files: Vec<Artifact>,
}
pub(super) struct Artifact {
    pub path: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}
impl Release {
    /// Admits a trusted version 2 manifest for `platform`. Unknown members are refused except
    /// inside `extensions` and unknown component entries, which are ignored.
    pub fn parse(manifest: TrustedManifest<'_>, platform: &str) -> Option<Self> {
        let bytes = manifest.bytes();
        let root = Value::Object(strict_json::parse_object(bytes)?);
        let object = strict_json::closed(
            &root,
            &[
                "schemaVersion",
                "product",
                "channel",
                "platform",
                "version",
                "buildId",
                "files",
                "components",
            ],
            &["extensions"],
        )?;
        strict_json::extensions(object)?;
        if object.get("schemaVersion")?.as_u64()? != 2
            || object.get("product")?.as_str()? != "orchbee"
        {
            return None;
        }
        let channel = match token(object.get("channel")?, 32, false)? {
            "stable" => UpdateChannel::Stable,
            "preview" => UpdateChannel::Preview,
            _ => return None,
        };
        if token(object.get("platform")?, 32, true)? != platform {
            return None;
        }
        let version = semantic_version(object.get("version")?)?;
        let build_id = label(object.get("buildId")?)?;
        let files = files(object.get("files")?)?;
        components(object.get("components")?, &files)?;
        Some(Self {
            encoded: bytes.to_vec(),
            channel,
            identity: format!("{:x}", Sha256::digest(bytes)),
            version,
            build_id,
            files,
        })
    }
}

fn files(value: &Value) -> Option<Vec<Artifact>> {
    let array = value.as_array()?;
    if array.is_empty() || array.len() > FILE_COUNT_LIMIT {
        return None;
    }
    let mut paths = BTreeSet::new();
    let mut files = Vec::with_capacity(array.len());
    let mut total = 0_u64;
    for value in array {
        let entry = strict_json::closed(value, &["path", "url", "sha256", "size"], &[])?;
        let path = entry.get("path")?.as_str()?;
        if !valid_release_path(path)
            || RESERVED_PATHS
                .iter()
                .any(|reserved| path.eq_ignore_ascii_case(reserved))
            || !paths.insert(path.to_ascii_lowercase())
        {
            return None;
        }
        let url = entry.get("url")?.as_str()?;
        if !valid_https(url) && !valid_release_path(url) {
            return None;
        }
        let sha256 = entry.get("sha256")?.as_str()?;
        if !lowercase_sha256(sha256) {
            return None;
        }
        let size = entry.get("size")?.as_u64()?;
        total = total.checked_add(size)?;
        if size > FILE_LIMIT || total > TOTAL_LIMIT {
            return None;
        }
        files.push(Artifact {
            path: path.into(),
            url: url.into(),
            sha256: sha256.into(),
            size,
        });
    }
    // A listed file must not also be a directory of another listed file.
    for path in &paths {
        let mut prefix = String::new();
        for part in path.split('/').take(path.split('/').count() - 1) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if paths.contains(&prefix) {
                return None;
            }
        }
    }
    // The installation's integrity manifest names every executable the cohort admits.
    files
        .iter()
        .any(|file| file.path == "cohort.json")
        .then_some(files)
}

/// Checks every known component strictly and skips unknown ones. A missing entry imposes nothing.
fn components(value: &Value, files: &[Artifact]) -> Option<()> {
    let array = value.as_array()?;
    if array.len() > COMPONENT_LIMIT {
        return None;
    }
    let mut ids = BTreeSet::new();
    for value in array {
        let id = token(value.as_object()?.get("id")?, 64, false)?;
        if !ids.insert(id) {
            return None;
        }
        if KNOWN_COMPONENTS.contains(&id) {
            component(value, files)?;
        }
    }
    Some(())
}

fn component(value: &Value, files: &[Artifact]) -> Option<()> {
    let object = strict_json::closed(
        value,
        &["id", "version", "files", "connections"],
        &["securityFloor", "recommendedFloor", "extensions"],
    )?;
    strict_json::extensions(object)?;
    semantic_version(object.get("version")?)?;
    for floor in ["securityFloor", "recommendedFloor"] {
        if let Some(value) = object.get(floor) {
            semantic_version(value)?;
        }
    }
    let listed = object.get("files")?.as_array()?;
    if listed.is_empty() || listed.len() > FILE_COUNT_LIMIT {
        return None;
    }
    let mut seen = BTreeSet::new();
    for path in listed {
        let path = path.as_str()?;
        if !files.iter().any(|file| file.path == path) || !seen.insert(path) {
            return None;
        }
    }
    connections(object.get("connections")?.as_object()?)
}

/// Each connection kind with its generation range. Kinds this build does not know are validated
/// and ignored, as unknown components are, so a later connection never makes older updaters refuse.
fn connections(object: &Map<String, Value>) -> Option<()> {
    if object.len() > CONNECTION_LIMIT {
        return None;
    }
    for (kind, value) in object {
        name(kind, 64, false)?;
        let entry = strict_json::closed(value, &["generations"], &[])?;
        let range = strict_json::closed(entry.get("generations")?, &["lowest", "highest"], &[])?;
        let lowest = generation(range.get("lowest")?)?;
        let highest = generation(range.get("highest")?)?;
        if lowest > highest {
            return None;
        }
    }
    Some(())
}

fn generation(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .filter(|generation| (1..=65_535).contains(generation))
}

/// A string of 1 to `limit` bytes of `a`–`z`, `0`–`9` and `-`, plus `_` when `underscore`.
fn token(value: &Value, limit: usize, underscore: bool) -> Option<&str> {
    let text = value.as_str()?;
    name(text, limit, underscore)?;
    Some(text)
}

fn name(text: &str, limit: usize, underscore: bool) -> Option<()> {
    (!text.is_empty()
        && text.len() <= limit
        && text.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || (underscore && b == b'_')
        }))
    .then_some(())
}

/// Whether `text` is a version a manifest admits, so it may name a store version directory.
pub(super) fn valid_version(text: &str) -> bool {
    semantic_version(&Value::String(text.to_owned())).is_some()
}

/// A SemVer 2.0.0 version of at most 64 bytes, without build metadata.
fn semantic_version(value: &Value) -> Option<String> {
    let text = value.as_str()?;
    if text.is_empty() || text.len() > 64 {
        return None;
    }
    let (core, prerelease) = match text.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (text, None),
    };
    let core: Vec<&str> = core.split('.').collect();
    if core.len() != 3 || !core.iter().all(|part| numeric_identifier(part)) {
        return None;
    }
    if let Some(prerelease) = prerelease {
        let admitted = prerelease.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && (!part.bytes().all(|b| b.is_ascii_digit()) || numeric_identifier(part))
        });
        if !admitted {
            return None;
        }
    }
    Some(text.to_owned())
}

/// Digits without a leading zero, except `0` itself.
fn numeric_identifier(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

fn label(value: &Value) -> Option<String> {
    let text = value.as_str()?;
    (!text.is_empty()
        && text.len() <= 96
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_+".contains(&b)))
    .then(|| text.to_owned())
}

pub(super) fn lowercase_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Curl receives a literal HTTPS URL, with no userinfo, controls, fragments or glob expansion.
pub(super) fn valid_https(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    if value.len() > 4096
        || !value
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != 0x60 && !b"\\\"'<>#".contains(&b))
    {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or("");
    rest.contains('/')
        && !authority.is_empty()
        && authority
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b))
}
