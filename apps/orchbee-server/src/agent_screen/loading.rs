//! Bounded local inputs select one complete compiled manifest; failed candidates never execute.
use super::version::Version;
use super::{ScreenDetector, manifest::Manifest};
use orchbee_workbench_protocol::{
    AgentDetectionManifest, AgentManifestSource as Source, AgentManifestWarning as Warning,
    AgentManifestWarningKind as Reason,
};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
const MAXIMUM_MANIFEST_BYTES: u64 = 256 * 1024;
/// Whether cached remote rules join selection, read from the configured catalog's directory.
/// Ignored rules are never read or reported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RemoteCache {
    Admitted(PathBuf),
    Ignored,
}

fn read(path: &Path) -> Result<Option<String>, Reason> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Reason::Unreadable),
    };
    let metadata = file.metadata().map_err(|_| Reason::Unreadable)?;
    if !metadata.is_file() {
        return Err(Reason::NotRegular);
    }
    if metadata.len() > MAXIMUM_MANIFEST_BYTES {
        return Err(Reason::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MAXIMUM_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Reason::Unreadable)?;
    if bytes.len() as u64 > MAXIMUM_MANIFEST_BYTES {
        return Err(Reason::TooLarge);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| Reason::InvalidUtf8)
}
fn path_text(path: &Path) -> Result<String, Reason> {
    let path = path.to_str().ok_or(Reason::Unreadable)?;
    if path.is_empty() || path.len() > 32768 || path.chars().any(char::is_control) {
        return Err(Reason::Unreadable);
    }
    Ok(path.to_owned())
}
fn candidate(
    provider: orchbee_agent_observation_protocol::AgentReportProvider,
    path: &Path,
) -> Result<Option<Manifest>, Reason> {
    let Some(text) = read(path)? else {
        return Ok(None);
    };
    path_text(path)?;
    Manifest::parse(provider, &text)
        .map(Some)
        .map_err(|_| Reason::InvalidManifest)
}
impl ScreenDetector {
    /// Startup and later reloads use the same deterministic source ordering without writing inputs.
    pub(crate) fn load(
        configuration_root: Option<&Path>,
        remote_cache: RemoteCache,
    ) -> Result<Self, String> {
        let mut detector = Self::bundled()?;
        let Some(root) = configuration_root else {
            return Ok(detector);
        };
        let root = root.join("agent-detection");
        for current in &mut detector.0 {
            let filename = format!("{}.toml", current.provider.report_label());
            let local = root.join(&filename);
            let mut info = AgentDetectionManifest {
                source: Source::Bundled,
                path: None,
                cached_remote_version: None,
                local_override_shadowing_remote: false,
                warnings: vec![],
            };
            let bundled_version = current.version.as_deref().and_then(Version::parse);
            let remote_candidate = match &remote_cache {
                RemoteCache::Admitted(directory) => {
                    let path = directory.join(&filename);
                    candidate(current.provider, &path).map(|found| found.map(|found| (path, found)))
                }
                RemoteCache::Ignored => Ok(None),
            };
            match remote_candidate {
                Ok(Some((_, candidate))) if candidate.minimum_engine_version.is_none() => {
                    info.warnings.push(Warning {
                        source: Source::RemoteCache,
                        kind: Reason::InvalidManifest,
                    });
                }
                Ok(Some((remote, mut candidate))) => {
                    match candidate.version.as_deref().and_then(Version::parse) {
                        None => info.warnings.push(Warning {
                            source: Source::RemoteCache,
                            kind: Reason::InvalidVersion,
                        }),
                        Some(version) => {
                            let value = candidate
                                .version
                                .as_deref()
                                .expect("parsed remote version")
                                .trim()
                                .to_owned();
                            candidate.version = Some(value.clone());
                            info.cached_remote_version = Some(value);
                            if bundled_version
                                .as_ref()
                                .is_some_and(|bundled| version < *bundled)
                            {
                                info.warnings.push(Warning {
                                    source: Source::RemoteCache,
                                    kind: Reason::OlderThanBundled,
                                });
                            } else {
                                info.source = Source::RemoteCache;
                                info.path = Some(
                                    path_text(&remote)
                                        .map_err(|_| "Invalid admitted remote path")?,
                                );
                                *current = candidate;
                            }
                        }
                    }
                }
                Err(kind) => info.warnings.push(Warning {
                    source: Source::RemoteCache,
                    kind,
                }),
                Ok(None) => {}
            }
            match candidate(current.provider, &local) {
                Ok(Some(candidate)) => {
                    info.local_override_shadowing_remote = info.cached_remote_version.is_some();
                    info.source = Source::LocalOverride;
                    info.path =
                        Some(path_text(&local).map_err(|_| "Invalid admitted override path")?);
                    *current = candidate;
                }
                Err(kind) => {
                    info.local_override_shadowing_remote = info.cached_remote_version.is_some();
                    info.warnings.push(Warning {
                        source: Source::LocalOverride,
                        kind,
                    });
                }
                Ok(None) => {}
            }
            current.provenance = info;
        }
        Ok(detector)
    }
}

#[cfg(test)]
mod tests;
