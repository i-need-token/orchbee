//! Client-owned last displayed position; canonical membership remains authoritative.
use super::workbench_projection::{
    ClientContextLocation, ClientContextTabs, ClientMemberTarget, ClientProjectionError,
    preferred_workspace_location, project_native_context_tabs,
};
use orchbee_workbench_protocol::Snapshot;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix::storage_path;
#[cfg(windows)]
use windows::storage_path;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub(super) location: ClientContextLocation,
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub(super) member: Option<ClientMemberTarget>,
}

/// Startup advice and the resolver's result retained until the initial view settles.
pub(super) struct Startup {
    pub(super) fallback: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(super) failed: bool,
    pub(super) saved: Option<Position>,
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    position: &Position,
) -> Result<(ClientContextTabs, Option<ClientMemberTarget>, bool), ClientProjectionError> {
    let mut locations = Vec::new();
    for (_, workspace) in snapshot
        .navigation
        .ordered_workspaces()
        .map_err(|_| ClientProjectionError)?
    {
        locations.push(ClientContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        });
    }
    let location_exists = locations.contains(&position.location);
    let fallback_location = preferred_workspace_location(snapshot)?;
    let location = if location_exists {
        &position.location
    } else {
        &fallback_location
    };
    let cache = project_native_context_tabs(snapshot, location)?;
    if location_exists
        && position
            .member
            .as_ref()
            .is_some_and(|member| member_exists(&cache, member))
    {
        return Ok((cache, position.member.clone(), false));
    }
    let first = cache.tabs().first().and_then(|tab| {
        let panel = tab.panel(0)?;
        Some(ClientMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: panel.presentation_id().clone(),
        })
    });
    let fallback = !location_exists || position.member != first;
    Ok((cache, first, fallback))
}

fn member_exists(cache: &ClientContextTabs, member: &ClientMemberTarget) -> bool {
    cache
        .tabs()
        .iter()
        .find(|tab| tab.tab_id() == &member.tab)
        .is_some_and(|tab| {
            (0..tab.panel_count())
                .filter_map(|index| tab.panel(index))
                .find(|panel| panel.panel_id() == &member.panel)
                .is_some_and(|panel| {
                    panel
                        .presentations()
                        .any(|p| p.presentation_id() == &member.presentation)
                })
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draw {
    pub(super) position: Position,
    pub(super) focus: Option<super::state::PaneRef>,
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Schema {
    #[serde(rename = "orchbee.tui.position")]
    Position,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Record<P> {
    schema: Schema,
    version: u32,
    position: P,
}

const MAXIMUM_BYTES: usize = 4096;
const FILE: &str = "position.json";

fn decode(bytes: &[u8]) -> std::io::Result<Position> {
    if bytes.len() > MAXIMUM_BYTES {
        return Err(std::io::ErrorKind::FileTooLarge.into());
    }
    let record: Record<Position> =
        serde_json::from_slice(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
    if record.version != 1 {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    Ok(record.position)
}

pub(super) struct Preferences {
    path: std::path::PathBuf,
    pub(super) position: Option<Position>,
    pub(super) failed: bool,
}
impl Preferences {
    pub(super) fn load(runtime: &std::path::Path) -> Self {
        let Ok(path) = storage_path(runtime) else {
            return Self {
                path: Default::default(),
                position: None,
                failed: true,
            };
        };
        let result = orchbee_local_runtime::PrivateCache::read_existing(&path, FILE, MAXIMUM_BYTES)
            .and_then(|bytes| bytes.map(|bytes| decode(&bytes)).transpose());
        match result {
            Ok(position) => Self {
                path,
                position,
                failed: false,
            },
            Err(_) => Self {
                path,
                position: None,
                failed: true,
            },
        }
    }
    pub(super) fn writer(self) -> std::io::Result<Option<Writer>> {
        if self.failed {
            return Ok(None);
        }
        Writer::start(self.path).map(Some)
    }
}

#[derive(Default)]
struct Queue {
    pending: Option<Position>,
    closing: bool,
}
#[derive(Default)]
struct Shared {
    queue: std::sync::Mutex<Queue>,
    wake: std::sync::Condvar,
    failed: std::sync::atomic::AtomicBool,
}
pub(super) struct Writer {
    shared: std::sync::Arc<Shared>,
    task: Option<std::thread::JoinHandle<()>>,
    last: Option<Position>,
}
impl Writer {
    fn start(path: std::path::PathBuf) -> std::io::Result<Self> {
        let shared = std::sync::Arc::new(Shared::default());
        let worker = std::sync::Arc::clone(&shared);
        let task = std::thread::Builder::new()
            .name("orchbee-position-writer".into())
            .spawn(move || {
                loop {
                    let position = {
                        let Ok(mut queue) = worker.queue.lock() else {
                            return;
                        };
                        while queue.pending.is_none() && !queue.closing {
                            let Ok(next) = worker.wake.wait(queue) else {
                                return;
                            };
                            queue = next;
                        }
                        let Some(position) = queue.pending.take() else {
                            return;
                        };
                        position
                    };
                    if write(&path, position).is_err() {
                        worker
                            .failed
                            .store(true, std::sync::atomic::Ordering::Release);
                    }
                }
            })?;
        Ok(Self {
            shared,
            task: Some(task),
            last: None,
        })
    }
    pub(super) fn publish(&mut self, position: Position) {
        if self.last.as_ref() == Some(&position) {
            return;
        }
        let Ok(mut queue) = self.shared.queue.lock() else {
            return;
        };
        queue.pending = Some(position.clone());
        self.last = Some(position);
        self.shared.wake.notify_one();
    }
    pub(super) fn take_failure(&self) -> bool {
        self.shared
            .failed
            .swap(false, std::sync::atomic::Ordering::AcqRel)
    }
}
fn write(path: &std::path::Path, position: Position) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(&Record {
        schema: Schema::Position,
        version: 1,
        position,
    })
    .map_err(std::io::Error::other)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match orchbee_local_runtime::PrivateCache::acquire(path) {
            Ok(mut cache) => return cache.replace(FILE, &bytes),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                // The OS cache lock exposes contention, not a release notification. This
                // one-second persistence job samples only while another writer owns the file.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.shared.queue.lock() {
            queue.closing = true;
            self.shared.wake.notify_one();
        }
        if let Some(task) = self.task.take() {
            let _joined = task.join();
        }
    }
}

#[cfg(test)]
mod tests;
