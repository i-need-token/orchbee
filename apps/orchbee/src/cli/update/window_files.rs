//! The window file list `desktop-files.json` (AU § Window archive and file list, § macOS window
//! versions): `{"schemaVersion": 1, "version", "files": [{"path", "size", "sha256"}],
//! "extensions"?}`, at most 1 MiB. It names every regular file of a window version once, by a
//! `/`-separated path, under the release manifest's file rules: at most 4096 entries, 512 MiB each
//! and 2 GiB in all, unique ignoring case. The feed's `extensions.desktop.manifest` and the window
//! pointer bind it by its SHA-256, so its digest covers every file of the window.
//!
//! A Windows list sits at the root of the window directory and names its other files by release
//! paths. A macOS list sits beside the signed `<name>.app` and names only the bundle: every path
//! starts with `<name>.app/`, whose parts also admit spaces and parentheses; a file its owner may
//! run carries `"executable": true`; and the bundle's symbolic links form the optional
//! `links: [{"path", "target"}]`, each target relative, inside the bundle and naming a listed file,
//! a listed link or a folder holding listed entries. Links share the entry bound and the uniqueness
//! with files. `scripts/desktop-files.mjs` writes the list and is the tooling twin of this reader;
//! the bounds never shrink. No file effects here.
use super::manifest::{FILE_COUNT_LIMIT, FILE_LIMIT, TOTAL_LIMIT, semantic_version};
use crate::release_store::PathGrammar;
use orchbee_release_records::{check_extensions, closed_object, lowercase_sha256, parse_object};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// The list's name in a window version folder.
pub(super) const NAME: &str = "desktop-files.json";
/// The most bytes the list may hold.
pub(super) const LIMIT: usize = 1024 * 1024;
/// The most links a path may pass through on its way to a listed entry, as the system resolves it.
const HOPS: usize = 40;

/// Whose list rules apply: Windows release paths, or a macOS bundle with modes and links.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Rules {
    Windows,
    Macos,
}

impl Rules {
    /// The rules of the platform this build runs on: macOS's there, and Windows' elsewhere, since
    /// only Windows and macOS have window versions.
    pub(super) const fn own() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Windows
        }
    }

    /// The grammar of a listed path.
    pub(super) const fn grammar(self) -> PathGrammar {
        match self {
            Self::Windows => PathGrammar::Release,
            Self::Macos => PathGrammar::Bundle,
        }
    }
}

/// An admitted window file list.
pub(super) struct WindowFiles {
    /// The exact bytes, which a stored window version keeps unchanged.
    pub encoded: Vec<u8>,
    /// The lowercase SHA-256 of [`Self::encoded`].
    pub identity: String,
    pub version: String,
    pub files: Vec<WindowFile>,
    /// macOS only; empty in a Windows list.
    pub links: Vec<WindowLink>,
}

/// One listed regular file of a window version.
pub(super) struct WindowFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    /// Whether its owner may run it: a macOS list marks it, and a Windows list never does.
    pub executable: bool,
}

/// One listed symbolic link of a macOS window version.
pub(super) struct WindowLink {
    pub path: String,
    pub target: String,
}

impl WindowFiles {
    /// Admits `bytes` as a version 1 list under `rules`. Unknown members are refused except inside
    /// `extensions`, which is checked and otherwise ignored.
    pub(super) fn parse(bytes: &[u8], rules: Rules) -> Option<Self> {
        if bytes.len() > LIMIT {
            return None;
        }
        let macos = rules == Rules::Macos;
        let root = Value::Object(parse_object(bytes)?);
        let optional: &[&str] = if macos {
            &["extensions", "links"]
        } else {
            &["extensions"]
        };
        let object = closed_object(&root, &["schemaVersion", "version", "files"], optional)?;
        check_extensions(object)?;
        if object.get("schemaVersion")?.as_u64()? != 1 {
            return None;
        }
        let version = semantic_version(object.get("version")?)?;
        let listed = object.get("files")?.as_array()?;
        let linked = match object.get("links") {
            Some(links) => links.as_array()?.as_slice(),
            None => &[],
        };
        if listed.len() + linked.len() > FILE_COUNT_LIMIT {
            return None;
        }
        // On macOS every entry lies inside the one bundle the first file names.
        let bundle = listed
            .first()
            .and_then(|file| file.get("path")?.as_str())
            .and_then(|path| path.split('/').next())
            .unwrap_or_default()
            .to_owned();
        let admitted = |path: &str| {
            rules.grammar().admits(path)
                && if macos {
                    bundle.ends_with(".app") && path.starts_with(&format!("{bundle}/"))
                } else {
                    !path.eq_ignore_ascii_case(NAME)
                }
        };
        let mut seen = BTreeSet::new();
        let mut total = 0_u64;
        let mut files = Vec::with_capacity(listed.len());
        for value in listed {
            let entry = closed_object(
                value,
                &["path", "size", "sha256"],
                if macos { &["executable"] } else { &[] },
            )?;
            let path = entry.get("path")?.as_str()?;
            if !admitted(path) || !seen.insert(path.to_ascii_lowercase()) {
                return None;
            }
            let size = entry
                .get("size")?
                .as_u64()
                .filter(|size| *size <= FILE_LIMIT)?;
            total = total
                .checked_add(size)
                .filter(|total| *total <= TOTAL_LIMIT)?;
            let sha256 = entry
                .get("sha256")?
                .as_str()
                .filter(|digest| lowercase_sha256(digest))?;
            let executable = match entry.get("executable") {
                None => false,
                Some(Value::Bool(true)) => true,
                Some(_) => return None,
            };
            files.push(WindowFile {
                path: path.to_owned(),
                size,
                sha256: sha256.to_owned(),
                executable,
            });
        }
        let mut links = Vec::with_capacity(linked.len());
        for value in linked {
            let entry = closed_object(value, &["path", "target"], &[])?;
            let path = entry.get("path")?.as_str()?;
            if !admitted(path) || !seen.insert(path.to_ascii_lowercase()) {
                return None;
            }
            links.push(WindowLink {
                path: path.to_owned(),
                target: entry.get("target")?.as_str()?.to_owned(),
            });
        }
        let targets: BTreeMap<&str, &str> = links
            .iter()
            .map(|link| (link.path.as_str(), link.target.as_str()))
            .collect();
        if !links.iter().all(|link| {
            reaches_listed(link_destination(&link.path, &link.target), &files, &targets)
        }) {
            return None;
        }
        Some(Self {
            encoded: bytes.to_vec(),
            identity: format!("{:x}", Sha256::digest(bytes)),
            version,
            files,
            links,
        })
    }
}

/// Where the link at `link` points, as a path of the list, or none when its relative `target`
/// leaves the bundle, the first part of `link`, or is not relative.
fn link_destination(link: &str, target: &str) -> Option<String> {
    if target.is_empty() || target.len() > 240 {
        return None;
    }
    let mut parts: Vec<&str> = link.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        if part == ".." {
            if parts.len() <= 1 {
                return None;
            }
            parts.pop();
        } else if PathGrammar::Bundle.admits(part) {
            parts.push(part);
        } else {
            return None;
        }
    }
    (parts.len() > 1).then(|| parts.join("/"))
}

/// Whether `destination` names a listed file or a folder holding one, after following the listed
/// links on its way, at most [`HOPS`] of them, as the system resolves a path.
fn reaches_listed(
    destination: Option<String>,
    files: &[WindowFile],
    targets: &BTreeMap<&str, &str>,
) -> bool {
    let mut current = destination;
    for _ in 0..HOPS {
        let Some(here) = current.take() else {
            return false;
        };
        let inside = format!("{here}/");
        if files
            .iter()
            .any(|file| file.path == here || file.path.starts_with(&inside))
        {
            return true;
        }
        let parts: Vec<&str> = here.split('/').collect();
        let Some(at) = (1..parts.len())
            .rev()
            .find(|&index| targets.contains_key(parts[..=index].join("/").as_str()))
        else {
            return false;
        };
        let link = parts[..=at].join("/");
        current = link_destination(&link, targets[link.as_str()]).map(|next| {
            std::iter::once(next.as_str())
                .chain(parts[at + 1..].iter().copied())
                .collect::<Vec<_>>()
                .join("/")
        });
    }
    false
}

#[cfg(test)]
mod tests;
