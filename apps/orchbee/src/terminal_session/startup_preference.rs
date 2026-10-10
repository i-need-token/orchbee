//! Client-owned last displayed position; canonical membership remains authoritative.
use super::workbench_projection::{
    NativeContextLocation, NativeContextTabs, NativeMemberTarget, NativeProjectionError,
    project_native_context_tabs,
};
use orchbee_workbench_protocol::Snapshot;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Position {
    pub(super) location: NativeContextLocation,
    pub(super) member: Option<NativeMemberTarget>,
}

pub(super) fn resolve(
    snapshot: &Snapshot,
    position: &Position,
) -> Result<(NativeContextTabs, Option<NativeMemberTarget>, bool), NativeProjectionError> {
    let mut locations = vec![NativeContextLocation::Standalone(
        snapshot.navigation.standalone_terminal_context_id.clone(),
    )];
    for (_, workspace) in snapshot
        .navigation
        .ordered_workspaces()
        .map_err(|_| NativeProjectionError)?
    {
        locations.push(NativeContextLocation::Workspace {
            workspace_id: workspace.workspace_id.clone(),
            terminal_context_id: workspace.terminal_context_id.clone(),
        });
    }
    let location_exists = locations.contains(&position.location);
    let location = if location_exists {
        &position.location
    } else {
        locations
            .iter()
            .find(|location| {
                snapshot.terminal_contexts.iter().any(|context| {
                    &context.terminal_context_id == location.terminal_context_id()
                        && !context.tabs.is_empty()
                })
            })
            .unwrap_or(&locations[0])
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
        Some(NativeMemberTarget {
            tab: tab.tab_id().clone(),
            panel: panel.panel_id().clone(),
            presentation: panel.presentation_id().clone(),
        })
    });
    let fallback = !location_exists || position.member != first;
    Ok((cache, first, fallback))
}

fn member_exists(cache: &NativeContextTabs, member: &NativeMemberTarget) -> bool {
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
pub(crate) struct Draw {
    pub(super) position: Position,
    pub(super) focus: Option<super::state::PaneRef>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Record<P> {
    version: u32,
    position: P,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalPosition {
    location: HistoricalLocation,
    member: Option<NativeMemberTarget>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", content = "identity", deny_unknown_fields)]
enum HistoricalLocation {
    Standalone(orchbee_workbench_protocol::WorkbenchTerminalContextId),
    Workspace {
        project_id: orchbee_workbench_protocol::WorkbenchProjectId,
        workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId,
        terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId,
    },
}
const MAXIMUM_BYTES: usize = 4096;
const FILE: &str = "position.json";

fn decode(bytes: &[u8]) -> std::io::Result<Position> {
    if bytes.len() > MAXIMUM_BYTES {
        return Err(std::io::ErrorKind::FileTooLarge.into());
    }
    // Dispatch by version, then parse the complete strict historical/current DTO. No legacy
    // Project identity survives into Client selection, but malformed old identities still fail.
    #[derive(serde::Deserialize)]
    struct Version {
        version: u32,
    }
    let version: Version =
        serde_json::from_slice(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
    match version.version {
        1 => {
            let record: Record<HistoricalPosition> =
                serde_json::from_slice(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
            let location = match record.position.location {
                HistoricalLocation::Standalone(id) => NativeContextLocation::Standalone(id),
                HistoricalLocation::Workspace {
                    project_id,
                    workspace_id,
                    terminal_context_id,
                } => {
                    drop(project_id);
                    NativeContextLocation::Workspace {
                        workspace_id,
                        terminal_context_id,
                    }
                }
            };
            Ok(Position {
                location,
                member: record.position.member,
            })
        }
        2 => {
            let record: Record<Position> =
                serde_json::from_slice(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
            Ok(record.position)
        }
        _ => Err(std::io::ErrorKind::InvalidData.into()),
    }
}

pub(super) struct Preferences {
    path: std::path::PathBuf,
    pub(super) position: Option<Position>,
    pub(super) failed: bool,
}
impl Preferences {
    pub(super) fn load(runtime: &std::path::Path) -> Self {
        let path = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .map(|root| {
                root.join("Orchbee").join("tui-position").join(
                    orchbee_local_site_runtime::ModeCheckedLocalSiteRendezvous::host_endpoint_id(
                        runtime,
                    ),
                )
            });
        let Some(path) = path else {
            return Self {
                path: Default::default(),
                position: None,
                failed: true,
            };
        };
        let result = orchbee_local_site_runtime::WindowsPrivateCache::read_existing(
            &path,
            FILE,
            MAXIMUM_BYTES,
        )
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
        version: 2,
        position,
    })
    .map_err(std::io::Error::other)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        match orchbee_local_site_runtime::WindowsPrivateCache::acquire(path) {
            Ok(mut cache) => return cache.replace(FILE, &bytes),
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
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
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
    fn position(context: u32) -> Position {
        Position {
            location: NativeContextLocation::Standalone(
                orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(&format!(
                    "{context:032x}"
                ))
                .unwrap(),
            ),
            member: None,
        }
    }
    #[test]
    fn startup_preference_codec_rejects_future_incomplete_unknown_and_oversized_records() {
        let position = position(1);
        let bytes = serde_json::to_vec(&Record {
            version: 2,
            position: position.clone(),
        })
        .unwrap();
        assert_eq!(decode(&bytes).unwrap(), position);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["version"] = 3.into();
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
        value["version"] = 2.into();
        value["binding"] = "foreign authority".into();
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(decode(br#"{"version":1}"#).is_err());
        assert!(decode(&vec![b' '; MAXIMUM_BYTES + 1]).is_err());
    }

    // Historical bytes deliberately retain the old required Project identity.
    const HISTORICAL_WORKSPACE: &[u8] = br#"{
        "version": 1,
        "position": {
            "location": {
                "kind": "Workspace",
                "identity": {
                    "project_id": "00000000000000000000000000000001",
                    "workspace_id": "00000000000000000000000000000002",
                    "terminal_context_id": "00000000000000000000000000000003"
                }
            },
            "member": null
        }
    }"#;

    #[test]
    fn historical_workspace_requires_valid_project_but_current_position_rejects_it() {
        let expected = Position {
            location: NativeContextLocation::Workspace {
                workspace_id: orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                    "00000000000000000000000000000002",
                )
                .unwrap(),
                terminal_context_id: orchbee_workbench_protocol::WorkbenchTerminalContextId::parse(
                    "00000000000000000000000000000003",
                )
                .unwrap(),
            },
            member: None,
        };
        assert_eq!(decode(HISTORICAL_WORKSPACE).unwrap(), expected);
        for invalid in [None, Some(serde_json::Value::Null), Some("invalid".into())] {
            let mut value: serde_json::Value =
                serde_json::from_slice(HISTORICAL_WORKSPACE).unwrap();
            let identity = value["position"]["location"]["identity"]
                .as_object_mut()
                .unwrap();
            match invalid {
                None => {
                    identity.remove("project_id");
                }
                Some(value) => {
                    identity.insert("project_id".into(), value);
                }
            }
            assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        let mut current: serde_json::Value = serde_json::from_slice(HISTORICAL_WORKSPACE).unwrap();
        current["version"] = 2.into();
        assert!(decode(&serde_json::to_vec(&current).unwrap()).is_err());
        current["position"]["location"]["identity"]
            .as_object_mut()
            .unwrap()
            .remove("project_id");
        assert_eq!(
            decode(&serde_json::to_vec(&current).unwrap()).unwrap(),
            expected
        );
    }
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            let actual = self.0.canonicalize().unwrap();
            assert_eq!(
                actual.parent(),
                Some(std::env::temp_dir().canonicalize().unwrap().as_path())
            );
            assert!(
                actual
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchbee-startup-preference-")
            );
            fs::remove_dir_all(actual).unwrap();
        }
    }
    #[test]
    fn startup_preference_writer_flushes_latest_complete_record_before_join() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            Root(std::env::temp_dir().join(format!("orchbee-startup-preference-{nonce:032x}")));
        fs::create_dir(&root.0).unwrap();
        let path = root.0.join("position");
        let lock = orchbee_local_site_runtime::WindowsPrivateCache::acquire(&path).unwrap();
        let mut writer = Writer::start(path.clone()).unwrap();
        writer.publish(position(1));
        writer.publish(position(2));
        writer.publish(position(2));
        drop(lock);
        drop(writer);
        let raw = orchbee_local_site_runtime::WindowsPrivateCache::read_existing(
            &path,
            FILE,
            MAXIMUM_BYTES,
        )
        .unwrap()
        .unwrap();
        assert_eq!(decode(&raw).unwrap(), position(2));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&raw).unwrap()["version"],
            2
        );
        assert!(!path.join(".orchbee-update.pending").exists());
    }

    #[test]
    fn historical_workspace_is_rewritten_as_v2_with_the_same_stable_selection() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            Root(std::env::temp_dir().join(format!("orchbee-startup-preference-{nonce:032x}")));
        fs::create_dir(&root.0).unwrap();
        let path = root.0.join("position");
        let mut cache = orchbee_local_site_runtime::WindowsPrivateCache::acquire(&path).unwrap();
        cache.replace(FILE, HISTORICAL_WORKSPACE).unwrap();
        drop(cache);
        let old = orchbee_local_site_runtime::WindowsPrivateCache::read_existing(
            &path,
            FILE,
            MAXIMUM_BYTES,
        )
        .unwrap()
        .unwrap();
        assert_eq!(old, HISTORICAL_WORKSPACE);
        let migrated = decode(&old).unwrap();
        let mut writer = Writer::start(path.clone()).unwrap();
        writer.publish(migrated.clone());
        drop(writer);
        let raw = orchbee_local_site_runtime::WindowsPrivateCache::read_existing(
            &path,
            FILE,
            MAXIMUM_BYTES,
        )
        .unwrap()
        .unwrap();
        let record: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(record["version"], 2);
        assert!(
            record["position"]["location"]["identity"]
                .get("project_id")
                .is_none()
        );
        assert_eq!(decode(&raw).unwrap(), migrated);
        assert!(!path.join(".orchbee-update.pending").exists());
    }
}
