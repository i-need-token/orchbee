//! The content-addressed adapter payload capsule — a Rust port of
//! `apps/desktop/src/main/agent-integration-management/
//! FileAgentObservationAdapterCapsule.ts`. Digest parity is contractual:
//! the canonical sha256 is computed over `JSON.stringify` of the sorted
//! payload entry array with the exact member order `{kind,path,sha256,size}`
//! for files and `{kind,path}` for directories, so the field order and the
//! compact separators below are load-bearing.

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
            if relative == "capsule.json" {
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
    // Canonical `compareCapsulePayloadEntries` sorts by PATH — directories and
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
pub(crate) fn bundled_payload_sha256() -> String {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../desktop/resources/agent-observation-adapters/capsule.json"
    ))
    .expect("the checked-in capsule manifest is valid JSON");
    manifest["payloadSha256"]
        .as_str()
        .filter(|digest| valid_sha256(&Value::String((*digest).to_owned())))
        .expect("the checked-in capsule manifest declares a canonical digest")
        .to_owned()
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
/// when a distinct source is present, the source's digest still matches (the
/// canonical `isReady` — a moved/replaced bundle downgrades the entry). An
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
