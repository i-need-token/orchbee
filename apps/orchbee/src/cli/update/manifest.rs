//! Admission of explicit Windows release manifests; no download or installation effects.
use crate::release_store::valid_release_path;
use orchbee_local_runtime::UpdateChannel;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) const MANIFEST_LIMIT: usize = 64 * 1024;
pub(super) const FILE_LIMIT: u64 = 512 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

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
    pub fn parse(bytes: &[u8], channel: UpdateChannel) -> Option<Self> {
        if bytes.len() > MANIFEST_LIMIT {
            return None;
        }
        let value: Value = serde_json::from_slice(bytes).ok()?;
        object(
            &value,
            &[
                "schemaVersion",
                "product",
                "platform",
                "channel",
                "version",
                "buildId",
                "files",
            ],
        )?;
        if value["schemaVersion"] != 1
            || value["product"] != "orchbee"
            || value["platform"] != "windows-x86_64"
            || value["channel"] != channel.as_str()
        {
            return None;
        }
        let version = label(&value["version"])?;
        let build_id = label(&value["buildId"])?;
        let array = value["files"].as_array()?;
        if array.is_empty() || array.len() > 4096 {
            return None;
        }
        let mut paths = BTreeSet::new();
        let mut files = Vec::new();
        let mut total = 0_u64;
        for value in array {
            object(value, &["path", "url", "sha256", "size"])?;
            let path = value["path"].as_str()?;
            if !valid_release_path(path)
                || path.eq_ignore_ascii_case("release.json")
                || !paths.insert(path.to_ascii_lowercase())
            {
                return None;
            }
            let url = value["url"].as_str()?;
            if !valid_https(url) && !valid_release_path(url) {
                return None;
            }
            let sha256 = value["sha256"].as_str()?;
            if sha256.len() != 64
                || !sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            let size = value["size"].as_u64()?;
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
        for required in [
            "orchbee.exe",
            "orchbee-server.exe",
            "orchbee-worker.exe",
            "orchbee-terminal-host.exe",
            "cohort.json",
            "agent-observation-adapters/capsule.json",
            "conpty/conpty.dll",
            "conpty/openconsole.exe",
            "conpty/microsoft.conpty-license.txt",
        ] {
            if !paths.contains(required) {
                return None;
            }
        }
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
fn object(value: &Value, names: &[&str]) -> Option<()> {
    let map = value.as_object()?;
    (map.len() == names.len() && names.iter().all(|name| map.contains_key(*name))).then_some(())
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

#[cfg(test)]
mod tests;
