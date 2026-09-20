//! Pure projection of a validated Workbench `Snapshot` into the CLI result
//! model. Everything here is total over a snapshot the transport already
//! verified; no fabricated fields appear where no authoritative source exists.

use std::collections::HashMap;

use orchbee_client_server_protocol::TerminalLaunchFact;
use orchbee_terminal_protocol::MAXIMUM_FRAME_ROWS;
use orchbee_workbench_protocol::{
    PanelNode, PanelSlot, Presentation, Project, Snapshot, Tab, TerminalContext,
    WorkbenchProtocolDocument, Workspace, WorktreeEntry, WorktreeEntryText,
};
use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, target_details};
use crate::invocation::{QueryCommand, Resource, Target};

pub(crate) fn schema_id(command: &QueryCommand) -> &'static str {
    match command {
        QueryCommand::Status => "orchbee.status",
        QueryCommand::ProjectList => "orchbee.project.list",
        QueryCommand::ProjectGet(_) => "orchbee.project.get",
        QueryCommand::WorkspaceList { .. } => "orchbee.workspace.list",
        QueryCommand::WorkspaceGet(_) => "orchbee.workspace.get",
        QueryCommand::TabList { .. } => "orchbee.tab.list",
        QueryCommand::TabGet(_) => "orchbee.tab.get",
        QueryCommand::PanelList { .. } => "orchbee.panel.list",
        QueryCommand::PanelGet(_) => "orchbee.panel.get",
        QueryCommand::TerminalList => "orchbee.terminal.list",
        QueryCommand::TerminalGet(_) => "orchbee.terminal.get",
        QueryCommand::TerminalRead { history, .. } => {
            if history.is_some() {
                "orchbee.terminal.history"
            } else {
                "orchbee.terminal.read"
            }
        }
        QueryCommand::TerminalWaitOutput { .. } => "orchbee.terminal.wait-output",
        QueryCommand::WorktreeList { .. } => "orchbee.worktree.list",
        QueryCommand::ApiSnapshot => "orchbee.api.snapshot",
        QueryCommand::ApiSchema { .. } => "orchbee.api.schema",
        QueryCommand::ConfigCheck => "orchbee.config.check",
    }
}

/// `terminal get` is the one document at `version:2`: it gained the `launch`
/// member and a Server-joined source, a deliberate break older `version:1`
/// validators reject. Every other envelope stays at `1`.
pub(crate) const fn schema_version(command: &QueryCommand) -> u64 {
    match command {
        QueryCommand::TerminalGet(_) => 2,
        _ => 1,
    }
}

/// `source` for every snapshot-derived answer.
pub(crate) fn snapshot_source(snapshot: &Snapshot) -> Value {
    json!({
        "kind": "workbench-snapshot",
        "siteId": snapshot.site_id.as_str(),
        "revision": snapshot.revision,
    })
}

/// `source` for the one answer the Server itself joined: the launch read's
/// site and the Workbench revision it observed under the coordinator lock —
/// a decimal string like every u64 in a machine document.
pub(crate) fn launch_read_source(site_id: &str, revision: u64) -> Value {
    json!({
        "kind": "terminal-launch-read",
        "siteId": site_id,
        "revision": revision.to_string(),
    })
}

#[derive(Clone, Debug)]
pub(crate) struct StatusDoc {
    pub(crate) site_id: String,
    pub(crate) revision: String,
    pub(crate) projects: usize,
    pub(crate) workspaces: usize,
    pub(crate) tabs: usize,
    pub(crate) panels: usize,
    pub(crate) bound_terminals: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectRow {
    pub(crate) project_id: String,
    pub(crate) label: String,
    pub(crate) directory_count: usize,
    pub(crate) workspace_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryRow {
    pub(crate) directory_id: String,
    pub(crate) worker_id: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceRef {
    pub(crate) workspace_id: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectDoc {
    pub(crate) row: ProjectRow,
    pub(crate) directories: Vec<DirectoryRow>,
    pub(crate) workspaces: Vec<WorkspaceRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceRow {
    pub(crate) workspace_id: String,
    pub(crate) label: String,
    pub(crate) project_id: String,
    pub(crate) primary_directory_id: Option<String>,
    pub(crate) terminal_context_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TabRef {
    pub(crate) tab_id: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceDoc {
    pub(crate) row: WorkspaceRow,
    pub(crate) tabs: Vec<TabRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct ContextRef {
    pub(crate) standalone: bool,
    pub(crate) terminal_context_id: String,
    pub(crate) workspace_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TabRow {
    pub(crate) tab_id: String,
    pub(crate) label: String,
    pub(crate) pinned: bool,
    pub(crate) group_id: Option<String>,
    pub(crate) context: ContextRef,
}

#[derive(Clone, Debug)]
pub(crate) struct TabDoc {
    pub(crate) row: TabRow,
    /// Semantic re-encode of the whole PanelNode subtree.
    pub(crate) root: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct PanelRow {
    pub(crate) panel_id: String,
    pub(crate) tab_id: String,
    pub(crate) terminal_context_id: String,
    pub(crate) primary_presentation_id: String,
    pub(crate) presentation_count: usize,
    pub(crate) primary_terminal_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PanelDoc {
    pub(crate) row: PanelRow,
    pub(crate) context: ContextRef,
    pub(crate) slot_path: Vec<String>,
    pub(crate) primary_presentation: PresentationRow,
    pub(crate) child_presentations: Vec<PresentationRow>,
}

#[derive(Clone, Debug)]
pub(crate) struct PresentationRow {
    pub(crate) presentation_id: String,
    pub(crate) label: String,
    pub(crate) terminal_id: Option<String>,
    pub(crate) worker_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalRow {
    pub(crate) terminal_id: String,
    pub(crate) worker_id: String,
    pub(crate) presentation_id: String,
    pub(crate) panel_id: String,
    pub(crate) tab_id: String,
    pub(crate) terminal_context_id: String,
}

/// `terminal get` v2: the Server's point-in-time join — the current routing
/// row plus the retained launch fact under one coordinator lock. `terminal`
/// keeps the shared identity shape; only this document version carries
/// `launch`.
#[derive(Clone, Debug)]
pub(crate) struct TerminalGetDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) launch: TerminalLaunchFact,
}

/// Routing metadata of the applied projection, re-encoded as decimal strings
/// so no u64 ever loses precision inside a machine document.
#[derive(Clone, Debug)]
pub(crate) struct TerminalFrameDoc {
    pub(crate) revision: u64,
    pub(crate) generation: u64,
    pub(crate) sequence: u64,
    pub(crate) alternate_screen: bool,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) cursor_row: u16,
    pub(crate) cursor_column: u16,
    pub(crate) cursor_visible: bool,
}

/// One grid row emitted in the answer: absolute index, joined cell text, and
/// the soft-wrap continuation flags the frame carried.
#[derive(Clone, Debug)]
pub(crate) struct TerminalContentRow {
    pub(crate) index: u16,
    pub(crate) text: String,
    pub(crate) from_previous: bool,
    pub(crate) into_next: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalReadDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) frame: TerminalFrameDoc,
    /// Emitted rows only; `row_count` counts every row in the applied frame.
    pub(crate) rows: Vec<TerminalContentRow>,
    pub(crate) row_count: usize,
    pub(crate) omitted_rows: usize,
}

/// One history row emitted in the answer: absolute index into the complete
/// retained-plus-screen coordinate space, the row's text, and its soft-wrap
/// continuation flags.
#[derive(Clone, Debug)]
pub(crate) struct HistoryContentRow {
    pub(crate) index: u64,
    pub(crate) text: String,
    pub(crate) from_previous: bool,
    pub(crate) into_next: bool,
}

/// `terminal read --history`: the validated frame facts and the bounded
/// scrollback window. `window_rows` is the Host row window's complete row
/// count; `omitted_rows` counts the gap between `min(requested, scrollback)`
/// and the rows actually emitted.
#[derive(Clone, Debug)]
pub(crate) struct TerminalHistoryDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) revision: u64,
    pub(crate) generation: u64,
    pub(crate) alternate_screen: bool,
    pub(crate) columns: u16,
    pub(crate) screen_rows: u16,
    pub(crate) scrollback_rows: u64,
    pub(crate) total_rows: u64,
    pub(crate) start_row: u64,
    pub(crate) requested_rows: u16,
    pub(crate) window_rows: usize,
    pub(crate) rows: Vec<HistoryContentRow>,
    pub(crate) omitted_rows: u64,
}

/// The read bound: the summed JSON-encoded `text` member length of emitted
/// rows never exceeds this budget, so every format stays far under the
/// 8 MiB preparation cap even on hostile cell content.
pub(crate) const READ_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// Project the applied frame into the bounded answer: rows are emitted in
/// frame order until the encoded-text budget would overflow, and every
/// omitted row is counted honestly.
#[cfg(unix)]
pub(crate) fn terminal_read_doc(
    terminal: TerminalRow,
    frame: &orchbee_terminal_protocol::TerminalTextFrame,
) -> Result<TerminalReadDoc, Failure> {
    let row_count = frame.rows.len();
    let mut budget = READ_TEXT_BYTES;
    let mut rows = Vec::new();
    for row in &frame.rows {
        let mut text = String::new();
        for cell in &row.cells {
            // A spacer tail is already covered by its wide cell; a spacer
            // head or an empty cell renders as one space.
            if cell.width == 2 {
                continue;
            }
            if cell.width == 3 || cell.text.is_empty() {
                text.push(' ');
            } else {
                text.push_str(&cell.text);
            }
        }
        // Trailing cells are grid padding, never content: a blank-only row
        // emits an empty line.
        while text.ends_with(' ') {
            text.pop();
        }
        let encoded =
            serde_json::to_string(&text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
        if encoded.len() > budget {
            break;
        }
        budget -= encoded.len();
        rows.push(TerminalContentRow {
            index: row.index,
            text,
            // Wire bits: bit 0 is `wrap` (this row continues into the next),
            // bit 1 is `wrap_continuation` (this row continues from the
            // previous); the decoder pairs them across adjacent rows.
            from_previous: row.flags & 2 != 0,
            into_next: row.flags & 1 != 0,
        });
    }
    let doc = TerminalFrameDoc {
        revision: frame.route.revision,
        generation: frame.route.generation.get(),
        sequence: frame.route.sequence.get(),
        alternate_screen: frame.alternate_screen,
        columns: frame.geometry.columns,
        rows: frame.geometry.rows,
        cursor_row: frame.cursor.row,
        cursor_column: frame.cursor.column,
        cursor_visible: frame.cursor.visible,
    };
    Ok(TerminalReadDoc {
        terminal,
        frame: doc,
        omitted_rows: row_count - rows.len(),
        rows,
        row_count,
    })
}

/// The history read bound admitted by the `--rows` grammar: 1..=1000.
pub(crate) const MAXIMUM_HISTORY_ROWS: u16 = 1000;

/// Project a validated history window into the bounded answer. `window` is
/// the complete Host row window the CLI verified against its request; emitted
/// rows stop when the encoded-text budget would overflow, and `omitted_rows`
/// honestly counts `min(requested, scrollback)` minus the emitted rows —
/// including rows the protocol window cap never reached.
#[cfg(unix)]
pub(crate) fn terminal_history_doc(
    terminal: TerminalRow,
    frame: &orchbee_terminal_protocol::TerminalTextFrame,
    requested_rows: u16,
    window: Option<&orchbee_terminal_protocol::TerminalTextRowWindow>,
) -> Result<TerminalHistoryDoc, Failure> {
    let scrollback_rows = frame.scrollback_rows;
    let screen_rows = frame.geometry.rows;
    let total_rows = scrollback_rows
        .checked_add(u64::from(screen_rows))
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    let (start_row, window_rows, window_rows_slice) = match window {
        Some(window) => (window.start_row, window.rows.len(), window.rows.as_slice()),
        None => (0, 0, &[][..]),
    };
    let mut budget = READ_TEXT_BYTES;
    let mut rows = Vec::new();
    for row in window_rows_slice {
        let text = row.text.to_string();
        let encoded =
            serde_json::to_string(&text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
        if encoded.len() > budget {
            break;
        }
        budget -= encoded.len();
        rows.push(HistoryContentRow {
            index: row.index,
            text,
            from_previous: row.wrap_continuation,
            into_next: row.wrap,
        });
    }
    // Rows the window cap or text budget never emitted still count against
    // the caller's `min(requested, scrollback)` expectation.
    let entitled = u64::from(requested_rows).min(scrollback_rows);
    let omitted_rows = entitled.saturating_sub(u64::try_from(rows.len()).unwrap_or(u64::MAX));
    Ok(TerminalHistoryDoc {
        terminal,
        revision: frame.route.revision,
        generation: frame.route.generation.get(),
        alternate_screen: frame.alternate_screen,
        columns: frame.geometry.columns,
        screen_rows,
        scrollback_rows,
        total_rows,
        start_row,
        requested_rows,
        window_rows,
        rows,
        omitted_rows,
    })
}

/// One literal-substring hit inside a joined logical line, plus the bounded
/// excerpt that always contains the complete match.
#[derive(Clone, Debug)]
pub(crate) struct WaitMatch {
    pub(crate) excerpt: String,
    pub(crate) excerpt_truncated: bool,
    /// Byte offset of the excerpt within the matched logical line.
    pub(crate) excerpt_offset: u64,
    /// Byte offset of the pattern within the matched logical line.
    pub(crate) match_offset: u64,
    /// First and last physical rows the matched logical line spans.
    pub(crate) row_start: u64,
    pub(crate) row_end: u64,
}

/// The observed-window facts a successful wait reports: the consistent
/// window's own routing and extent, not a frame-ack or revision count.
#[derive(Debug)]
pub(crate) struct WaitObservation {
    pub(crate) matched: WaitMatch,
    pub(crate) revision: u64,
    pub(crate) generation: u64,
    pub(crate) window_start: u64,
    pub(crate) window_rows: u64,
    pub(crate) total_rows: u64,
}

/// `terminal wait-output`: the resolved binding plus the honest match and
/// observation record. Nothing here claims command completion, a new turn,
/// or complete output capture.
#[derive(Debug)]
pub(crate) struct TerminalWaitOutputDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) attachment_id: String,
    pub(crate) host_epoch: String,
    pub(crate) matched: WaitMatch,
    pub(crate) revision: u64,
    pub(crate) generation: u64,
    pub(crate) window_start: u64,
    pub(crate) window_rows: u64,
    /// `true` when the byte-capacity clamp — not the requested rows or the
    /// retained total — bound the searched window.
    pub(crate) window_truncated: bool,
    pub(crate) total_rows: u64,
    pub(crate) requested_rows: u16,
    /// Successful complete row-window reads that each also completed one
    /// full match pass.
    pub(crate) observations: u64,
    pub(crate) timeout_ms: u32,
    pub(crate) elapsed_ms: u64,
}

/// The match excerpt bound: at most this many UTF-8 bytes, always
/// containing the complete pattern.
pub(crate) const MAXIMUM_EXCERPT_BYTES: usize = 4096;

/// Join every wrap-compatible row pair into logical lines — the decoder
/// already proved `wrap`/`wrap_continuation` agree on every adjacent pair —
/// then find the first logical line containing `pattern` as a literal,
/// case-sensitive substring. Window-edge fragments never reach outside the
/// window and a hard boundary never joins.
#[cfg(unix)]
pub(crate) fn match_window(
    window: &orchbee_terminal_protocol::TerminalTextRowWindow,
    pattern: &str,
) -> Option<WaitMatch> {
    let mut text = String::new();
    let mut row_start = 0_u64;
    let mut row_end = 0_u64;
    for row in &window.rows {
        if !row.wrap_continuation && !text.is_empty() {
            // A hard boundary completes the previous logical line; it is
            // matched before the new one accumulates.
            if let Some(matched) = match_line(&text, row_start, row_end, pattern) {
                return Some(matched);
            }
            text.clear();
        }
        if text.is_empty() {
            // A hard boundary — or a window opening mid-line on a
            // continuation fragment — starts the visible logical line here.
            row_start = row.index;
        }
        text.push_str(&row.text);
        row_end = row.index;
    }
    if !text.is_empty() {
        return match_line(&text, row_start, row_end, pattern);
    }
    None
}

/// The literal position of `pattern` inside one complete logical line, plus
/// the bounded excerpt around it.
#[cfg(unix)]
fn match_line(text: &str, row_start: u64, row_end: u64, pattern: &str) -> Option<WaitMatch> {
    text.find(pattern).map(|offset| {
        let (excerpt, truncated, excerpt_offset) = match_excerpt(text, offset, pattern.len());
        WaitMatch {
            excerpt,
            excerpt_truncated: truncated,
            excerpt_offset,
            match_offset: u64::try_from(offset).unwrap_or(u64::MAX),
            row_start,
            row_end,
        }
    })
}

/// A ≤4096-byte slice of `line` that always contains the complete match at
/// `match_start..match_start+pattern_len`, on UTF-8 boundaries: the match
/// is centered inside the budget, and the window re-anchors on the match
/// when a tail hit would otherwise lose bytes to boundary alignment. No
/// code point is ever split.
#[cfg(unix)]
fn match_excerpt(line: &str, match_start: usize, pattern_len: usize) -> (String, bool, u64) {
    if line.len() <= MAXIMUM_EXCERPT_BYTES {
        return (line.to_owned(), false, 0);
    }
    let match_end = match_start + pattern_len;
    let context = (MAXIMUM_EXCERPT_BYTES - pattern_len) / 2;
    let mut start = match_start.saturating_sub(context);
    while !line.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = start + MAXIMUM_EXCERPT_BYTES;
    if end > line.len() {
        end = line.len();
        start = end - MAXIMUM_EXCERPT_BYTES;
        while !line.is_char_boundary(start) {
            start += 1;
        }
    } else {
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        if end < match_end {
            // The match touches the aligned end; re-anchor on its boundary.
            end = match_end;
            start = end - MAXIMUM_EXCERPT_BYTES;
            while !line.is_char_boundary(start) {
                start += 1;
            }
        }
    }
    (
        line[start..end].to_owned(),
        true,
        u64::try_from(start).unwrap_or(u64::MAX),
    )
}

/// The run facts a wait answer reports beyond the matched window: how many
/// consistent reads completed a match pass, what the caller asked for, and
/// how long the whole wait took.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WaitReport {
    pub(crate) observations: u64,
    pub(crate) requested_rows: u16,
    pub(crate) timeout_ms: u32,
    pub(crate) elapsed_ms: u64,
}

/// Project the confirmed observation into the answer document; every field
/// already carries validated transport facts, so the projection itself
/// cannot fail.
#[cfg(unix)]
pub(crate) fn terminal_wait_output_doc(
    terminal: TerminalRow,
    attachment: orchbee_terminal_protocol::AttachmentId,
    epoch: orchbee_terminal_protocol::HostEpoch,
    observed: WaitObservation,
    report: WaitReport,
) -> TerminalWaitOutputDoc {
    TerminalWaitOutputDoc {
        terminal,
        attachment_id: attachment.to_text(),
        host_epoch: epoch.to_text(),
        matched: observed.matched,
        revision: observed.revision,
        generation: observed.generation,
        window_start: observed.window_start,
        window_rows: observed.window_rows,
        window_truncated: observed.window_rows
            < u64::from(report.requested_rows).min(observed.total_rows),
        total_rows: observed.total_rows,
        requested_rows: report.requested_rows,
        observations: report.observations,
        timeout_ms: report.timeout_ms,
        elapsed_ms: report.elapsed_ms,
    }
}

/// The bound terminal row for a typed identity, or `None` when no
/// presentation binds it — the same visibility rule as `terminal get`.
pub(crate) fn terminal_binding(snapshot: &Snapshot, identity: &str) -> Option<TerminalRow> {
    terminal_rows(snapshot)
        .into_iter()
        .find(|row| row.terminal_id == identity)
}

#[derive(Debug)]
pub(crate) enum Projection {
    Status(StatusDoc),
    ProjectList(Vec<ProjectRow>),
    ProjectGet(ProjectDoc),
    WorkspaceList(Vec<WorkspaceRow>),
    WorkspaceGet(WorkspaceDoc),
    TabList(Vec<TabRow>),
    TabGet(TabDoc),
    PanelList(Vec<PanelRow>),
    PanelGet(PanelDoc),
    TerminalList(Vec<TerminalRow>),
    TerminalGet(TerminalGetDoc),
    TerminalRead(TerminalReadDoc),
    TerminalHistory(TerminalHistoryDoc),
    TerminalWaitOutput(TerminalWaitOutputDoc),
    /// The `worktree list` answer: the registered directory identity plus the
    /// observed Git entries, exactly as the workbench response carried them.
    WorktreeList(WorktreeListDoc),
    ApiSnapshot(Value),
}

/// The `worktree list` result body. `path` is the registered Workbench
/// directory — distinct from each entry's observed `worktreePath`.
#[derive(Clone, Debug)]
pub(crate) struct WorktreeListDoc {
    pub(crate) project_id: String,
    pub(crate) directory_id: String,
    pub(crate) path: String,
    pub(crate) entries: Vec<WorktreeEntry>,
}

impl Projection {
    /// The `result` member of the machine envelope.
    pub(crate) fn result(&self) -> Value {
        match self {
            Self::Status(doc) => json!({
                "site": {
                    "reachable": true,
                    "siteId": doc.site_id,
                    "revision": doc.revision,
                },
                "counts": {
                    "projects": doc.projects,
                    "workspaces": doc.workspaces,
                    "tabs": doc.tabs,
                    "panels": doc.panels,
                    "boundTerminals": doc.bound_terminals,
                },
                "worker": {
                    "status": "unknown",
                    "reason": "not-in-workbench-snapshot",
                },
            }),
            Self::ProjectList(rows) => json!({
                "projects": rows
                    .iter()
                    .map(|row| {
                        json!({
                            "projectId": row.project_id,
                            "label": row.label,
                            "directoryCount": row.directory_count,
                            "workspaceCount": row.workspace_count,
                        })
                    })
                    .collect::<Vec<_>>(),
            }),
            Self::ProjectGet(doc) => json!({
                "project": {
                    "projectId": doc.row.project_id,
                    "label": doc.row.label,
                    "directories": doc
                        .directories
                        .iter()
                        .map(|directory| {
                            json!({
                                "directoryId": directory.directory_id,
                                "workerId": directory.worker_id,
                                "path": directory.path,
                            })
                        })
                        .collect::<Vec<_>>(),
                    "workspaces": doc
                        .workspaces
                        .iter()
                        .map(|workspace| {
                            json!({
                                "workspaceId": workspace.workspace_id,
                                "label": workspace.label,
                            })
                        })
                        .collect::<Vec<_>>(),
                },
            }),
            Self::WorkspaceList(rows) => json!({
                "workspaces": rows.iter().map(workspace_row_json).collect::<Vec<_>>(),
            }),
            Self::WorkspaceGet(doc) => {
                let mut workspace = match workspace_row_json(&doc.row) {
                    Value::Object(object) => object,
                    _ => unreachable!("workspace rows always encode as objects"),
                };
                workspace.insert(
                    "tabs".to_owned(),
                    json!(
                        doc.tabs
                            .iter()
                            .map(|tab| {
                                json!({
                                    "tabId": tab.tab_id,
                                    "label": tab.label,
                                })
                            })
                            .collect::<Vec<_>>()
                    ),
                );
                json!({ "workspace": workspace })
            }
            Self::TabList(rows) => json!({
                "tabs": rows.iter().map(tab_row_json).collect::<Vec<_>>(),
            }),
            Self::TabGet(doc) => {
                let mut tab = match tab_row_json(&doc.row) {
                    Value::Object(object) => object,
                    _ => unreachable!("tab rows always encode as objects"),
                };
                tab.insert("root".to_owned(), doc.root.clone());
                json!({ "tab": tab })
            }
            Self::PanelList(rows) => json!({
                "panels": rows
                    .iter()
                    .map(|row| {
                        json!({
                            "panelId": row.panel_id,
                            "tabId": row.tab_id,
                            "terminalContextId": row.terminal_context_id,
                            "primaryPresentationId": row.primary_presentation_id,
                            "presentationCount": row.presentation_count,
                            "primaryTerminalId": row.primary_terminal_id,
                        })
                    })
                    .collect::<Vec<_>>(),
            }),
            Self::PanelGet(doc) => json!({
                "panel": {
                    "panelId": doc.row.panel_id,
                    "tabId": doc.row.tab_id,
                    "context": context_json(&doc.context),
                    "slotPath": doc.slot_path,
                    "primaryPresentation": presentation_json(&doc.primary_presentation),
                    "childPresentations": doc
                        .child_presentations
                        .iter()
                        .map(presentation_json)
                        .collect::<Vec<_>>(),
                },
            }),
            Self::TerminalList(rows) => json!({
                "terminals": rows.iter().map(terminal_row_json).collect::<Vec<_>>(),
            }),
            Self::TerminalGet(doc) => {
                // `terminal` keeps the shared routing row and gains `launch`
                // inside it — only this document nests the retained fact.
                let mut terminal = terminal_row_json(&doc.terminal);
                if let Value::Object(members) = &mut terminal {
                    members.insert("launch".to_owned(), launch_fact_json(&doc.launch));
                }
                json!({ "terminal": terminal })
            }
            Self::TerminalRead(doc) => json!({
                "terminal": terminal_row_json(&doc.terminal),
                "frame": {
                    "revision": doc.frame.revision.to_string(),
                    "generation": doc.frame.generation.to_string(),
                    "sequence": doc.frame.sequence.to_string(),
                    "alternateScreen": doc.frame.alternate_screen,
                    "geometry": {
                        "columns": doc.frame.columns,
                        "rows": doc.frame.rows,
                    },
                    "cursor": {
                        "row": doc.frame.cursor_row,
                        "column": doc.frame.cursor_column,
                        "visible": doc.frame.cursor_visible,
                    },
                },
                "content": {
                    "rows": doc
                        .rows
                        .iter()
                        .map(|row| {
                            json!({
                                "index": row.index,
                                "text": row.text,
                                "continuation": {
                                    "fromPrevious": row.from_previous,
                                    "intoNext": row.into_next,
                                },
                            })
                        })
                        .collect::<Vec<_>>(),
                    "rowCount": doc.row_count,
                    "truncated": doc.omitted_rows > 0,
                    "omittedRows": doc.omitted_rows,
                    "limits": {
                        "rows": MAXIMUM_FRAME_ROWS,
                        "textBytes": READ_TEXT_BYTES,
                    },
                },
            }),
            Self::TerminalHistory(doc) => json!({
                "terminal": terminal_row_json(&doc.terminal),
                "history": {
                    "revision": doc.revision.to_string(),
                    "generation": doc.generation.to_string(),
                    "alternateScreen": doc.alternate_screen,
                    "columns": doc.columns,
                    "screenRows": doc.screen_rows,
                    "scrollbackRows": doc.scrollback_rows.to_string(),
                    "totalRows": doc.total_rows.to_string(),
                    "startRow": doc.start_row.to_string(),
                    "requestedRows": doc.requested_rows,
                    "windowRows": doc.window_rows,
                },
                "content": {
                    "rows": doc
                        .rows
                        .iter()
                        .map(|row| {
                            json!({
                                "index": row.index.to_string(),
                                "text": row.text,
                                "continuation": {
                                    "fromPrevious": row.from_previous,
                                    "intoNext": row.into_next,
                                },
                            })
                        })
                        .collect::<Vec<_>>(),
                    "rowCount": doc.rows.len(),
                    "truncated": doc.omitted_rows > 0,
                    "omittedRows": doc.omitted_rows,
                    "limits": {
                        "rows": MAXIMUM_HISTORY_ROWS,
                        "textBytes": READ_TEXT_BYTES,
                    },
                },
            }),
            Self::TerminalWaitOutput(doc) => json!({
                "terminal": terminal_row_json(&doc.terminal),
                "matched": true,
                "match": {
                    "excerpt": doc.matched.excerpt,
                    "excerptTruncated": doc.matched.excerpt_truncated,
                    "excerptOffset": doc.matched.excerpt_offset.to_string(),
                    "matchOffset": doc.matched.match_offset.to_string(),
                    "rowStart": doc.matched.row_start.to_string(),
                    "rowEnd": doc.matched.row_end.to_string(),
                    "revision": doc.revision.to_string(),
                    "generation": doc.generation.to_string(),
                },
                "observed": {
                    "attachmentId": doc.attachment_id,
                    "hostEpoch": doc.host_epoch,
                    "completedObservations": doc.observations,
                    "windowStart": doc.window_start.to_string(),
                    "windowRows": doc.window_rows.to_string(),
                    "windowTruncated": doc.window_truncated,
                    "totalRows": doc.total_rows.to_string(),
                    "requestedRows": doc.requested_rows,
                    "timeoutMs": doc.timeout_ms,
                    "elapsedMs": doc.elapsed_ms,
                },
            }),
            Self::WorktreeList(doc) => json!({
                "projectId": doc.project_id,
                "directoryId": doc.directory_id,
                "path": doc.path,
                "worktrees": doc
                    .entries
                    .iter()
                    .map(worktree_entry_json)
                    .collect::<Vec<_>>(),
            }),
            Self::ApiSnapshot(document) => json!({ "snapshot": document }),
        }
    }
}

/// The wire-faithful entry encoding: `worktreePath`/`worktreePathBytesHex`
/// and `branch`/`branchBytesHex` stay semantic-exclusive, matching the
/// workbench contract byte for byte.
fn worktree_entry_json(entry: &WorktreeEntry) -> Value {
    let mut object = Map::new();
    match &entry.path {
        WorktreeEntryText::Utf8(path) => {
            object.insert("worktreePath".to_owned(), json!(path));
        }
        WorktreeEntryText::Bytes(bytes) => {
            object.insert("worktreePathBytesHex".to_owned(), json!(hex_encode(bytes)));
        }
    }
    object.insert("head".to_owned(), json!(entry.head));
    match &entry.branch {
        Some(WorktreeEntryText::Utf8(branch)) => {
            object.insert("branch".to_owned(), json!(branch));
        }
        Some(WorktreeEntryText::Bytes(bytes)) => {
            object.insert("branchBytesHex".to_owned(), json!(hex_encode(bytes)));
        }
        None => {
            object.insert("branch".to_owned(), Value::Null);
        }
    }
    object.insert("bare".to_owned(), json!(entry.bare));
    object.insert("detached".to_owned(), json!(entry.detached));
    object.insert("locked".to_owned(), json!(entry.locked));
    object.insert("prunable".to_owned(), json!(entry.prunable));
    Value::Object(object)
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble"));
        encoded.push(char::from_digit(u32::from(byte & 0x0f), 16).expect("nibble"));
    }
    encoded
}

fn workspace_row_json(row: &WorkspaceRow) -> Value {
    json!({
        "workspaceId": row.workspace_id,
        "label": row.label,
        "projectId": row.project_id,
        "primaryDirectoryId": row.primary_directory_id,
        "terminalContextId": row.terminal_context_id,
    })
}

fn context_json(context: &ContextRef) -> Value {
    let mut object = Map::from_iter([
        (
            "kind".to_owned(),
            json!(if context.standalone {
                "standalone"
            } else {
                "workspace"
            }),
        ),
        (
            "terminalContextId".to_owned(),
            json!(context.terminal_context_id),
        ),
    ]);
    if let Some(workspace_id) = &context.workspace_id {
        object.insert("workspaceId".to_owned(), json!(workspace_id));
    }
    Value::Object(object)
}

fn tab_row_json(row: &TabRow) -> Value {
    json!({
        "tabId": row.tab_id,
        "label": row.label,
        "pinned": row.pinned,
        "groupId": row.group_id,
        "context": context_json(&row.context),
    })
}

fn presentation_json(row: &PresentationRow) -> Value {
    json!({
        "presentationId": row.presentation_id,
        "label": row.label,
        "binding": match (&row.terminal_id, &row.worker_id) {
            (Some(terminal_id), Some(worker_id)) => json!({
                "terminalId": terminal_id,
                "workerId": worker_id,
            }),
            _ => Value::Null,
        },
    })
}

/// The retained launch fact as its machine-document value — the exact
/// protocol shape: kebab-case `state`, a decimal-or-null `workerGeneration`,
/// and the retained `createOutcome`/`geometry` encoders verbatim. The fact is
/// validated at construction, so this projection cannot fail; machine and
/// text output share it.
pub(crate) fn launch_fact_json(launch: &TerminalLaunchFact) -> Value {
    json!({
        "state": launch.state(),
        "workerGeneration": launch
            .worker_generation()
            .map(|generation| generation.get().to_string()),
        "createOutcome": launch.create_outcome(),
        "geometry": launch.geometry(),
    })
}

fn terminal_row_json(row: &TerminalRow) -> Value {
    json!({
        "terminalId": row.terminal_id,
        "workerId": row.worker_id,
        "boundTo": {
            "presentationId": row.presentation_id,
            "panelId": row.panel_id,
            "tabId": row.tab_id,
            "terminalContextId": row.terminal_context_id,
        },
    })
}

/// context id → owning workspace id for every workspace-bound context.
struct ContextIndex<'a> {
    standalone_id: &'a str,
    owners: HashMap<&'a str, &'a str>,
}

impl<'a> ContextIndex<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let mut owners = HashMap::new();
        for project in &snapshot.navigation.projects {
            for workspace in &project.workspaces {
                owners.insert(
                    workspace.terminal_context_id.as_str(),
                    workspace.workspace_id.as_str(),
                );
            }
        }
        Self {
            standalone_id: snapshot.navigation.standalone_terminal_context_id.as_str(),
            owners,
        }
    }

    fn reference(&self, context: &'a TerminalContext) -> ContextRef {
        let workspace_id = self.owners.get(context.terminal_context_id.as_str());
        ContextRef {
            standalone: context.terminal_context_id.as_str() == self.standalone_id,
            terminal_context_id: context.terminal_context_id.as_str().to_owned(),
            workspace_id: workspace_id.map(|id| (*id).to_owned()),
        }
    }
}

/// A typed target hits identity first; only a non-ID token becomes an exact
/// case-sensitive label search.
fn resolve<'a, Item>(
    mut items: impl Iterator<Item = (&'a str, &'a str, Item)>,
    target: &Target,
) -> Result<Item, Vec<&'a str>> {
    match target {
        Target::Id(identity) => items
            .find(|(id, _label, _item)| *id == identity.as_str())
            .map(|(_id, _label, item)| item)
            .ok_or_else(Vec::new),
        Target::Label(label) => {
            let mut matches = items
                .filter(|(_id, item_label, _item)| *item_label == label.as_str())
                .collect::<Vec<_>>();
            match matches.len() {
                0 => Err(Vec::new()),
                1 => Ok(matches.pop().expect("len checked").2),
                _ => Err(matches.into_iter().map(|(id, _label, _item)| id).collect()),
            }
        }
    }
}

pub(crate) fn not_found(resource: Resource, target: &Target) -> Failure {
    Failure::new(FailureKind::NotFound).details(target_details(resource, target.text()))
}

fn ambiguous(resource: Resource, target: &Target, mut candidates: Vec<String>) -> Failure {
    let total = candidates.len();
    candidates.truncate(8);
    Failure::new(FailureKind::Ambiguous).details(Map::from_iter([
        ("resource".to_owned(), json!(resource.as_str())),
        ("target".to_owned(), json!(target.text())),
        ("candidates".to_owned(), json!(candidates)),
        ("totalCount".to_owned(), json!(total)),
    ]))
}

pub(crate) fn resolve_or_fail<'a, Item>(
    items: impl Iterator<Item = (&'a str, &'a str, Item)>,
    target: &Target,
    resource: Resource,
) -> Result<Item, Failure> {
    match resolve(items, target) {
        Ok(item) => Ok(item),
        Err(candidates) if candidates.is_empty() => Err(not_found(resource, target)),
        Err(candidates) => Err(ambiguous(
            resource,
            target,
            candidates.into_iter().map(str::to_owned).collect(),
        )),
    }
}

/// Run one snapshot-derived command over a validated snapshot.
pub(crate) fn project(command: &QueryCommand, snapshot: &Snapshot) -> Result<Projection, Failure> {
    let index = ContextIndex::new(snapshot);
    match command {
        QueryCommand::Status => Ok(Projection::Status(status(snapshot))),
        QueryCommand::ProjectList => Ok(Projection::ProjectList(
            snapshot
                .navigation
                .projects
                .iter()
                .map(project_row)
                .collect(),
        )),
        QueryCommand::ProjectGet(target) => {
            let project =
                resolve_or_fail(
                    snapshot.navigation.projects.iter().map(|project| {
                        (project.project_id.as_str(), project.label.as_str(), project)
                    }),
                    target,
                    Resource::Project,
                )?;
            Ok(Projection::ProjectGet(project_doc(project)))
        }
        QueryCommand::WorkspaceList { project } => {
            let projects: Vec<&Project> = match project {
                Some(target) => vec![resolve_or_fail(
                    snapshot.navigation.projects.iter().map(|project| {
                        (project.project_id.as_str(), project.label.as_str(), project)
                    }),
                    target,
                    Resource::Project,
                )?],
                None => snapshot.navigation.projects.iter().collect(),
            };
            Ok(Projection::WorkspaceList(
                projects
                    .iter()
                    .flat_map(|project| {
                        project
                            .workspaces
                            .iter()
                            .map(|workspace| workspace_row(project, workspace))
                    })
                    .collect(),
            ))
        }
        QueryCommand::WorkspaceGet(target) => {
            let (project, workspace) = resolve_or_fail(
                snapshot.navigation.projects.iter().flat_map(|project| {
                    project.workspaces.iter().map(move |workspace| {
                        (
                            workspace.workspace_id.as_str(),
                            workspace.label.as_str(),
                            (project, workspace),
                        )
                    })
                }),
                target,
                Resource::Workspace,
            )?;
            Ok(Projection::WorkspaceGet(workspace_doc(
                snapshot, project, workspace,
            )?))
        }
        QueryCommand::TabList {
            workspace,
            standalone,
        } => {
            let contexts: Vec<&TerminalContext> = if *standalone {
                snapshot
                    .terminal_contexts
                    .iter()
                    .filter(|context| context.terminal_context_id.as_str() == index.standalone_id)
                    .collect()
            } else if let Some(target) = workspace {
                let (_project, workspace) = resolve_or_fail(
                    snapshot.navigation.projects.iter().flat_map(|project| {
                        project.workspaces.iter().map(move |workspace| {
                            (
                                workspace.workspace_id.as_str(),
                                workspace.label.as_str(),
                                (project, workspace),
                            )
                        })
                    }),
                    target,
                    Resource::Workspace,
                )?;
                snapshot
                    .terminal_contexts
                    .iter()
                    .filter(|context| context.terminal_context_id == workspace.terminal_context_id)
                    .collect()
            } else {
                snapshot.terminal_contexts.iter().collect()
            };
            Ok(Projection::TabList(
                contexts
                    .iter()
                    .flat_map(|context| {
                        context
                            .tabs
                            .iter()
                            .map(|tab| tab_row(tab, index.reference(context)))
                    })
                    .collect(),
            ))
        }
        QueryCommand::TabGet(target) => {
            let (context, tab) = resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context
                        .tabs
                        .iter()
                        .map(move |tab| (tab.tab_id.as_str(), tab.label.as_str(), (context, tab)))
                }),
                target,
                Resource::Tab,
            )?;
            Ok(Projection::TabGet(TabDoc {
                row: tab_row(tab, index.reference(context)),
                root: serde_json::to_value(&tab.root)
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            }))
        }
        QueryCommand::PanelList { tab } => {
            let tabs: Vec<(&TerminalContext, &Tab)> = match tab {
                Some(target) => {
                    let resolved = resolve_or_fail(
                        snapshot.terminal_contexts.iter().flat_map(|context| {
                            context.tabs.iter().map(move |tab| {
                                (tab.tab_id.as_str(), tab.label.as_str(), (context, tab))
                            })
                        }),
                        target,
                        Resource::Tab,
                    )?;
                    vec![resolved]
                }
                None => snapshot
                    .terminal_contexts
                    .iter()
                    .flat_map(|context| context.tabs.iter().map(move |tab| (context, tab)))
                    .collect(),
            };
            let mut rows = Vec::new();
            for (context, tab) in tabs {
                let mut path = Vec::new();
                collect_panels(&tab.root, context, tab, &mut path, &mut |row, _path| {
                    rows.push(row);
                });
            }
            Ok(Projection::PanelList(rows))
        }
        QueryCommand::PanelGet(target) => {
            let identity = match target {
                Target::Id(identity) => identity.as_str(),
                Target::Label(_) => {
                    return Err(Failure::new(FailureKind::InternalFailure));
                }
            };
            for context in &snapshot.terminal_contexts {
                for tab in &context.tabs {
                    let mut path = Vec::new();
                    let mut found = None;
                    collect_panels(&tab.root, context, tab, &mut path, &mut |row, slot_path| {
                        if row.panel_id == identity {
                            found = Some((row, slot_path.to_vec()));
                        }
                    });
                    if let Some((row, slot_path)) = found {
                        let Some((primary, children)) = panel_presentations(&tab.root, identity)
                        else {
                            return Err(Failure::new(FailureKind::InternalFailure));
                        };
                        return Ok(Projection::PanelGet(PanelDoc {
                            row,
                            context: index.reference(context),
                            slot_path,
                            primary_presentation: primary,
                            child_presentations: children,
                        }));
                    }
                }
            }
            Err(not_found(Resource::Panel, target))
        }
        QueryCommand::TerminalList => Ok(Projection::TerminalList(terminal_rows(snapshot))),
        QueryCommand::ApiSnapshot => Ok(Projection::ApiSnapshot(
            serde_json::to_value(WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
        )),
        // `terminal get` answers the Server's own launch-read join and
        // `terminal read`/`terminal wait-output` project live viewer
        // observations, not snapshot data alone; their answers are built by
        // the query layer. `worktree list` issues its own Workbench request
        // after the snapshot resolves the project; `api schema`/`config
        // check` never reach a snapshot projection.
        QueryCommand::TerminalGet(_)
        | QueryCommand::TerminalRead { .. }
        | QueryCommand::TerminalWaitOutput { .. }
        | QueryCommand::WorktreeList { .. }
        | QueryCommand::ApiSchema { .. }
        | QueryCommand::ConfigCheck => Err(Failure::new(FailureKind::InternalFailure)),
    }
}

fn status(snapshot: &Snapshot) -> StatusDoc {
    let mut tabs = 0_usize;
    let mut panels = 0_usize;
    let mut bound_terminals = 0_usize;
    for context in &snapshot.terminal_contexts {
        tabs += context.tabs.len();
        for tab in &context.tabs {
            let mut path = Vec::new();
            collect_panel_nodes(&tab.root, &mut path, &mut |node| {
                panels += 1;
                if let PanelNode::Panel {
                    primary_presentation,
                    child_presentations,
                    ..
                } = node
                {
                    bound_terminals += primary_presentation
                        .binding
                        .iter()
                        .chain(child_presentations.iter().flat_map(|p| p.binding.iter()))
                        .count();
                }
            });
        }
    }
    StatusDoc {
        site_id: snapshot.site_id.as_str().to_owned(),
        revision: snapshot.revision.to_string(),
        projects: snapshot.navigation.projects.len(),
        workspaces: snapshot
            .navigation
            .projects
            .iter()
            .map(|project| project.workspaces.len())
            .sum(),
        tabs,
        panels,
        bound_terminals,
    }
}

fn project_row(project: &Project) -> ProjectRow {
    ProjectRow {
        project_id: project.project_id.as_str().to_owned(),
        label: project.label.clone(),
        directory_count: project.directories.len(),
        workspace_count: project.workspaces.len(),
    }
}

fn project_doc(project: &Project) -> ProjectDoc {
    ProjectDoc {
        row: project_row(project),
        directories: project
            .directories
            .iter()
            .map(|directory| DirectoryRow {
                directory_id: directory.directory_id.as_str().to_owned(),
                worker_id: directory.worker_id.as_str().to_owned(),
                path: directory.path.as_str().to_owned(),
            })
            .collect(),
        workspaces: project
            .workspaces
            .iter()
            .map(|workspace| WorkspaceRef {
                workspace_id: workspace.workspace_id.as_str().to_owned(),
                label: workspace.label.clone(),
            })
            .collect(),
    }
}

fn workspace_row(project: &Project, workspace: &Workspace) -> WorkspaceRow {
    WorkspaceRow {
        workspace_id: workspace.workspace_id.as_str().to_owned(),
        label: workspace.label.clone(),
        project_id: project.project_id.as_str().to_owned(),
        primary_directory_id: workspace
            .primary_directory_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        terminal_context_id: workspace.terminal_context_id.as_str().to_owned(),
    }
}

fn workspace_doc(
    snapshot: &Snapshot,
    project: &Project,
    workspace: &Workspace,
) -> Result<WorkspaceDoc, Failure> {
    let context_id = workspace.terminal_context_id.as_str();
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id.as_str() == context_id)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    Ok(WorkspaceDoc {
        row: workspace_row(project, workspace),
        tabs: context
            .tabs
            .iter()
            .map(|tab| TabRef {
                tab_id: tab.tab_id.as_str().to_owned(),
                label: tab.label.clone(),
            })
            .collect(),
    })
}

fn tab_row(tab: &Tab, context: ContextRef) -> TabRow {
    TabRow {
        tab_id: tab.tab_id.as_str().to_owned(),
        label: tab.label.clone(),
        pinned: tab.pinned,
        group_id: tab.group_id.as_ref().map(|id| id.as_str().to_owned()),
        context,
    }
}

fn presentation_row(presentation: &Presentation) -> PresentationRow {
    PresentationRow {
        presentation_id: presentation.presentation_id.as_str().to_owned(),
        label: presentation.label.clone(),
        terminal_id: presentation
            .binding
            .as_ref()
            .map(|binding| binding.terminal_id.as_str().to_owned()),
        worker_id: presentation
            .binding
            .as_ref()
            .map(|binding| binding.worker_id.as_str().to_owned()),
    }
}

fn collect_panel_nodes<'a>(
    node: &'a PanelNode,
    path: &mut Vec<String>,
    visit: &mut impl FnMut(&'a PanelNode),
) {
    match node {
        PanelNode::Panel { .. } => visit(node),
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                let PanelSlot { node, slot_id } = slot;
                path.push(slot_id.as_str().to_owned());
                collect_panel_nodes(node, path, visit);
                path.pop();
            }
        }
    }
}

fn collect_panels<'a>(
    node: &'a PanelNode,
    context: &'a TerminalContext,
    tab: &'a Tab,
    path: &mut Vec<String>,
    visit: &mut impl FnMut(PanelRow, &[String]),
) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
        } => visit(
            PanelRow {
                panel_id: panel_id.as_str().to_owned(),
                tab_id: tab.tab_id.as_str().to_owned(),
                terminal_context_id: context.terminal_context_id.as_str().to_owned(),
                primary_presentation_id: primary_presentation.presentation_id.as_str().to_owned(),
                presentation_count: 1 + child_presentations.len(),
                primary_terminal_id: primary_presentation
                    .binding
                    .as_ref()
                    .map(|binding| binding.terminal_id.as_str().to_owned()),
            },
            path,
        ),
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                path.push(slot.slot_id.as_str().to_owned());
                collect_panels(&slot.node, context, tab, path, visit);
                path.pop();
            }
        }
    }
}

/// Find the Panel node by id and extract its presentation rows.
fn panel_presentations(
    node: &PanelNode,
    panel_id: &str,
) -> Option<(PresentationRow, Vec<PresentationRow>)> {
    match node {
        PanelNode::Panel {
            panel_id: id,
            primary_presentation,
            child_presentations,
        } if id.as_str() == panel_id => Some((
            presentation_row(primary_presentation),
            child_presentations.iter().map(presentation_row).collect(),
        )),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| panel_presentations(&slot.node, panel_id)),
        PanelNode::Panel { .. } => None,
    }
}

/// Every bound terminal exactly once, in canonical traversal order. Validation
/// already guarantees bound terminal ids never repeat across the snapshot.
fn terminal_rows(snapshot: &Snapshot) -> Vec<TerminalRow> {
    let mut rows = Vec::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            collect_terminal_rows(&tab.root, context, tab, &mut rows);
        }
    }
    rows
}

fn collect_terminal_rows(
    node: &PanelNode,
    context: &TerminalContext,
    tab: &Tab,
    rows: &mut Vec<TerminalRow>,
) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
        } => {
            for presentation in
                std::iter::once(primary_presentation).chain(child_presentations.iter())
            {
                if let Some(binding) = &presentation.binding {
                    rows.push(TerminalRow {
                        terminal_id: binding.terminal_id.as_str().to_owned(),
                        worker_id: binding.worker_id.as_str().to_owned(),
                        presentation_id: presentation.presentation_id.as_str().to_owned(),
                        panel_id: panel_id.as_str().to_owned(),
                        tab_id: tab.tab_id.as_str().to_owned(),
                        terminal_context_id: context.terminal_context_id.as_str().to_owned(),
                    });
                }
            }
        }
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                collect_terminal_rows(&slot.node, context, tab, rows);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Resolver verdicts over real validated snapshots: bounded ambiguity with
    //! the true total, traversal-order candidates, per-resource ambiguity, and
    //! filter-target hit/miss/ambiguity for every filter arm.

    use super::*;
    use orchbee_workbench_protocol::parse_snapshot;

    fn hex(n: u8) -> String {
        format!("{n:032x}")
    }

    const WORKER: &str = "6f7263686265652d6c6f63616c2d7631";

    /// Deliberately unsorted document order: candidate ordering must follow
    /// canonical traversal order, never id sort.
    const DUP_TABS: [u8; 9] = [0x29, 0x25, 0x27, 0x22, 0x28, 0x24, 0x26, 0x23, 0x21];

    fn presentation(id: u8, terminal: Option<u8>) -> Value {
        json!({
            "presentationId": hex(id),
            "label": "p",
            "binding": terminal.map(|terminal| json!({
                "terminalId": hex(terminal),
                "workerId": WORKER,
            })),
        })
    }

    fn panel(id: u8, primary: u8, children: &[u8], terminal: Option<u8>) -> Value {
        json!({
            "kind": "panel",
            "panelId": hex(id),
            "primaryPresentation": presentation(primary, terminal),
            "childPresentations": children
                .iter()
                .map(|&child| presentation(child, None))
                .collect::<Vec<_>>(),
        })
    }

    fn tab(id: u8, label: &str, root: Value) -> Value {
        json!({
            "tabId": hex(id),
            "label": label,
            "pinned": false,
            "groupId": null,
            "root": root,
        })
    }

    /// Two same-label projects (`alpha`), each owning one same-label
    /// workspace (`ws`) on its own context; nine same-label `dup` tabs in
    /// scrambled id order under the standalone context; one `main` tab under
    /// each workspace context (a split root under the first).
    fn snapshot() -> Snapshot {
        let document = json!({
            "kind": "workbench.snapshot",
            "protocolVersion": 1,
            "revision": "9",
            "siteId": hex(0xaa),
            "navigation": {
                "projects": [
                    {
                        "projectId": hex(0x10),
                        "label": "alpha",
                        "directories": [{
                            "directoryId": hex(0x30),
                            "workerId": WORKER,
                            "path": "/a",
                        }],
                        "workspaces": [{
                            "workspaceId": hex(0x40),
                            "label": "ws",
                            "primaryDirectoryId": hex(0x30),
                            "terminalContextId": hex(0x50),
                        }],
                    },
                    {
                        "projectId": hex(0x11),
                        "label": "alpha",
                        "directories": [{
                            "directoryId": hex(0x31),
                            "workerId": WORKER,
                            "path": "/b",
                        }],
                        "workspaces": [{
                            "workspaceId": hex(0x41),
                            "label": "ws",
                            "primaryDirectoryId": hex(0x31),
                            "terminalContextId": hex(0x51),
                        }],
                    },
                ],
                "standaloneTerminalContextId": hex(0x01),
            },
            "terminalContexts": [
                {
                    "terminalContextId": hex(0x01),
                    "groups": [],
                    "tabs": DUP_TABS
                        .iter()
                        .map(|&n| tab(n, "dup", panel(0x80 + n, 0xa0 + n, &[], None)))
                        .collect::<Vec<_>>(),
                },
                {
                    "terminalContextId": hex(0x50),
                    "groups": [],
                    "tabs": [tab(0x52, "main", json!({
                        "kind": "split",
                        "splitId": hex(0x53),
                        "orientation": "horizontal",
                        "children": [
                            {"slotId": hex(0x54), "node": panel(0x55, 0x56, &[], None)},
                            {"slotId": hex(0x57), "node": panel(0x58, 0x59, &[0x5b], Some(0x5a))},
                        ],
                    }))],
                },
                {
                    "terminalContextId": hex(0x51),
                    "groups": [],
                    "tabs": [tab(0x62, "main", panel(0x63, 0x64, &[], None))],
                },
            ],
        });
        parse_snapshot(&serde_json::to_vec(&document).expect("fixture encodes"))
            .expect("fixture snapshot validates")
    }

    fn failure(command: &QueryCommand) -> Failure {
        project(command, &snapshot()).expect_err("resolver failure expected")
    }

    fn details(failure: &Failure) -> &Map<String, Value> {
        failure.details.as_ref().expect("typed failure details")
    }

    fn candidates(failure: &Failure) -> Vec<String> {
        details(failure)["candidates"]
            .as_array()
            .expect("candidate list")
            .iter()
            .map(|id| id.as_str().expect("typed candidate id").to_owned())
            .collect()
    }

    #[test]
    fn ambiguity_truncates_candidates_but_keeps_the_true_total() {
        let failure = failure(&QueryCommand::TabGet(Target::Label("dup".into())));
        assert_eq!(failure.kind, FailureKind::Ambiguous);
        let details = details(&failure);
        assert_eq!(details["resource"], json!("tab"));
        assert_eq!(details["target"], json!("dup"));
        assert_eq!(details["totalCount"], json!(DUP_TABS.len()));
        // At most the first eight candidate typed ids, in canonical traversal
        // order — context order then tab order — not sorted identity order.
        assert_eq!(
            candidates(&failure),
            DUP_TABS[..8].iter().map(|&n| hex(n)).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn label_lookup_is_ambiguous_per_resource() {
        for (command, resource, expected) in [
            (
                QueryCommand::ProjectGet(Target::Label("alpha".into())),
                "project",
                vec![hex(0x10), hex(0x11)],
            ),
            (
                QueryCommand::WorkspaceGet(Target::Label("ws".into())),
                "workspace",
                vec![hex(0x40), hex(0x41)],
            ),
        ] {
            let failure = failure(&command);
            assert_eq!(failure.kind, FailureKind::Ambiguous, "{resource}");
            let details = details(&failure);
            assert_eq!(details["resource"], json!(resource));
            assert_eq!(details["totalCount"], json!(expected.len()));
            assert_eq!(candidates(&failure), expected, "{resource}");
        }
    }

    #[test]
    fn unmatched_targets_are_not_found_without_candidates() {
        let missing = hex(0x7f);
        for (command, resource, target) in [
            (
                QueryCommand::TabGet(Target::Label("ghost".into())),
                "tab",
                "ghost".to_owned(),
            ),
            (
                QueryCommand::TabGet(Target::Id(missing.clone())),
                "tab",
                missing.clone(),
            ),
            (
                QueryCommand::WorkspaceGet(Target::Label("ghost".into())),
                "workspace",
                "ghost".to_owned(),
            ),
        ] {
            let failure = failure(&command);
            assert_eq!(failure.kind, FailureKind::NotFound, "{resource}");
            let details = details(&failure);
            assert_eq!(details["resource"], json!(resource));
            assert_eq!(details["target"], json!(target));
            assert!(details.get("candidates").is_none(), "{resource}");
        }
    }

    #[test]
    fn project_filter_hit_miss_and_ambiguity() {
        // Hit: only the resolved project's workspaces survive the filter.
        let projection = project(
            &QueryCommand::WorkspaceList {
                project: Some(Target::Id(hex(0x10))),
            },
            &snapshot(),
        )
        .expect("resolved project filter");
        let Projection::WorkspaceList(rows) = projection else {
            panic!("workspace list projection");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].workspace_id, hex(0x40));
        assert_eq!(rows[0].project_id, hex(0x10));
        assert_eq!(rows[0].terminal_context_id, hex(0x50));

        // Ambiguous filter target: the list itself is never built.
        let ambiguous = failure(&QueryCommand::WorkspaceList {
            project: Some(Target::Label("alpha".into())),
        });
        assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
        assert_eq!(details(&ambiguous)["resource"], json!("project"));
        assert_eq!(candidates(&ambiguous), vec![hex(0x10), hex(0x11)]);

        // Miss: a typed not-found, never an empty list.
        let missed = failure(&QueryCommand::WorkspaceList {
            project: Some(Target::Label("ghost".into())),
        });
        assert_eq!(missed.kind, FailureKind::NotFound);
        assert_eq!(details(&missed)["resource"], json!("project"));
        assert_eq!(details(&missed)["target"], json!("ghost"));
    }

    #[test]
    fn workspace_filter_hit_miss_and_ambiguity() {
        // Hit: tabs from exactly the resolved workspace's own context, each
        // carrying the workspace context reference.
        let projection = project(
            &QueryCommand::TabList {
                workspace: Some(Target::Id(hex(0x41))),
                standalone: false,
            },
            &snapshot(),
        )
        .expect("resolved workspace filter");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tab_id, hex(0x62));
        assert!(!rows[0].context.standalone);
        assert_eq!(rows[0].context.terminal_context_id, hex(0x51));
        assert_eq!(rows[0].context.workspace_id, Some(hex(0x41)));

        // Ambiguous: two workspaces share the label across projects.
        let ambiguous = failure(&QueryCommand::TabList {
            workspace: Some(Target::Label("ws".into())),
            standalone: false,
        });
        assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
        assert_eq!(details(&ambiguous)["resource"], json!("workspace"));
        assert_eq!(candidates(&ambiguous), vec![hex(0x40), hex(0x41)]);

        // Miss.
        let missed = failure(&QueryCommand::TabList {
            workspace: Some(Target::Label("ghost".into())),
            standalone: false,
        });
        assert_eq!(missed.kind, FailureKind::NotFound);
        assert_eq!(details(&missed)["resource"], json!("workspace"));
    }

    #[test]
    fn tab_filter_hit_miss_and_ambiguity() {
        // Hit: panels under exactly the resolved tab, in traversal order.
        let projection = project(
            &QueryCommand::PanelList {
                tab: Some(Target::Id(hex(0x52))),
            },
            &snapshot(),
        )
        .expect("resolved tab filter");
        let Projection::PanelList(rows) = projection else {
            panic!("panel list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.panel_id.clone())
                .collect::<Vec<_>>(),
            vec![hex(0x55), hex(0x58)],
        );
        assert!(rows.iter().all(|row| row.tab_id == hex(0x52)));
        assert!(rows.iter().all(|row| row.terminal_context_id == hex(0x50)));

        // Ambiguous: "main" names a tab in each workspace context.
        let ambiguous = failure(&QueryCommand::PanelList {
            tab: Some(Target::Label("main".into())),
        });
        assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
        assert_eq!(details(&ambiguous)["resource"], json!("tab"));
        assert_eq!(candidates(&ambiguous), vec![hex(0x52), hex(0x62)]);

        // Miss.
        let missed = failure(&QueryCommand::PanelList {
            tab: Some(Target::Label("ghost".into())),
        });
        assert_eq!(missed.kind, FailureKind::NotFound);
        assert_eq!(details(&missed)["resource"], json!("tab"));
    }

    #[test]
    fn unfiltered_lists_follow_canonical_traversal_order() {
        // Workspaces flatten across projects in project/workspace order.
        let projection = project(&QueryCommand::WorkspaceList { project: None }, &snapshot())
            .expect("workspace list");
        let Projection::WorkspaceList(rows) = projection else {
            panic!("workspace list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.workspace_id.clone())
                .collect::<Vec<_>>(),
            vec![hex(0x40), hex(0x41)],
        );

        // Tabs concatenate contexts in context order, then document order:
        // the scrambled standalone ids lead, then each workspace's tab.
        let projection = project(
            &QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
            &snapshot(),
        )
        .expect("tab list");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(rows.len(), DUP_TABS.len() + 2);
        assert_eq!(
            rows.iter()
                .map(|row| row.tab_id.clone())
                .collect::<Vec<_>>(),
            DUP_TABS
                .iter()
                .chain([0x52, 0x62].iter())
                .map(|&n| hex(n))
                .collect::<Vec<_>>(),
        );
        // Context references pin the owning workspace only where one exists.
        assert!(
            rows[..9]
                .iter()
                .all(|row| row.context.standalone && row.context.workspace_id.is_none())
        );
        assert_eq!(rows[9].context.workspace_id, Some(hex(0x40)));
        assert_eq!(rows[10].context.workspace_id, Some(hex(0x41)));

        // `--standalone` selects exactly the standalone context's tabs.
        let projection = project(
            &QueryCommand::TabList {
                workspace: None,
                standalone: true,
            },
            &snapshot(),
        )
        .expect("standalone tab list");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(rows.len(), DUP_TABS.len());
        assert!(rows.iter().all(|row| row.context.standalone));
    }

    /// `terminal read` answer bounds over decoder-legal frame states: every
    /// constructed frame honors the wire invariants the OBTF decoder enforces
    /// (row index < `rows`, `cells == columns`, adjacent wrap-flag agreement),
    /// so each case is a state a real record can deliver. The same shapes
    /// cross the wire in the scripted-peer witnesses in `tests/cli_query.rs`.
    #[cfg(unix)]
    mod terminal_read {
        use super::super::{Projection, TerminalRow, terminal_read_doc};
        use crate::cli::{output::render_text, present};
        use crate::invocation::{QueryCommand, Target};
        use crate::localization::Locale;
        use orchbee_terminal_protocol::{
            AttachmentId, HostEpoch, ProtocolErrorCode, TerminalFrameRoute, TerminalInteraction,
            TerminalPixelGeometry, TerminalTextCell, TerminalTextCursor, TerminalTextFrame,
            TerminalTextRow, TerminalTextStyle, decode_terminal_text_frame,
        };
        use std::num::NonZeroU64;
        use std::sync::Arc;

        const COLUMNS: u16 = 64;

        fn hex(n: u8) -> String {
            format!("{n:032x}")
        }

        fn terminal() -> TerminalRow {
            TerminalRow {
                terminal_id: hex(0x01),
                worker_id: "6f7263686265652d6c6f63616c2d7631".to_owned(),
                presentation_id: hex(0x20),
                panel_id: hex(0x21),
                tab_id: hex(0x22),
                terminal_context_id: hex(0x50),
            }
        }

        fn cell(text: &Arc<str>, width: u8) -> TerminalTextCell {
            TerminalTextCell {
                text: text.clone(),
                style: TerminalTextStyle {
                    foreground: 0,
                    background: 0,
                    underline_color: 0,
                    flags: 0,
                    underline: 0,
                },
                width,
            }
        }

        fn frame(rows: Vec<TerminalTextRow>) -> TerminalTextFrame {
            frame_with_columns(rows, COLUMNS)
        }

        fn frame_with_columns(rows: Vec<TerminalTextRow>, columns: u16) -> TerminalTextFrame {
            TerminalTextFrame {
                route: TerminalFrameRoute {
                    attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
                    host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
                    sequence: NonZeroU64::new(1).expect("sequence"),
                    generation: NonZeroU64::new(1).expect("generation"),
                    revision: 1,
                    base_revision: None,
                },
                geometry: TerminalPixelGeometry {
                    cell_pixel_height: 1,
                    cell_pixel_width: 8,
                    columns,
                    rows: u16::try_from(rows.len()).expect("row count fits u16"),
                },
                cursor: TerminalTextCursor {
                    column: 0,
                    row: 0,
                    visible: true,
                },
                alternate_screen: false,
                interaction: TerminalInteraction::default(),
                foreground: 0,
                background: 0,
                theme_revision: 1,
                scrollback_rows: 0,
                rows,
            }
        }

        /// Minimal OBTF snapshot serializer for real-decoder round-trip
        /// checks: cell texts are interned in first-use order, every cell
        /// shares style 0, and a cell's width code goes into the wire
        /// cell-flags byte. Rows are serialized with their actual cell
        /// count, so an under-width row fails the decoder's
        /// `cells_count == columns` rule instead of being padded silently.
        fn wire(frame: &TerminalTextFrame) -> Vec<u8> {
            let mut strings: Vec<Arc<str>> = Vec::new();
            for row in &frame.rows {
                for cell in &row.cells {
                    if !strings.iter().any(|text| text == &cell.text) {
                        strings.push(cell.text.clone());
                    }
                }
            }
            let route = &frame.route;
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"OBTF");
            bytes.extend_from_slice(&[0, 0]); // format major/minor
            bytes.push(1); // snapshot kind: no base revision
            bytes.push(0); // routing flags
            bytes.extend_from_slice(&[0; 4]); // total length, patched below
            bytes.extend_from_slice(&160_u16.to_le_bytes()); // header bytes
            bytes.extend_from_slice(&[0; 2]); // reserved
            bytes.extend_from_slice(route.host_epoch.as_bytes());
            bytes.extend_from_slice(route.attachment_id.as_bytes());
            bytes.extend_from_slice(&route.sequence.get().to_le_bytes());
            bytes.extend_from_slice(&u64::MAX.to_le_bytes()); // base revision: none
            bytes.extend_from_slice(&route.revision.to_le_bytes());
            bytes.extend_from_slice(&route.generation.get().to_le_bytes());
            bytes.extend_from_slice(&frame.scrollback_rows.to_le_bytes());
            bytes.extend_from_slice(&frame.geometry.columns.to_le_bytes());
            bytes.extend_from_slice(&frame.geometry.rows.to_le_bytes());
            bytes.extend_from_slice(&frame.cursor.column.to_le_bytes());
            bytes.extend_from_slice(&frame.cursor.row.to_le_bytes());
            bytes.push(1); // cursor shape
            bytes.push(u8::from(frame.cursor.visible)); // cursor flags
            bytes.push(if frame.alternate_screen { 2 } else { 1 }); // screen
            bytes.push(0); // reserved
            bytes.extend_from_slice(&0_u32.to_le_bytes()); // interaction
            bytes.extend_from_slice(&frame.foreground.to_le_bytes());
            bytes.extend_from_slice(&frame.background.to_le_bytes());
            bytes.extend_from_slice(&0_u32.to_le_bytes()); // cursor color
            bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // title: none
            bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // directory: none
            bytes.extend_from_slice(&(strings.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&1_u16.to_le_bytes()); // one style entry
            bytes.extend_from_slice(&frame.geometry.rows.to_le_bytes()); // all rows
            bytes.extend_from_slice(&[0; 2]); // no extensions
            bytes.extend_from_slice(&[0; 2]); // reserved tables
            bytes.extend_from_slice(&frame.geometry.cell_pixel_width.to_le_bytes());
            bytes.extend_from_slice(&frame.geometry.cell_pixel_height.to_le_bytes());
            bytes.extend_from_slice(
                &(u32::from(frame.geometry.columns) * u32::from(frame.geometry.rows)).to_le_bytes(),
            );
            bytes.push(3); // OBTF version byte checked by the route inspector
            bytes.extend_from_slice(&frame.theme_revision.to_le_bytes());
            bytes.extend_from_slice(&[0; 7]); // trailing reserved header
            for text in &strings {
                bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
                bytes.extend_from_slice(text.as_bytes());
            }
            // One style entry: default colors, no link, no flags or underline.
            bytes.extend_from_slice(&[0; 12]); // foreground/background/underline
            bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // link: none
            bytes.extend_from_slice(&[0; 4]); // flags, underline, reserved
            for row in &frame.rows {
                bytes.extend_from_slice(&row.index.to_le_bytes());
                bytes.push(row.flags);
                bytes.push(0); // prompt: none
                bytes.extend_from_slice(&(row.cells.len() as u16).to_le_bytes());
                bytes.extend_from_slice(&[0; 2]); // reserved
                for cell in &row.cells {
                    let string = strings
                        .iter()
                        .position(|text| text == &cell.text)
                        .expect("interned") as u32;
                    bytes.extend_from_slice(&string.to_le_bytes());
                    bytes.extend_from_slice(&0_u16.to_le_bytes()); // style 0
                    bytes.push(cell.width); // cell flags carry the width code
                    bytes.push(0); // reserved
                }
            }
            let total = u32::try_from(bytes.len()).expect("frame length fits u32");
            bytes[8..12].copy_from_slice(&total.to_le_bytes());
            bytes
        }

        /// One row of 64 cells that all share a single string.
        fn shared_row(index: u16, flags: u8, text: &Arc<str>) -> TerminalTextRow {
            TerminalTextRow {
                index,
                flags,
                cells: vec![cell(text, 0); usize::from(COLUMNS)],
            }
        }

        /// The exact-boundary frame: 64 cells sharing one 16 KiB string join
        /// into a 1,048,576-char text whose JSON encoding debits 1,048,578
        /// bytes. Three such rows debit 3,145,734; row 3 keeps 63 shared
        /// cells plus a mixed tail (`中`×4000 = 12,000 encoded bytes, ESC×500
        /// = 3,000, `x`×1376 = 1,376 — 16,376 total), encoding to exactly
        /// 1,048,570 and landing the sum on READ_TEXT_BYTES. This pins the
        /// `>`-not-`>=` debit check: a boundary row must still be emitted.
        #[test]
        fn exact_budget_emits_every_row() {
            let full: Arc<str> = Arc::from("x".repeat(16384).as_str());
            let tail_text = format!(
                "{}{}{}",
                "中".repeat(4000),
                "\u{1b}".repeat(500),
                "x".repeat(1376)
            );
            let tail: Arc<str> = Arc::from(tail_text.as_str());
            let mut last_cells = vec![cell(&full, 0); 63];
            last_cells.push(cell(&tail, 0));
            let frame = frame(vec![
                shared_row(0, 0, &full),
                shared_row(1, 0, &full),
                shared_row(2, 0, &full),
                TerminalTextRow {
                    index: 3,
                    flags: 0,
                    cells: last_cells,
                },
            ]);
            let doc = terminal_read_doc(terminal(), &frame).expect("document");
            assert_eq!(doc.row_count, 4);
            assert_eq!(doc.rows.len(), 4);
            assert_eq!(doc.omitted_rows, 0);
            let full_text = "x".repeat(usize::from(COLUMNS) * 16384);
            for (index, row) in doc.rows[..3].iter().enumerate() {
                assert_eq!(row.index, index as u16);
                assert_eq!(row.text, full_text);
            }
            let mut last_text = "x".repeat(63 * 16384);
            last_text.push_str(&tail);
            assert_eq!(doc.rows[3].index, 3);
            assert_eq!(doc.rows[3].text, last_text);
            let content = &Projection::TerminalRead(doc).result()["content"];
            assert_eq!(content["truncated"], false);
            assert_eq!(content["omittedRows"], 0);
            assert_eq!(content["rowCount"], 4);
        }

        /// One encoded byte past the remaining budget omits the whole row:
        /// the mixed tail's `x` run grows to 1,377 so row 3 encodes to
        /// 1,048,571 against 1,048,570 left. The kept rows carry a wrap pair
        /// — bit 0 `intoNext` on row 0 meets bit 1 `fromPrevious` on row 1 —
        /// so a flag-swap mutation fails here too.
        #[test]
        fn one_byte_over_budget_omits_the_row() {
            let full: Arc<str> = Arc::from("x".repeat(16384).as_str());
            let tail: Arc<str> = Arc::from(
                format!(
                    "{}{}{}",
                    "中".repeat(4000),
                    "\u{1b}".repeat(500),
                    "x".repeat(1377)
                )
                .as_str(),
            );
            let mut last_cells = vec![cell(&full, 0); 63];
            last_cells.push(cell(&tail, 0));
            let frame = frame(vec![
                shared_row(0, 1, &full),
                shared_row(1, 2, &full),
                shared_row(2, 0, &full),
                TerminalTextRow {
                    index: 3,
                    flags: 0,
                    cells: last_cells,
                },
            ]);
            let doc = terminal_read_doc(terminal(), &frame).expect("document");
            assert_eq!(doc.row_count, 4);
            assert_eq!(doc.rows.len(), 3);
            assert_eq!(doc.omitted_rows, 1);
            assert!(doc.rows[0].into_next && !doc.rows[0].from_previous);
            assert!(!doc.rows[1].into_next && doc.rows[1].from_previous);
            assert!(!doc.rows[2].into_next && !doc.rows[2].from_previous);
            let content = &Projection::TerminalRead(doc).result()["content"];
            assert_eq!(content["truncated"], true);
            assert_eq!(content["omittedRows"], 1);
        }

        /// The decoder maximum of 4,096 rows with a first row already over
        /// budget: 64 cells sharing a 16 KiB control-string join to
        /// 1,048,576 chars that JSON-escape to 6,291,458 bytes, so every row
        /// is omitted and the count stays honest at the schema's upper bound.
        /// A miscounted `omitted_rows` or a budget that only checked `text`
        /// length before escaping fails here.
        #[test]
        fn over_budget_first_row_omits_all_rows() {
            let control: Arc<str> = Arc::from("\u{1}".repeat(16384).as_str());
            let rows = (0..4096)
                .map(|index| shared_row(index, 0, &control))
                .collect();
            let frame = frame(rows);
            let doc = terminal_read_doc(terminal(), &frame).expect("document");
            assert!(doc.rows.is_empty());
            assert_eq!(doc.row_count, 4096);
            assert_eq!(doc.omitted_rows, 4096);
            let content = &Projection::TerminalRead(doc).result()["content"];
            assert_eq!(content["truncated"], true);
            assert_eq!(content["omittedRows"], 4096);
            assert_eq!(content["rows"].as_array().expect("rows").len(), 0);
        }

        /// Grid-cell rendering inside the budget path: a spacer tail is
        /// collapsed into its wide cell, a spacer head or an empty cell
        /// renders one space, trailing padding trims, and a blank-only row
        /// emits an empty line. The rows obey the decoder's topology: a row
        /// ending in a spacer head sets `intoNext`, so the next row opens
        /// with a wide cell and `fromPrevious`. The dedicated 3-column
        /// geometry keeps every row at exactly `columns` cells so the real
        /// decoder accepts the frame.
        #[test]
        fn spacer_and_blank_cells_render_as_grid_content() {
            let wide_text: Arc<str> = Arc::from("中");
            let blank: Arc<str> = Arc::from("");
            let narrow: Arc<str> = Arc::from("x");
            let rows = vec![
                TerminalTextRow {
                    index: 0,
                    flags: 1,
                    cells: vec![cell(&narrow, 0), cell(&narrow, 0), cell(&blank, 3)],
                },
                TerminalTextRow {
                    index: 1,
                    flags: 2,
                    cells: vec![
                        cell(&wide_text, 1),
                        cell(&blank, 2),
                        cell(&Arc::from("界"), 0),
                    ],
                },
                TerminalTextRow {
                    index: 2,
                    flags: 0,
                    cells: vec![cell(&blank, 0), cell(&narrow, 0), cell(&blank, 0)],
                },
                TerminalTextRow {
                    index: 3,
                    flags: 0,
                    cells: vec![cell(&blank, 0), cell(&blank, 0), cell(&blank, 0)],
                },
            ];
            // The pre-repair shape: the same rows under the shared 64-column
            // geometry carry fewer than `columns` cells, so the real decoder
            // rejects the frame as `InvalidField`.
            let mut under_width = rows.clone();
            under_width[3].cells.pop();
            let error = decode_terminal_text_frame(&wire(&frame(under_width)))
                .expect_err("under-width rows must be rejected");
            assert_eq!(error.code(), ProtocolErrorCode::InvalidField);
            let frame = frame_with_columns(rows, 3);
            let decoded = decode_terminal_text_frame(&wire(&frame)).expect("fixture decodes");
            assert_eq!(decoded.geometry, frame.geometry);
            assert_eq!(decoded.rows, frame.rows);
            let doc = terminal_read_doc(terminal(), &frame).expect("document");
            let texts: Vec<&str> = doc.rows.iter().map(|row| row.text.as_str()).collect();
            assert_eq!(texts, ["xx", "中界", " x", ""]);
            assert!(doc.rows[0].into_next && doc.rows[1].from_previous);
            assert_eq!(doc.omitted_rows, 0);
        }

        /// Negative control for the budget stop: emitted rows are a strict
        /// prefix of the frame, never a greedy subset. The middle row alone
        /// overflows the remaining budget — 64 cells sharing one 16 KiB
        /// control string JSON-escape to 6,291,458 bytes — so the small tail
        /// row behind it must be omitted even though it would fit on its
        /// own. A `break`→`continue` mutation would emit that tail and fail
        /// here. This pins the CLI/decoder boundary only; no real host is
        /// involved.
        #[test]
        fn rows_after_the_budget_stop_stay_omitted() {
            let head: Arc<str> = Arc::from("head\u{1b}row");
            let control: Arc<str> = Arc::from("\u{1}".repeat(16384).as_str());
            let tail: Arc<str> = Arc::from("tail-marker-row");
            // One literal cell plus narrow blank cells: the joined line is
            // the literal text after trailing-blank trim, and each row
            // carries exactly `columns` cells as the decoder requires. A
            // spacer head is not legal padding — it is only valid as the
            // final cell of a row that wraps `intoNext`.
            let blank: Arc<str> = Arc::from("");
            let text_row = |index: u16, text: &Arc<str>| {
                let mut cells = vec![cell(text, 0)];
                cells.extend((1..COLUMNS).map(|_| cell(&blank, 0)));
                TerminalTextRow {
                    index,
                    flags: 0,
                    cells,
                }
            };
            // The pre-repair shape padded with spacer heads whose flags were
            // 0: mid-row spacer heads and a spacer head on a non-wrapping row
            // are both `InvalidField` to the real decoder.
            let spacer_head_row = |index: u16, text: &Arc<str>| {
                let mut cells = vec![cell(text, 0)];
                cells.extend((1..COLUMNS).map(|_| cell(&blank, 3)));
                TerminalTextRow {
                    index,
                    flags: 0,
                    cells,
                }
            };
            let illegal = frame(vec![
                spacer_head_row(0, &head),
                shared_row(1, 0, &control),
                spacer_head_row(2, &tail),
            ]);
            let error = decode_terminal_text_frame(&wire(&illegal))
                .expect_err("spacer-head padding must be rejected");
            assert_eq!(error.code(), ProtocolErrorCode::InvalidField);
            let frame = frame(vec![
                text_row(0, &head),
                shared_row(1, 0, &control),
                text_row(2, &tail),
            ]);
            let decoded = decode_terminal_text_frame(&wire(&frame)).expect("fixture decodes");
            assert_eq!(decoded.geometry, frame.geometry);
            assert_eq!(decoded.rows, frame.rows);
            let doc = terminal_read_doc(terminal(), &frame).expect("document");
            assert_eq!(doc.row_count, 3);
            assert_eq!(doc.rows.len(), 1);
            assert_eq!(doc.rows[0].index, 0);
            assert_eq!(doc.rows[0].text, "head\u{1b}row");
            assert_eq!(doc.omitted_rows, 2);

            let projection = Projection::TerminalRead(doc);
            let content = &projection.result()["content"];
            assert_eq!(content["rowCount"], 3);
            assert_eq!(content["truncated"], true);
            assert_eq!(content["omittedRows"], 2);
            let rows = content["rows"].as_array().expect("rows array");
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["index"], 0);
            assert_eq!(rows[0]["text"], "head\u{1b}row");

            // The real text path emits only the prefix: the head's control
            // character stays escaped, the affordable tail never leaks, and
            // the localized marker counts both omitted rows.
            let command = QueryCommand::TerminalRead {
                target: Target::Id(hex(0x01)),
                history: None,
            };
            let body = render_text(|out| present::render(out, &projection, &command, Locale::EnUs))
                .expect("text output fits the preparation bound");
            let body = String::from_utf8(body).expect("utf8 output");
            assert!(body.contains("head\\u{001b}row"));
            assert!(!body.contains("tail-marker-row"));
            assert!(body.ends_with("… 2 more rows truncated at the read limit\n"));
        }
    }

    /// `terminal wait-output` matching: soft-wrapped rows join into logical
    /// lines before the literal substring check, the excerpt always contains
    /// the complete pattern within 4096 UTF-8 bytes on char boundaries, and
    /// the observed-window projection reports the honest truncation flag.
    #[cfg(unix)]
    mod terminal_wait {
        use super::super::{
            MAXIMUM_EXCERPT_BYTES, TerminalRow, WaitMatch, WaitObservation, match_window,
            terminal_wait_output_doc,
        };
        use orchbee_terminal_protocol::{
            AttachmentId, HostEpoch, TerminalRowPromptClass, TerminalTextRowWindow,
            TerminalTextWindowRow,
        };
        use std::num::NonZeroU64;

        fn row(index: u64, wrap: bool, continuation: bool, text: &str) -> TerminalTextWindowRow {
            TerminalTextWindowRow {
                index,
                prompt_class: TerminalRowPromptClass::None,
                wrap,
                wrap_continuation: continuation,
                identity_ascii: true,
                text: text.into(),
                mappings: Vec::new(),
            }
        }

        fn window(rows: Vec<TerminalTextWindowRow>, total_rows: u64) -> TerminalTextRowWindow {
            TerminalTextRowWindow {
                attachment_id: AttachmentId::from_bytes([7; 16]).expect("attachment"),
                host_epoch: HostEpoch::from_bytes([9; 16]).expect("epoch"),
                sequence: NonZeroU64::new(1).expect("sequence"),
                revision: 4,
                generation: NonZeroU64::new(2).expect("generation"),
                total_rows,
                start_row: rows.first().map(|row| row.index).unwrap_or(0),
                columns: 80,
                ignored_optional_flags: 0,
                rows,
            }
        }

        /// A match that only exists inside the joined logical line: no single
        /// physical row contains the pattern, so a per-row search would miss.
        #[test]
        fn wrap_chain_joins_before_matching() {
            let hit = match_window(
                &window(
                    vec![
                        row(5, true, false, "prompt$ echo abc"),
                        row(6, false, true, "def done"),
                        row(7, false, false, "unrelated"),
                    ],
                    8,
                ),
                "abcdef",
            )
            .expect("joined line matches");
            assert_eq!(hit.row_start, 5);
            assert_eq!(hit.row_end, 6);
            assert_eq!(hit.excerpt, "prompt$ echo abcdef done");
            assert!(!hit.excerpt_truncated);
            assert_eq!(hit.match_offset, 13);
            assert_eq!(hit.excerpt_offset, 0);
        }

        /// The first matching logical line wins; hard boundaries never join.
        #[test]
        fn first_logical_line_wins_across_hard_boundaries() {
            let hit = match_window(
                &window(
                    vec![
                        row(0, false, false, "first hit here"),
                        row(1, true, false, "second "),
                        row(2, false, true, "hit joined"),
                    ],
                    3,
                ),
                "hit",
            )
            .expect("earliest line matches");
            assert_eq!(hit.row_start, 0);
            assert_eq!(hit.row_end, 0);
            assert_eq!(hit.excerpt, "first hit here");
        }

        /// Matching is a literal, case-sensitive substring: regex syntax and
        /// case folds never match.
        #[test]
        fn literal_case_sensitive_substring() {
            let window = window(vec![row(0, false, false, "Foo a.b")], 1);
            assert!(match_window(&window, "foo").is_none());
            assert!(match_window(&window, "a.b").is_some());
            assert!(match_window(&window, "a[x]b").is_none());
            assert!(match_window(&window, "a.*b").is_none());
        }

        /// A window that opens mid-line (leading `wrap_continuation`) still
        /// joins that fragment with the following continuation rows.
        #[test]
        fn leading_continuation_fragment_joins() {
            let hit = match_window(
                &window(
                    vec![
                        row(9, true, true, "tail of earlier "),
                        row(10, false, true, "marker"),
                        row(11, false, false, "other"),
                    ],
                    12,
                ),
                "earlier marker",
            )
            .expect("fragment plus continuation matches");
            assert_eq!(hit.row_start, 9);
            assert_eq!(hit.row_end, 10);
        }

        /// Long lines excerpt to ≤4096 bytes with the complete pattern kept;
        /// a tail hit re-anchors instead of losing the match to alignment.
        #[test]
        fn excerpt_keeps_complete_pattern_within_budget() {
            let padding = "x".repeat(6000);
            let line = format!("{padding} needle");
            let (excerpt, truncated, offset) =
                super::super::match_excerpt(&line, 6001, "needle".len());
            assert!(truncated);
            assert!(excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
            assert!(excerpt.contains("needle"));
            let start = usize::try_from(offset).expect("offset");
            assert!(start <= 6001);
            assert!(6001 + "needle".len() <= start + excerpt.len());

            // Under the bound the whole line is the excerpt.
            let short = "short line with needle";
            let (excerpt, truncated, offset) =
                super::super::match_excerpt(short, 17, "needle".len());
            assert!(!truncated);
            assert_eq!(excerpt, short);
            assert_eq!(offset, 0);
        }

        /// Excerpt slicing never splits a UTF-8 code point: a CJK-dense line
        /// still yields a valid, pattern-containing window.
        #[test]
        fn excerpt_respects_char_boundaries() {
            let line = format!("{}目标", "中".repeat(2000));
            let start = line.find("目标").expect("pattern");
            let (excerpt, truncated, offset) = super::super::match_excerpt(&line, start, 6);
            assert!(truncated);
            assert!(excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
            assert!(excerpt.contains("目标"));
            assert!(line.is_char_boundary(usize::try_from(offset).expect("offset")));
        }

        /// Review F3: the re-anchor arm is reachable — a 3-byte CJK prefix
        /// pushes the centered start down to 0, so the aligned end lands at
        /// 4096 < 4097 below the match end. Re-anchoring on the match
        /// boundary keeps the complete 4094-byte pattern inside the budget.
        #[test]
        fn excerpt_reanchors_when_cjk_prefix_drops_the_aligned_end() {
            let pattern = "a".repeat(4094);
            let line = format!("中{pattern}{}", "z".repeat(10));
            let match_start = line.find(&pattern).expect("pattern");
            assert_eq!(match_start, 3);
            let (excerpt, truncated, offset) =
                super::super::match_excerpt(&line, match_start, pattern.len());
            assert!(truncated);
            assert!(excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
            // The re-anchored excerpt is exactly the whole pattern.
            assert_eq!(excerpt, pattern);
            assert_eq!(usize::try_from(offset).expect("offset"), 3);
            assert!(line.is_char_boundary(3));
        }

        /// `windowTruncated` is the byte-capacity clamp fact: true only when
        /// the searched window is smaller than the requested-or-retained
        /// extent, never a guess.
        #[test]
        fn doc_reports_window_truncation_honestly() {
            let matched = WaitMatch {
                excerpt: "hit".to_owned(),
                excerpt_truncated: false,
                excerpt_offset: 0,
                match_offset: 0,
                row_start: 3,
                row_end: 3,
            };
            let doc = |window_rows, total_rows, requested| {
                terminal_wait_output_doc(
                    TerminalRow {
                        terminal_id: "0".repeat(32),
                        worker_id: "0".repeat(32),
                        presentation_id: "0".repeat(32),
                        panel_id: "0".repeat(32),
                        tab_id: "0".repeat(32),
                        terminal_context_id: "0".repeat(32),
                    },
                    AttachmentId::from_bytes([7; 16]).expect("attachment"),
                    HostEpoch::from_bytes([9; 16]).expect("epoch"),
                    WaitObservation {
                        matched: matched.clone(),
                        revision: 4,
                        generation: 2,
                        window_start: 0,
                        window_rows,
                        total_rows,
                    },
                    super::super::WaitReport {
                        observations: 1,
                        requested_rows: requested,
                        timeout_ms: 30_000,
                        elapsed_ms: 12,
                    },
                )
            };
            // 500 rows searched but 1000 requested and 1000 retained: clamped.
            assert!(doc(500, 2000, 1000).window_truncated);
            // The full retained window was searched.
            assert!(!doc(24, 24, 1000).window_truncated);
            // Fewer requested rows than retained: the request bound the window.
            assert!(!doc(10, 24, 10).window_truncated);
        }
    }
}
