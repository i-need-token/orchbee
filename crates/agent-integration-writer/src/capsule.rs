//! The adapter payload capsule. Its payload digest is its identity and is contractual with the
//! verifier in `scripts/stage-local-runtime.mjs`:
//! the canonical sha256 is computed over `JSON.stringify` of the sorted
//! payload entry array with the exact member order `{kind,path,sha256,size}`
//! for files and `{kind,path}` for directories, so the field order and the
//! compact separators below are load-bearing. An installed Capsule's folder is
//! named by its version, `bundle/package.json`'s `version`, so a person reads
//! which Capsule an Agent entry runs, and its owner record names the environment
//! that installed it.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const MAX_CAPSULE_ENTRIES: usize = 128;
const MAX_CAPSULE_DEPTH: usize = 8;
const MAX_CAPSULE_FILE_BYTES: u64 = 512 * 1024;
const MAX_CAPSULE_TOTAL_BYTES: u64 = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024;
const MAX_PACKAGE_BYTES: u64 = 64 * 1024;
/// The last `-<n>` suffix a version folder may take before the name counts as exhausted.
const MAX_VERSION_SUFFIX: usize = 64;
const PACKAGE_MANIFEST: &str = "bundle/package.json";
/// The record naming the environment that installed a Capsule, beside the transport that reads it
/// at hook time. The writer adds it to every Capsule it publishes and the payload digest leaves it
/// out, so the identity stays the source's while every copy a provider makes of `bundle/` keeps
/// it.
pub(crate) const OWNER_RECORD: &str = "bundle/scripts/environment.json";
/// The owner record's name inside a copied scripts folder.
pub(crate) const OWNER_RECORD_NAME: &str = "environment.json";
const MAX_OWNER_RECORD_BYTES: u64 = 256;
const RELEASE_ENVIRONMENT: &str = "orchbee";
const REQUIRED_RUNTIME_FILES: [&str; 4] = [
    "bundle/package.json",
    "bundle/scripts/forward-hook.mjs",
    "bundle/scripts/hook-sanitizers.mjs",
    "bundle/scripts/observation-transport.mjs",
];

/// Whether the strict current manifest and its actual payload agree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CapsuleValidity {
    Ready,
    /// Any other invalidity: malformed manifest, digest mismatch, bounds,
    /// symlinks, missing required files, unreadable tree.
    Invalid,
}

/// `capsule.json` contains exactly `{contractVersion:1, payloadSha256:<64hex>}`.
fn read_manifest(root: &Path) -> Option<Map<String, Value>> {
    let manifest_path = root.join("capsule.json");
    let metadata = fs::symlink_metadata(&manifest_path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    #[cfg(unix)]
    let bytes = fs::read(&manifest_path).ok()?;
    #[cfg(windows)]
    let bytes = super::files::read_bounded(&manifest_path, MAX_MANIFEST_BYTES).ok()?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return None;
    }
    match serde_json::from_slice::<Value>(&bytes).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

fn valid_sha256(value: &Value) -> bool {
    matches!(value, Value::String(text) if text.len() == 64
        && text.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
}

#[cfg(windows)]
pub(super) fn verify_distribution(root: &Path, expected: &str) -> bool {
    let Some(manifest) = read_manifest(root) else {
        return false;
    };
    manifest.len() == 2
        && manifest.get("contractVersion").and_then(Value::as_f64) == Some(1.0)
        && manifest.get("payloadSha256").and_then(Value::as_str) == Some(expected)
        && REQUIRED_RUNTIME_FILES.iter().all(|relative| {
            fs::symlink_metadata(root.join(relative)).is_ok_and(|metadata| metadata.is_file())
        })
        && payload_digest(root).as_deref() == Some(expected)
}

/// The bounded canonical digest of a payload tree: lexicographic path order,
/// `/` separators, `capsule.json` excluded, symlinks or other non-file
/// non-directory entries invalid, every bound enforced.
pub(crate) fn payload_digest(root: &Path) -> Option<String> {
    if !fs::symlink_metadata(root).ok()?.is_dir() {
        return None;
    }
    let mut entry_count = 0usize;
    let mut total_bytes = 0u64;
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<(usize, PathBuf)> = vec![(0, root.to_path_buf())];
    while let Some((depth, directory)) = pending.pop() {
        #[cfg(windows)]
        let _pins = super::files::pin_tree(&directory, false).ok()?;
        if depth > MAX_CAPSULE_DEPTH {
            return None;
        }
        for entry in fs::read_dir(&directory).ok()? {
            entry_count += 1;
            if entry_count > MAX_CAPSULE_ENTRIES {
                return None;
            }
            let entry = entry.ok()?;
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(root)
                .ok()?
                .to_str()?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                return None;
            }
            if file_type.is_dir() {
                entries.push((
                    relative.clone(),
                    json!({ "kind": "directory", "path": relative }).to_string(),
                ));
                pending.push((depth + 1, entry_path));
                continue;
            }
            if !file_type.is_file() {
                return None;
            }
            if relative == "capsule.json" || relative == OWNER_RECORD {
                continue;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() || metadata.len() > MAX_CAPSULE_FILE_BYTES {
                return None;
            }
            #[cfg(unix)]
            let bytes = fs::read(&entry_path).ok()?;
            #[cfg(windows)]
            let bytes = super::files::read_bounded(&entry_path, MAX_CAPSULE_FILE_BYTES).ok()?;
            if bytes.len() as u64 > MAX_CAPSULE_FILE_BYTES {
                return None;
            }
            total_bytes += bytes.len() as u64;
            if total_bytes > MAX_CAPSULE_TOTAL_BYTES {
                return None;
            }
            let digest = format!("{:x}", Sha256::digest(&bytes));
            let size = bytes.len();
            // serde_json's map is BTree-ordered here — `kind,path,sha256,size`
            // is already alphabetical, so this serializes identically to the
            // canonical JSON.stringify member order.
            entries.push((
                relative.clone(),
                json!({ "kind": "file", "path": relative, "sha256": digest, "size": size })
                    .to_string(),
            ));
        }
    }
    // The verifier sorts entries by path — directories and
    // files interleave (`bundle/package.json` sits between the `bundle/*`
    // directories), so the sort key is the path, never the serialized entry.
    entries.sort_by(|left, right| left.0.encode_utf16().cmp(right.0.encode_utf16()));
    let joined = format!(
        "[{}]",
        entries
            .iter()
            .map(|(_, serialized)| serialized.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    Some(format!("{:x}", Sha256::digest(joined.as_bytes())))
}

/// The manifest's declared payload digest (already validated shape). Shared
/// with the cohort-binding check: a sidecar whose manifest declares a digest
/// different from the cohort manifest's is a binding failure.
pub(crate) fn manifest_declared_digest(root: &Path) -> Option<String> {
    read_manifest(root)?
        .get("payloadSha256")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The build's bundled payload identity, also available when no cohort is staged.
pub fn bundled_payload_sha256() -> String {
    let manifest: Value =
        serde_json::from_str(include_str!("../agent-observation-adapters/capsule.json"))
            .expect("the checked-in capsule manifest is valid JSON");
    manifest["payloadSha256"]
        .as_str()
        .filter(|digest| valid_sha256(&Value::String((*digest).to_owned())))
        .expect("the checked-in capsule manifest declares a canonical digest")
        .to_owned()
}

/// Whether `version` can name a Capsule folder on every file system: 1–64 ASCII letters, digits,
/// `.`, `_`, and `-`, starting with a digit, so it is never a Windows device name, and not ending
/// with `.`.
fn folder_version(version: &str) -> bool {
    (1..=64).contains(&version.len())
        && version.starts_with(|first: char| first.is_ascii_digit())
        && !version.ends_with('.')
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn package_version(bytes: &[u8]) -> Option<String> {
    let manifest: Value = serde_json::from_slice(bytes).ok()?;
    manifest
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| folder_version(version))
        .map(str::to_owned)
}

/// The version a Capsule source declares in `bundle/package.json`, when it can name a folder.
pub(crate) fn declared_version(root: &Path) -> Option<String> {
    let path = root.join(PACKAGE_MANIFEST);
    let metadata = fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PACKAGE_BYTES {
        return None;
    }
    #[cfg(unix)]
    let bytes = fs::read(&path).ok()?;
    #[cfg(windows)]
    let bytes = super::files::read_bounded(&path, MAX_PACKAGE_BYTES).ok()?;
    package_version(&bytes)
}

/// The build's bundled Capsule version, also available when no cohort is staged.
pub(crate) fn bundled_version() -> String {
    package_version(include_bytes!(
        "../agent-observation-adapters/bundle/package.json"
    ))
    .expect("the checked-in capsule package declares a folder version")
}

/// The folder in `capsules` that holds the payload `digest`: the first of `<version>`,
/// `<version>-2`, `<version>-3`, ... whose manifest declares it, or else the first free name. A
/// name already taken by other content keeps that content, as the data root's naming rule
/// requires. Reading never creates anything.
pub(crate) fn installation_root(capsules: &Path, version: &str, digest: &str) -> PathBuf {
    for index in 1..=MAX_VERSION_SUFFIX {
        let candidate = if index == 1 {
            capsules.join(version)
        } else {
            capsules.join(format!("{version}-{index}"))
        };
        match fs::symlink_metadata(&candidate) {
            Ok(_) if manifest_declared_digest(&candidate).as_deref() != Some(digest) => {}
            // The payload's own folder, or a free name, or one whose state a later validation
            // reports.
            Ok(_) | Err(_) => return candidate,
        }
    }
    capsules.join(version)
}

/// This environment's owner record: `{"environment":"<stem>","version":1}` and LF.
pub(crate) fn owner_record() -> Vec<u8> {
    let mut bytes = json!({
        "environment": orchbee_local_runtime::environment_stem(),
        "version": 1,
    })
    .to_string()
    .into_bytes();
    bytes.push(b'\n');
    bytes
}

/// The environment owning a scripts folder: its owner record's environment, or release when the
/// folder has none, like every Capsule installed before each environment kept its own. `None` when
/// a record exists but cannot be read.
pub(crate) fn recorded_owner(scripts: &Path) -> Option<String> {
    let path = scripts.join(OWNER_RECORD_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(RELEASE_ENVIRONMENT.to_owned());
        }
        Err(_) => return None,
    };
    if !metadata.is_file() || metadata.len() > MAX_OWNER_RECORD_BYTES {
        return None;
    }
    #[cfg(unix)]
    let bytes = fs::read(&path).ok()?;
    #[cfg(windows)]
    let bytes = super::files::read_bounded(&path, MAX_OWNER_RECORD_BYTES).ok()?;
    let Value::Object(record) = serde_json::from_slice::<Value>(&bytes).ok()? else {
        return None;
    };
    if record.len() != 2 || record.get("version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    record
        .get("environment")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Whether this environment owns the scripts folder `scripts`.
pub(crate) fn owned_here(scripts: &Path) -> bool {
    recorded_owner(scripts).as_deref() == Some(orchbee_local_runtime::environment_stem())
}

/// Validate one capsule root: strict manifest keys and current contract, the four required runtime files, and the canonical digest
/// matching the declared one.
pub(crate) fn validate(root: &Path) -> CapsuleValidity {
    let Some(manifest) = read_manifest(root) else {
        return CapsuleValidity::Invalid;
    };
    if manifest.len() != 2
        || manifest.get("contractVersion").and_then(Value::as_f64) != Some(1.0)
        || !valid_sha256(manifest.get("payloadSha256").unwrap_or(&Value::Null))
    {
        return CapsuleValidity::Invalid;
    }
    for relative in REQUIRED_RUNTIME_FILES {
        let metadata = match fs::symlink_metadata(root.join(relative)) {
            Ok(metadata) => metadata,
            Err(_) => return CapsuleValidity::Invalid,
        };
        if !metadata.is_file() {
            return CapsuleValidity::Invalid;
        }
    }
    let declared_digest = manifest
        .get("payloadSha256")
        .and_then(Value::as_str)
        .unwrap_or("");
    match payload_digest(root) {
        Some(computed) if computed == declared_digest => CapsuleValidity::Ready,
        Some(_) | None => CapsuleValidity::Invalid,
    }
}

/// Whether the installed capsule is usable for management: it validates AND,
/// when a distinct source is present, the source's digest still matches (a
/// moved or replaced bundle downgrades the entry). An
/// ENOENT source is tolerable; any other read failure is not.
pub(crate) fn is_ready(root: &Path, source: Option<&Path>) -> bool {
    if validate(root) != CapsuleValidity::Ready {
        return false;
    }
    let Some(source) = source else {
        return true;
    };
    if source == root {
        return true;
    }
    match fs::symlink_metadata(source) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
        Ok(_) => {
            validate(source) == CapsuleValidity::Ready
                && manifest_declared_digest(source) == manifest_declared_digest(root)
        }
    }
}
