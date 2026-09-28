//! Real remote-rule cache updates. Observer installation and public receipts are separate owners.
use super::{ScreenDetector, manifest::Manifest, version::Version};
use orchbee_agent_observation_protocol::AgentReportProvider;
use orchbee_local_site_runtime::WindowsPrivateCache;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::BTreeSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering as AtomicOrdering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod fetch;
const LIMIT: usize = 256 * 1024;
const UPDATE_BUDGET: Duration = Duration::from_secs(600);
pub(crate) const DEFAULT_CATALOG: &str = "https://herdr.dev/agent-detection/index.toml";
pub(crate) const CATALOG_ENV: &str = "ORCHBEE_AGENT_DETECTION_MANIFEST_CATALOG_URL";

#[derive(Clone, Debug)]
pub(crate) struct CatalogUrl(String);
impl CatalogUrl {
    pub(crate) fn parse(value: &str) -> Result<Self, Failure> {
        if value.len() > 4096
            || !value.is_ascii()
            || value
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
            || value.contains(['\\', '#', '?'])
        {
            return Err(Failure::InvalidSource);
        }
        let tail = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .ok_or(Failure::InvalidSource)?;
        let (authority, _) = tail.split_once('/').ok_or(Failure::InvalidSource)?;
        if authority.is_empty()
            || !authority
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
        {
            return Err(Failure::InvalidSource);
        }
        Ok(Self(value.into()))
    }
    fn join(&self, path: &str) -> Result<Self, Failure> {
        if path.is_empty()
            || path.len() > 512
            || path.starts_with('/')
            || !path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'-' | b'_' | b'~'))
            || path
                .split('/')
                .any(|p| p.is_empty() || matches!(p, "." | ".."))
        {
            return Err(Failure::InvalidCatalog);
        }
        let (base, _) = self.0.rsplit_once('/').ok_or(Failure::InvalidSource)?;
        Self::parse(&format!("{base}/{path}"))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Failure {
    InvalidSource,
    InvalidCatalog,
    Fetch,
    TooLarge,
    InvalidUtf8,
    InvalidManifest,
    Cache,
    Busy,
    OlderVersion,
    ChangedSameVersion,
    Cancelled,
    Deadline,
    Clock,
    Process,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Disposition {
    Updated,
    Current,
    Failed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderUpdate {
    pub(crate) agent: String,
    pub(crate) attempted_version: Option<String>,
    pub(crate) cached_version: Option<String>,
    pub(crate) result: Disposition,
    pub(crate) error: Option<Failure>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UpdateReport {
    pub(crate) version: u32,
    pub(crate) catalog_url: String,
    pub(crate) checked_unix_seconds: u64,
    pub(crate) agents: Vec<ProviderUpdate>,
    pub(crate) failure: Option<Failure>,
    pub(crate) status_saved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    #[serde(default)]
    agents: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    path: String,
}
fn catalog(
    text: &str,
    source: &CatalogUrl,
) -> Result<Vec<(AgentReportProvider, CatalogUrl)>, Failure> {
    if text.len() > LIMIT {
        return Err(Failure::TooLarge);
    }
    let catalog: Catalog = toml::from_str(text).map_err(|_| Failure::InvalidCatalog)?;
    if catalog.schema_version != 1 || catalog.agents.len() > 64 {
        return Err(Failure::InvalidCatalog);
    }
    let supported = ScreenDetector::bundled().map_err(|_| Failure::InvalidManifest)?;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for entry in catalog.agents {
        let Some(provider) = AgentReportProvider::lookup(&entry.id) else {
            continue;
        };
        if !supported.0.iter().any(|m| m.provider == provider) {
            continue;
        }
        if !seen.insert(provider.report_label()) {
            return Err(Failure::InvalidCatalog);
        }
        result.push((provider, source.join(&entry.path)?));
    }
    Ok(result)
}
fn validate(provider: AgentReportProvider, text: &str) -> Result<(Version, String), Failure> {
    let manifest = Manifest::parse(provider, text).map_err(|_| Failure::InvalidManifest)?;
    if manifest.minimum_engine_version.is_none() {
        return Err(Failure::InvalidManifest);
    }
    let version = manifest.version.ok_or(Failure::InvalidManifest)?;
    let numeric = Version::parse(&version).ok_or(Failure::InvalidManifest)?;
    Ok((numeric, version.trim().to_owned()))
}
fn cancelled(stop: &AtomicBool, deadline: Instant) -> Result<(), Failure> {
    if stop.load(AtomicOrdering::Acquire) {
        Err(Failure::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Failure::Deadline)
    } else {
        Ok(())
    }
}
fn update_provider(
    cache: &mut WindowsPrivateCache,
    provider: AgentReportProvider,
    text: &str,
    stop: &AtomicBool,
    deadline: Instant,
    row: &mut ProviderUpdate,
) -> Result<Disposition, Failure> {
    let (version, display) = validate(provider, text)?;
    row.attempted_version = Some(display.clone());
    let name = format!("{}.toml", provider.report_label());
    if let Some(bytes) = cache.read(&name, LIMIT).map_err(|_| Failure::Cache)? {
        let current = String::from_utf8(bytes).map_err(|_| Failure::Cache)?;
        let (cached, cached_display) = validate(provider, &current).map_err(|_| Failure::Cache)?;
        row.cached_version = Some(cached_display);
        match version.cmp(&cached) {
            Ordering::Less => return Err(Failure::OlderVersion),
            Ordering::Equal if text != current => return Err(Failure::ChangedSameVersion),
            Ordering::Equal => return Ok(Disposition::Current),
            Ordering::Greater => {}
        }
    }
    cancelled(stop, deadline)?;
    cache
        .replace(&name, text.as_bytes())
        .map_err(|_| Failure::Cache)?;
    row.cached_version = Some(display);
    Ok(Disposition::Updated)
}
/// The retained updater thread calls this once. Every fetch is a real bounded owned curl Job.
pub(crate) fn run(root: &Path, source: &CatalogUrl, stop: &AtomicBool) -> UpdateReport {
    run_with_fetch(
        root,
        source,
        stop,
        Instant::now() + UPDATE_BUDGET,
        fetch::text,
    )
}
fn run_with_fetch(
    root: &Path,
    source: &CatalogUrl,
    stop: &AtomicBool,
    deadline: Instant,
    mut fetch: impl FnMut(&CatalogUrl, &AtomicBool, Instant) -> Result<String, Failure>,
) -> UpdateReport {
    let mut report = UpdateReport {
        version: 1,
        catalog_url: source.0.clone(),
        checked_unix_seconds: 0,
        agents: vec![],
        failure: None,
        status_saved: false,
    };
    let mut cache = None;
    let mut status_admitted = false;
    let result = (|| {
        cancelled(stop, deadline)?;
        report.checked_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Failure::Clock)?
            .as_secs();
        cache = Some(
            WindowsPrivateCache::acquire(&root.join("agent-detection").join("remote")).map_err(
                |e| {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        Failure::Busy
                    } else {
                        Failure::Cache
                    }
                },
            )?,
        );
        if let Some(bytes) = cache
            .as_ref()
            .ok_or(Failure::Cache)?
            .read("status.json", LIMIT)
            .map_err(|_| Failure::Cache)?
        {
            let previous: UpdateReport =
                serde_json::from_slice(&bytes).map_err(|_| Failure::Cache)?;
            if previous.version != 1 || previous.agents.len() > 22 {
                return Err(Failure::Cache);
            }
        }
        status_admitted = true;
        let entries = catalog(&fetch(source, stop, deadline)?, source)?;
        for (provider, url) in entries {
            cancelled(stop, deadline)?;
            let mut row = ProviderUpdate {
                agent: provider.report_label().into(),
                attempted_version: None,
                cached_version: None,
                result: Disposition::Failed,
                error: None,
            };
            let result = fetch(&url, stop, deadline).and_then(|text| {
                update_provider(
                    cache.as_mut().ok_or(Failure::Cache)?,
                    provider,
                    &text,
                    stop,
                    deadline,
                    &mut row,
                )
            });
            match result {
                Ok(result) => row.result = result,
                Err(error) => row.error = Some(error),
            }
            // Preserve current cache provenance even when the download itself failed.
            if row.cached_version.is_none()
                && let Some(cache) = cache.as_ref()
                && let Ok(Some(bytes)) =
                    cache.read(&format!("{}.toml", provider.report_label()), LIMIT)
                && let Ok(text) = String::from_utf8(bytes)
                && let Ok((_, version)) = validate(provider, &text)
            {
                row.cached_version = Some(version);
            }
            let interrupted = row
                .error
                .filter(|e| matches!(e, Failure::Cancelled | Failure::Deadline));
            report.agents.push(row);
            if let Some(error) = interrupted {
                return Err(error);
            }
        }
        Ok(())
    })();
    report.failure = result.err();
    if let Some(mut cache) = cache
        && status_admitted
    {
        // The persisted status describes this attempt; publication failure remains visible in memory.
        report.status_saved = true;
        if serde_json::to_vec(&report)
            .ok()
            .filter(|b| b.len() <= LIMIT)
            .is_none_or(|bytes| cache.replace("status.json", &bytes).is_err())
        {
            report.status_saved = false;
            report.failure.get_or_insert(Failure::Cache);
        }
    }
    report
}

#[cfg(test)]
mod tests;
