//! Client-owned last displayed position; canonical membership remains authoritative.
use crate::workbench_projection::{
    ClientContextLocation, ClientContextTabs, ClientMemberTarget, ClientProjectionError,
    preferred_workspace_location, project_native_context_tabs,
};
use orchbee_workbench_protocol::Snapshot;

pub(crate) mod store;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub(crate) location: ClientContextLocation,
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub(crate) member: Option<ClientMemberTarget>,
}

/// Startup advice and the resolver's result retained until the initial view settles.
pub(crate) struct Startup {
    /// The first record this Session's stores cannot use, shown once the initial view settles.
    pub(crate) record: Option<orchbee_local_runtime::RecordNotice>,
    pub(crate) fallback: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) failed: bool,
    /// The saved record could not be read; startup proceeded as if none was saved.
    pub(crate) unreadable: bool,
    pub(crate) saved: Option<Position>,
}

pub(crate) fn resolve(
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
    pub(crate) position: Position,
    pub(crate) focus: Option<crate::workbench_state::PaneRef>,
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
/// `tui-position.json` in the Session folder.
const FILE: &str = "tui-position.json";
/// Names the writer lock and staging file beside `tui-position.json`.
const RECORD: &str = orchbee_local_runtime::TUI_POSITION_RECORD;

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

#[cfg(test)]
mod tests;
