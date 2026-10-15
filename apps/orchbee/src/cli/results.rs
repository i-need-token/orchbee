//! Pure projection of a validated Workbench `Snapshot` into the CLI result
//! model. Everything here is total over a snapshot the transport already
//! verified; no fabricated fields appear where no authoritative source exists.

#[cfg(not(windows))]
use orchbee_agent_observation_protocol::AgentGetOutcome;
use std::collections::HashMap;

use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentListOutcome, AgentSafeInteger, AgentSessionDetail, ObapRequestId,
};
use orchbee_client_server_protocol::{LaunchProfileProvenance, TerminalLaunchFact};
use orchbee_terminal_protocol::MAXIMUM_FRAME_ROWS;
use orchbee_workbench_protocol::{
    LaunchProfileCatalogRevision, LaunchProfileEntry, PanelNode, PanelSlot, Presentation, Project,
    Snapshot, Tab, TerminalContext, WorkbenchPanelId, WorkbenchProtocolDocument,
    WorkbenchWorkspaceCloseReadOutcome, Workspace, WorktreeEntry, WorktreeEntryText,
};
use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, target_details};
use crate::invocation::{AgentWaitCondition, PanelQueryTarget, QueryCommand, Resource, Target};

pub(crate) fn schema_id(command: &QueryCommand) -> &'static str {
    match command {
        QueryCommand::Status => "orchbee.status",
        QueryCommand::AgentList { .. } => "orchbee.agent.list",
        QueryCommand::AgentChanges { .. } => "orchbee.agent.changes",
        QueryCommand::AgentGet(_) | QueryCommand::AgentClientGet { .. } => "orchbee.agent.get",
        QueryCommand::AgentExplain(_) => "orchbee.agent.explain",
        QueryCommand::AgentWait { .. } => "orchbee.agent.wait",
        QueryCommand::AgentDoctor { .. } => "orchbee.agent.doctor",
        QueryCommand::ProjectList => "orchbee.project.list",
        QueryCommand::ProjectGet(_) => "orchbee.project.get",
        QueryCommand::WorkspaceList { .. } => "orchbee.workspace.list",
        QueryCommand::WorkspaceGet(_) => "orchbee.workspace.get",
        QueryCommand::WorkspaceCloseStatus { .. } => "orchbee.workspace.close-status",
        QueryCommand::TabList { .. } => "orchbee.tab.list",
        QueryCommand::TabGet(_) => "orchbee.tab.get",
        QueryCommand::PanelList { .. } => "orchbee.panel.list",
        QueryCommand::PanelCurrent(_) => "orchbee.panel.current",
        QueryCommand::PanelGet(_) => "orchbee.panel.get",
        QueryCommand::PanelProcessInfo(_) => "orchbee.panel.process-info",
        QueryCommand::TerminalList => "orchbee.terminal.list",
        QueryCommand::TerminalGet(_) => "orchbee.terminal.get",
        QueryCommand::TerminalRead { history, .. } | QueryCommand::AgentRead { history, .. } => {
            if history.is_some() {
                "orchbee.terminal.history"
            } else {
                "orchbee.terminal.read"
            }
        }
        QueryCommand::TerminalOutput { .. } => "orchbee.terminal.output",
        QueryCommand::TerminalWaitOutput { .. } | QueryCommand::TerminalSourceWait { .. } => {
            "orchbee.terminal.wait-output"
        }
        QueryCommand::WorktreeList { .. } => "orchbee.worktree.list",
        QueryCommand::LaunchProfileList => "orchbee.launch-profile.list",
        QueryCommand::LaunchProfileGet(_) => "orchbee.launch-profile.get",
        QueryCommand::ApiSnapshot => "orchbee.api.snapshot",
        QueryCommand::ApiSchema { .. } => "orchbee.api.schema",
        QueryCommand::ConfigCheck => "orchbee.config.check",
    }
}

/// `terminal get` v2 added the
/// `launch` member and a Server-joined source, v3 adds the `launchProfile`
/// provenance sibling — each a deliberate break older validators reject.
/// `terminal read`/`--history` and `terminal wait-output` carry a
/// conditional v2: the envelope only reports `2` when the document actually
/// bears the new members — `lines`/`lineCount`/`tail` on a read, the regex
/// match detail on a wait — so flag-free output stays byte-identical.
pub(crate) const fn schema_version(command: &QueryCommand) -> u64 {
    match command {
        QueryCommand::LaunchProfileList | QueryCommand::LaunchProfileGet(_) => 2,
        #[cfg(windows)]
        QueryCommand::AgentWait {
            client: Some(_), ..
        } => 8,
        #[cfg(windows)]
        QueryCommand::AgentList {
            client: Some(_), ..
        } => 3,
        #[cfg(windows)]
        QueryCommand::AgentGet(_) => 17,
        QueryCommand::AgentClientGet { .. } => {
            if cfg!(windows) {
                18
            } else {
                2
            }
        }
        QueryCommand::TerminalSourceWait { .. } => 3,
        QueryCommand::TerminalOutput { ansi: true, .. } => 2,
        QueryCommand::PanelGet(_) => {
            if cfg!(windows) {
                11
            } else {
                10
            }
        }
        QueryCommand::PanelCurrent(_) => {
            if cfg!(windows) {
                9
            } else {
                3
            }
        }
        QueryCommand::ApiSnapshot => 7,
        QueryCommand::PanelList { .. } => {
            if cfg!(windows) {
                11
            } else {
                10
            }
        }
        QueryCommand::TabGet(_) => 7,
        QueryCommand::WorkspaceList { .. } | QueryCommand::WorkspaceGet(_) => 8,
        QueryCommand::ProjectList
        | QueryCommand::ProjectGet(_)
        | QueryCommand::WorktreeList { .. }
        | QueryCommand::WorkspaceCloseStatus { .. } => 2,
        QueryCommand::TabList { .. } => 5,
        QueryCommand::TerminalGet(_) => 4,
        #[cfg(windows)]
        QueryCommand::AgentWait { .. } => 7,
        #[cfg(not(windows))]
        QueryCommand::AgentWait { .. } => 2,
        QueryCommand::PanelProcessInfo(_) => 2,
        QueryCommand::TerminalRead { unwrap, lines, .. }
        | QueryCommand::AgentRead { unwrap, lines, .. }
            if *unwrap || lines.is_some() =>
        {
            2
        }
        QueryCommand::TerminalWaitOutput {
            matcher: crate::invocation::WaitPattern::Regex(_),
            ..
        } => 2,
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

/// `source` for a correlated Agent read answer: the operation family plus
/// the request identity the semantic reply carried back. Only attached after
/// a valid correlated response — local transport, timeout, cancellation, and
/// protocol faults have no honest source.
pub(crate) fn agent_source(request_id: &ObapRequestId) -> Value {
    json!({
        "kind": "agent-read",
        "requestId": request_id.as_str(),
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
    pub(crate) number: usize,
    pub(crate) tab_count: usize,
    pub(crate) panel_count: usize,
    pub(crate) workspace_id: String,
    pub(crate) label: String,
    pub(crate) project_id: Option<String>,
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
    pub(crate) number: usize,
    pub(crate) panel_count: usize,
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
    pub(crate) workspace_id: Option<String>,
    pub(crate) label: String,
    pub(crate) custom_label: Option<String>,
    pub(crate) display_label: String,
    pub(crate) right_click: orchbee_workbench_protocol::PanelRightClickTarget,
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
    pub(crate) custom_label: Option<String>,
    pub(crate) display_label: String,
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

#[derive(Debug)]
pub(crate) struct PanelProcessInfoDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) info: orchbee_terminal_protocol::TerminalProcessInfo,
}

/// `terminal get` v3: the Server's point-in-time join — the current routing
/// row plus the retained launch fact under one coordinator lock, and the
/// launch-profile provenance when the terminal was launched through a named
/// profile. `terminal` keeps the shared identity shape; only this document
/// version carries `launch`/`launchProfile`.
#[derive(Clone, Debug)]
pub(crate) struct TerminalGetDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) launch: TerminalLaunchFact,
    pub(crate) launch_profile: Option<LaunchProfileProvenance>,
    pub(crate) launch_selection: Option<orchbee_client_server_protocol::LaunchSelection>,
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

/// One logical line presented under `--unwrap`: the joined text of a maximal
/// verified soft-wrap chain plus the first and last physical rows it spans.
/// A window opening or ending inside a wrapped line yields a fragment, never
/// a claimed complete line — `starts_mid_line`/`ends_mid_line` carry that.
#[derive(Clone, Debug)]
pub(crate) struct LogicalLine {
    pub(crate) first_row: u64,
    pub(crate) last_row: u64,
    pub(crate) text: String,
    pub(crate) starts_mid_line: bool,
    pub(crate) ends_mid_line: bool,
}

/// The `--lines <n>` presentation tail accounting: how many entries the
/// caller asked to keep and how many leading emitted entries the tail
/// dropped — physical rows, or logical lines under `--unwrap`. This count is
/// deliberately separate from `omitted_rows`, which measures rows lost to
/// the fetch window or the encoded-text budget.
#[derive(Clone, Debug)]
pub(crate) struct ReadTail {
    pub(crate) requested: u16,
    pub(crate) omitted: usize,
}

/// The read-only presentation transforms `terminal read` admits:
/// `--unwrap` joins verified soft-wrap chains into logical lines; `--lines`
/// tails the emitted entries. Neither changes the fetched window.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ReadPresentation {
    pub(crate) unwrap: bool,
    pub(crate) tail: Option<u16>,
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalReadDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) frame: TerminalFrameDoc,
    /// Emitted rows only; `row_count` counts every row in the applied frame.
    /// Under `--unwrap` the rows stay the join's source but the document
    /// emits `lines` instead.
    pub(crate) rows: Vec<TerminalContentRow>,
    pub(crate) row_count: usize,
    pub(crate) omitted_rows: usize,
    /// `--unwrap` output: logical lines joined over the emitted rows.
    pub(crate) lines: Option<Vec<LogicalLine>>,
    /// Logical lines the emitted rows formed before any tail — observed
    /// fragments only, never the retained-history total.
    pub(crate) line_count: Option<usize>,
    /// `--lines` tail accounting over the emitted entries.
    pub(crate) tail: Option<ReadTail>,
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
    /// `--unwrap` output: logical lines joined over the emitted window rows.
    pub(crate) lines: Option<Vec<LogicalLine>>,
    /// Logical lines the emitted rows formed before any tail.
    pub(crate) line_count: Option<usize>,
    /// `--lines` tail accounting over the emitted entries.
    pub(crate) tail: Option<ReadTail>,
}

/// The read bound: the summed JSON-encoded `text` member length of emitted
/// rows never exceeds this budget, so every format stays far under the
/// 8 MiB preparation cap even on hostile cell content.
pub(crate) const READ_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// Join verified soft-wrap chains into logical lines over already-emitted
/// rows. Each item is `(index, text, from_previous, into_next)`; a row joins
/// the open line only when its own `wrap_continuation` bit is set — the
/// decoder already proved every adjacent pair agrees — so a hard boundary
/// always completes the open line and an empty row still forms a line of
/// its own. Boundary flags report the window edges honestly: a line opens
/// mid-line when its first row continues from a row outside the window.
fn join_logical_lines<'a, I>(rows: I) -> Vec<LogicalLine>
where
    I: IntoIterator<Item = (u64, &'a str, bool, bool)>,
{
    let mut lines = Vec::new();
    let mut current: Option<LogicalLine> = None;
    for (index, text, from_previous, into_next) in rows {
        match current.as_mut() {
            Some(line) if from_previous => {
                line.text.push_str(text);
                line.last_row = index;
                line.ends_mid_line = into_next;
            }
            _ => {
                if let Some(line) = current.take() {
                    lines.push(line);
                }
                current = Some(LogicalLine {
                    first_row: index,
                    last_row: index,
                    text: text.to_owned(),
                    starts_mid_line: from_previous,
                    ends_mid_line: into_next,
                });
            }
        }
    }
    if let Some(line) = current {
        lines.push(line);
    }
    lines
}

/// The `--lines` tail: keep the last `requested` emitted entries and record
/// the honestly counted leading omissions.
fn apply_tail<T>(entries: &mut Vec<T>, requested: u16) -> ReadTail {
    let omitted = entries.len().saturating_sub(usize::from(requested));
    if omitted > 0 {
        entries.drain(..omitted);
    }
    ReadTail { requested, omitted }
}

/// Project the applied frame into the bounded answer: rows are emitted in
/// frame order until the encoded-text budget would overflow, and every
/// omitted row is counted honestly. The presentation transforms then apply
/// over the emitted rows only — `--unwrap` re-presents them as joined
/// logical lines, `--lines` keeps the tail — without touching the bound or
/// the physical-row facts.
#[cfg(any(unix, windows))]
pub(crate) fn terminal_read_doc(
    terminal: TerminalRow,
    frame: &orchbee_terminal_protocol::TerminalTextFrame,
    presentation: ReadPresentation,
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
    let mut doc = TerminalReadDoc {
        terminal,
        frame: doc,
        omitted_rows: row_count - rows.len(),
        rows,
        row_count,
        lines: None,
        line_count: None,
        tail: None,
    };
    if presentation.unwrap {
        let mut lines = join_logical_lines(doc.rows.iter().map(|row| {
            (
                u64::from(row.index),
                row.text.as_str(),
                row.from_previous,
                row.into_next,
            )
        }));
        doc.line_count = Some(lines.len());
        if let Some(requested) = presentation.tail {
            doc.tail = Some(apply_tail(&mut lines, requested));
        }
        doc.lines = Some(lines);
    } else if let Some(requested) = presentation.tail {
        doc.tail = Some(apply_tail(&mut doc.rows, requested));
    }
    Ok(doc)
}

/// The history read bound admitted by the `--rows` grammar: 1..=1000.
pub(crate) const MAXIMUM_HISTORY_ROWS: u16 = 1000;

/// Project a validated history window into the bounded answer. `window` is
/// the complete Host row window the CLI verified against its request; emitted
/// rows stop when the encoded-text budget would overflow, and `omitted_rows`
/// honestly counts `min(requested, scrollback)` minus the emitted rows —
/// including rows the protocol window cap never reached.
#[cfg(any(unix, windows))]
pub(crate) fn terminal_history_doc(
    terminal: TerminalRow,
    frame: &orchbee_terminal_protocol::TerminalTextFrame,
    requested_rows: u16,
    window: Option<&orchbee_terminal_protocol::TerminalTextRowWindow>,
    presentation: ReadPresentation,
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
    let mut doc = TerminalHistoryDoc {
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
        lines: None,
        line_count: None,
        tail: None,
    };
    if presentation.unwrap {
        let mut lines = join_logical_lines(doc.rows.iter().map(|row| {
            (
                row.index,
                row.text.as_str(),
                row.from_previous,
                row.into_next,
            )
        }));
        doc.line_count = Some(lines.len());
        if let Some(requested) = presentation.tail {
            doc.tail = Some(apply_tail(&mut lines, requested));
        }
        doc.lines = Some(lines);
    } else if let Some(requested) = presentation.tail {
        doc.tail = Some(apply_tail(&mut doc.rows, requested));
    }
    Ok(doc)
}

/// The regex-only match detail: a `--regex` hit reports the complete match
/// length, the match text bounded like the excerpt, and whether the observed
/// fragment is an edge fragment — `^`/`$` anchors bound the observed
/// fragment, never unseen text outside it.
#[derive(Clone, Debug)]
pub(crate) struct RegexMatchDetail {
    /// Byte length of the complete match inside the joined fragment; may
    /// exceed the excerpt bound.
    pub(crate) match_length: u64,
    /// The match text itself, bounded to `MAXIMUM_EXCERPT_BYTES` — the
    /// leading slice on a char boundary when the match is longer.
    pub(crate) matched: String,
    /// `true` when `matched` is a prefix of the complete match.
    pub(crate) matched_truncated: bool,
    /// The fragment's first observed row continues a logical line whose
    /// head was never fetched.
    pub(crate) starts_mid_line: bool,
    /// The fragment's last observed row continues into a row that was never
    /// fetched.
    pub(crate) ends_mid_line: bool,
}

/// One hit inside a joined logical line, plus the bounded excerpt that
/// contains the complete match whenever the match fits the excerpt bound.
#[derive(Clone, Debug)]
pub(crate) struct WaitMatch {
    pub(crate) excerpt: String,
    /// `true` when the excerpt is a slice, not the whole observed fragment.
    pub(crate) excerpt_truncated: bool,
    /// Byte offset of the excerpt within the matched logical fragment.
    pub(crate) excerpt_offset: u64,
    /// Byte offset of the match start within the matched logical fragment.
    pub(crate) match_offset: u64,
    /// First and last physical rows the matched logical line spans.
    pub(crate) row_start: u64,
    pub(crate) row_end: u64,
    /// The `--regex` extension members; absent for a literal `--match` hit.
    pub(crate) regex: Option<RegexMatchDetail>,
}

/// The compiled search input `wait-output` matches with: the literal
/// `--match` text, or the `--regex` program compiled once before any effect.
#[cfg(any(unix, windows))]
pub(crate) enum WaitMatcher<'a> {
    Literal(&'a str),
    Regex(&'a regex::Regex),
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
    /// Complete validated samples that each completed one match pass,
    /// including source-visible and zero-line samples without a row read.
    pub(crate) observations: u64,
    pub(crate) timeout_ms: Option<u32>,
    pub(crate) selection: Option<crate::invocation::OutputSelection>,
    pub(crate) elapsed_ms: u64,
}

/// The match excerpt bound: at most this many UTF-8 bytes. It always
/// contains a complete `--match` literal; a `--regex` match may exceed it,
/// in which case the excerpt leads with the match's start and the extended
/// members carry the truth.
pub(crate) const MAXIMUM_EXCERPT_BYTES: usize = 4096;

/// Join every wrap-compatible row pair into logical fragments — the decoder
/// already proved `wrap`/`wrap_continuation` agree on every adjacent pair —
/// then find the first fragment the matcher hits. A fragment boundary is a
/// row's own `wrap_continuation` bit, never accumulated-text emptiness, so
/// empty logical lines are observed and matchable (`^$` included). Window
/// edges are reported honestly: a fragment that begins or ends inside a
/// wrapped line marks `starts_mid_line`/`ends_mid_line` on a regex hit, and
/// `^`/`$` anchor at the observed fragment edges only.
#[cfg(any(unix, windows))]
pub(crate) fn match_window(
    window: &orchbee_terminal_protocol::TerminalTextRowWindow,
    matcher: &WaitMatcher<'_>,
) -> Option<WaitMatch> {
    match_rows(
        window.rows.iter().map(|row| {
            (
                row.index,
                row.text.as_ref(),
                row.wrap_continuation,
                row.wrap,
            )
        }),
        matcher,
        true,
    )
}

#[cfg(any(unix, windows))]
pub(super) fn match_rows<'a>(
    rows: impl IntoIterator<Item = (u64, &'a str, bool, bool)>,
    matcher: &WaitMatcher<'_>,
    join_soft_wraps: bool,
) -> Option<WaitMatch> {
    let mut text = String::new();
    let mut row_start = 0_u64;
    let mut row_end = 0_u64;
    let mut starts_mid_line = false;
    let mut ends_mid_line = false;
    // A fragment is accumulating whenever `open` holds — independent of
    // whether the joined text is empty.
    let mut open = false;
    for (index, content, from_previous, into_next) in rows {
        if open && !(join_soft_wraps && from_previous) {
            // A hard boundary completes the previous logical fragment; it is
            // matched before the new one accumulates.
            if let Some(matched) = match_line(
                &text,
                row_start,
                row_end,
                starts_mid_line,
                ends_mid_line,
                matcher,
            ) {
                return Some(matched);
            }
            text.clear();
            open = false;
        }
        if !open {
            // A hard boundary — or a window opening mid-line on a
            // continuation fragment — starts the observed fragment here.
            row_start = index;
            starts_mid_line = from_previous;
            open = true;
        }
        text.push_str(content);
        row_end = index;
        // Preserve original edge facts even when physical-source matching
        // deliberately separates two wrap-compatible rows.
        ends_mid_line = into_next;
    }
    if open {
        return match_line(
            &text,
            row_start,
            row_end,
            starts_mid_line,
            ends_mid_line,
            matcher,
        );
    }
    None
}

/// The first hit of `matcher` inside one observed logical fragment — the
/// literal substring position, or the regex engine's leftmost match — plus
/// the bounded excerpt around it. Offsets are UTF-8 byte offsets into the
/// joined fragment, not terminal columns.
#[cfg(any(unix, windows))]
fn match_line(
    text: &str,
    row_start: u64,
    row_end: u64,
    starts_mid_line: bool,
    ends_mid_line: bool,
    matcher: &WaitMatcher<'_>,
) -> Option<WaitMatch> {
    let (offset, match_len) = match matcher {
        WaitMatcher::Literal(pattern) => text.find(pattern).map(|found| (found, pattern.len())),
        WaitMatcher::Regex(regex) => regex.find(text).map(|hit| (hit.start(), hit.len())),
    }?;
    let (excerpt, truncated, excerpt_offset) = match_excerpt(text, offset, match_len);
    Some(WaitMatch {
        excerpt,
        excerpt_truncated: truncated,
        excerpt_offset,
        match_offset: u64::try_from(offset).unwrap_or(u64::MAX),
        row_start,
        row_end,
        regex: match matcher {
            WaitMatcher::Literal(_) => None,
            WaitMatcher::Regex(_) => {
                let (matched, matched_truncated) = bounded_match(text, offset, match_len);
                Some(RegexMatchDetail {
                    match_length: u64::try_from(match_len).unwrap_or(u64::MAX),
                    matched,
                    matched_truncated,
                    starts_mid_line,
                    ends_mid_line,
                })
            }
        },
    })
}

/// The match text bounded to the excerpt budget: the complete match when it
/// fits, else its leading slice on a char boundary plus the truncation flag.
/// Never subtracts past the bound — an oversized match yields a prefix, not
/// an underflow.
#[cfg(any(unix, windows))]
fn bounded_match(line: &str, match_start: usize, match_len: usize) -> (String, bool) {
    if match_len <= MAXIMUM_EXCERPT_BYTES {
        return (line[match_start..match_start + match_len].to_owned(), false);
    }
    let mut end = match_start + MAXIMUM_EXCERPT_BYTES;
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    (line[match_start..end].to_owned(), true)
}

/// A ≤4096-byte slice of `line` covering the match at
/// `match_start..match_start+match_len`, on UTF-8 boundaries: a match that
/// fits is centered inside the budget, and the window re-anchors on the
/// match when a tail hit would otherwise lose bytes to boundary alignment.
/// A match longer than the budget cannot sit inside the excerpt — the
/// excerpt then leads with the match's own start instead of claiming the
/// whole. No code point is ever split.
#[cfg(any(unix, windows))]
fn match_excerpt(line: &str, match_start: usize, match_len: usize) -> (String, bool, u64) {
    if line.len() <= MAXIMUM_EXCERPT_BYTES {
        return (line.to_owned(), false, 0);
    }
    if match_len > MAXIMUM_EXCERPT_BYTES {
        // Over-budget match: the excerpt is the slice leading with the
        // match's start — a prefix of the match, explicitly truncated.
        let mut end = match_start + MAXIMUM_EXCERPT_BYTES;
        if end > line.len() {
            end = line.len();
        }
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        return (
            line[match_start..end].to_owned(),
            true,
            u64::try_from(match_start).unwrap_or(u64::MAX),
        );
    }
    let match_end = match_start + match_len;
    let context = (MAXIMUM_EXCERPT_BYTES - match_len) / 2;
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
    pub(crate) timeout_ms: Option<u32>,
    pub(crate) elapsed_ms: u64,
}

/// Project the confirmed observation into the answer document; every field
/// already carries validated transport facts, so the projection itself
/// cannot fail.
#[cfg(any(unix, windows))]
pub(crate) fn terminal_wait_output_doc(
    terminal: TerminalRow,
    attachment: orchbee_terminal_protocol::AttachmentId,
    epoch: orchbee_terminal_protocol::HostEpoch,
    observed: WaitObservation,
    report: WaitReport,
) -> TerminalWaitOutputDoc {
    TerminalWaitOutputDoc {
        terminal,
        selection: None,
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

/// Every panel id inside one panel tree, in canonical traversal order — the
/// complete candidate set a `panel` operand resolves over within that scope.
pub(crate) fn panel_ids_in(node: &PanelNode) -> Vec<&WorkbenchPanelId> {
    fn collect<'a>(node: &'a PanelNode, ids: &mut Vec<&'a WorkbenchPanelId>) {
        match node {
            PanelNode::Panel { panel_id, .. } => ids.push(panel_id),
            PanelNode::Split { children, .. } => {
                for slot in children.iter() {
                    collect(&slot.node, ids);
                }
            }
        }
    }
    let mut ids = Vec::new();
    collect(node, &mut ids);
    ids
}

/// Every panel id with its owning context and tab, in canonical traversal
/// order — the complete candidate set for globally scoped `panel` operands.
pub(crate) fn panel_entries(
    snapshot: &Snapshot,
) -> Vec<(&WorkbenchPanelId, &TerminalContext, &Tab)> {
    let mut entries = Vec::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            for panel_id in panel_ids_in(&tab.root) {
                entries.push((panel_id, context, tab));
            }
        }
    }
    entries
}

/// One bound terminal row resolved by typed identity or `@terminal:<suffix>`
/// over the snapshot's complete bound set. Panels and terminals admit no
/// labels, so a `Label` target is parser-impossible and stays a defensive
/// internal failure rather than a silent empty match.
pub(crate) fn terminal_select(
    snapshot: &Snapshot,
    target: &Target,
) -> Result<TerminalRow, Failure> {
    if let Target::Label(_) = target {
        return Err(Failure::new(FailureKind::InternalFailure));
    }
    let rows = terminal_rows(snapshot);
    resolve_or_fail(
        rows.iter().map(|row| (row.terminal_id.as_str(), "", row)),
        target,
        Resource::Terminal,
    )
    .cloned()
}

#[derive(Debug)]
pub(crate) enum Projection {
    #[cfg(windows)]
    WorkbenchQuery(Box<super::workbench_query::Document>),
    Status(StatusDoc),
    /// The typed `listed` outcome, verbatim from the wire.
    AgentList(AgentListOutcome),
    AgentClientList(Box<AgentClientListDoc>),
    AgentChanges(orchbee_agent_observation_protocol::AgentChangesOutcome),
    /// The typed `found` outcome, verbatim from the wire.
    #[cfg(not(windows))]
    AgentGet(AgentGetOutcome),
    AgentInspection(Box<AgentInspectionDoc>),
    #[cfg(windows)]
    AgentTerminalInspection {
        client_view: Option<super::agent_view::windows::terminal_attention::Projection>,
        binding: orchbee_agent_observation_protocol::AgentLaunchBinding,
        terminal: Value,
        management: TerminalManagementDoc,
        semantic: super::query::terminal_agent::TerminalAgentRead,
    },
    /// The typed `found` outcome carrying session plus explanation.
    AgentExplain(AgentExplainOutcome),
    #[cfg(windows)]
    AgentTerminalExplanation(Box<orchbee_workbench_protocol::TerminalAgentExplanation>),
    /// The `agent wait` matched answer: the satisfying session detail
    /// verbatim plus the requested-order matched conditions and the
    /// effective wait echo.
    AgentWait(AgentWaitDoc),
    #[cfg(windows)]
    AgentTerminalWait(Box<super::query::terminal_agent_wait::Matched>),
    ProjectList(Vec<ProjectRow>),
    ProjectGet(ProjectDoc),
    WorkspaceList(Vec<WorkspaceRow>),
    WorkspaceGet(WorkspaceDoc),
    /// The `workspace close-status` answer: the retained operation record's
    /// typed outcome verbatim — `not-closing` or the full `available`
    /// aggregate — never re-projected client-side.
    WorkspaceCloseStatus(WorkbenchWorkspaceCloseReadOutcome),
    TabList(Vec<TabRow>),
    TabGet(TabDoc),
    PanelList(Vec<PanelRow>),
    PanelGet(PanelDoc),
    PanelProcessInfo(PanelProcessInfoDoc),
    TerminalList(Vec<TerminalRow>),
    TerminalGet(TerminalGetDoc),
    TerminalOutput(super::terminal_output::TerminalOutputDoc),
    TerminalRead(TerminalReadDoc),
    TerminalHistory(TerminalHistoryDoc),
    TerminalWaitOutput(TerminalWaitOutputDoc),
    /// The `worktree list` answer: the registered directory identity plus the
    /// observed Git entries, exactly as the workbench response carried them.
    WorktreeList(WorktreeListDoc),
    /// The `launch-profile list`/`get` answer: the catalog-private revision
    /// plus either the whole catalog or the single resolved entry, exactly
    /// as the Server settled them under its lock.
    LaunchProfileCatalog(LaunchProfileCatalogDoc),
    ApiSnapshot(Value),
}

/// The `agent wait` result body: the satisfying `sessionDetail` verbatim,
/// the matched conditions in the caller's requested order, and the
/// effective wait parameters plus honest observation accounting — the
/// poll that matched is the one `source.requestId` names.
#[derive(Debug)]
pub(crate) struct AgentWaitDoc {
    pub(crate) matched: Vec<AgentWaitCondition>,
    pub(crate) revision: AgentSafeInteger,
    pub(crate) session: Box<AgentSessionDetail>,
    #[cfg(windows)]
    pub(crate) matched_change: Option<orchbee_agent_observation_protocol::AgentChange>,
    #[cfg(windows)]
    pub(crate) client: Option<Box<AgentClientViewDoc>>,
    pub(crate) until: Vec<AgentWaitCondition>,
    pub(crate) after_revision: Option<AgentSafeInteger>,
    pub(crate) timeout_ms: Option<u32>,
    pub(crate) observations: u64,
    pub(crate) elapsed_ms: u64,
}

#[derive(Debug)]
pub(crate) struct AgentClientListDoc {
    pub(crate) client_id: orchbee_workbench_protocol::WorkbenchClientId,
    pub(crate) revision: AgentSafeInteger,
    pub(crate) next_cursor: Option<orchbee_agent_observation_protocol::AgentListCursor>,
    pub(crate) items: Vec<(
        orchbee_agent_observation_protocol::AgentSessionSummary,
        AgentClientViewDoc,
    )>,
}

#[derive(Debug)]
pub(crate) struct AgentManagementDoc {
    pub(crate) source: Value,
    pub(crate) revision: AgentSafeInteger,
    pub(crate) management: orchbee_agent_observation_protocol::AgentManagement,
}

#[cfg(windows)]
#[derive(Debug)]
pub(crate) struct TerminalManagementDoc {
    pub(crate) source: Value,
    pub(crate) state: orchbee_agent_observation_protocol::AgentTerminalManagement,
}
impl AgentManagementDoc {
    fn document(&self) -> Value {
        json!({"source":self.source,"revision":self.revision,"state":self.management})
    }
}

#[derive(Debug)]
pub(crate) struct AgentInspectionDoc {
    pub(crate) revision: AgentSafeInteger,
    pub(crate) session: Box<AgentSessionDetail>,
    pub(crate) client: Option<Box<AgentClientViewDoc>>,
    pub(crate) runtime: AgentRuntimeDoc,
    #[cfg(windows)]
    pub(crate) terminal: Option<Value>,
    pub(crate) management: AgentManagementDoc,
}

#[derive(Debug)]
pub(crate) enum AgentRuntimeDoc {
    Unavailable(&'static str),
    Observed {
        source: Value,
        processes: Box<PanelProcessInfoDoc>,
        prompt_ready: bool,
    },
}
impl AgentRuntimeDoc {
    fn document(&self) -> Value {
        match self {
            Self::Unavailable(reason) => json!({"kind":"unavailable","reason":reason}),
            Self::Observed {
                source,
                processes,
                prompt_ready,
            } => {
                json!({"kind":"observed","source":source,"processInfo":panel_process_result(processes),"promptReady":prompt_ready})
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct AgentClientViewDoc {
    pub(crate) client_id: orchbee_workbench_protocol::WorkbenchClientId,
    pub(crate) operation_id: orchbee_workbench_protocol::WorkbenchRequestId,
    pub(crate) phase: orchbee_workbench_protocol::ClientViewPhase,
    pub(crate) outer_focused: Option<bool>,
    pub(crate) focused: bool,
    pub(crate) attention_revision: AgentSafeInteger,
    pub(crate) state: AgentClientVersionMatch,
    pub(crate) state_change_revision: Option<AgentSafeInteger>,
    pub(crate) seen: Option<bool>,
    pub(crate) status: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentClientVersionMatch {
    Current,
    Missing,
    Stale,
}

impl AgentClientViewDoc {
    pub(crate) fn document(&self) -> Value {
        let state = match self.state {
            AgentClientVersionMatch::Current => "current",
            AgentClientVersionMatch::Missing => "missing",
            AgentClientVersionMatch::Stale => "stale",
        };
        json!({
            "clientId": self.client_id, "operationId": self.operation_id,
            "phase": self.phase, "outerFocused": self.outer_focused, "focused": self.focused,
            "attentionRevision": self.attention_revision, "state": state,
            "stateChangeRevision": self.state_change_revision, "seen": self.seen, "status": self.status,
        })
    }
}

/// The `worktree list` result body. `path` is the registered Workbench
/// directory — distinct from each entry's observed `worktreePath`.
#[derive(Clone, Debug)]
pub(crate) struct WorktreeListDoc {
    pub(crate) directory_id: String,
    pub(crate) path: String,
    pub(crate) entries: Vec<WorktreeEntry>,
}

/// The `launch-profile` catalog answer body: the catalog-private revision
/// the Server resolved under, plus the settled selection.
#[derive(Clone, Debug)]
pub(crate) struct LaunchProfileCatalogDoc {
    pub(crate) catalog_revision: LaunchProfileCatalogRevision,
    pub(crate) selection: LaunchProfileCatalogSelection,
}

/// `list` carries every retained entry; `get` carries the single resolved
/// one — both verbatim wire entries, never re-resolved client-side.
#[derive(Clone, Debug)]
pub(crate) enum LaunchProfileCatalogSelection {
    List(Vec<LaunchProfileEntry>),
    Profile(LaunchProfileEntry),
}

pub(super) fn panel_process_result(doc: &PanelProcessInfoDoc) -> Value {
    let view = super::processes::interpret(&doc.info);
    json!({
        "terminal": terminal_row_json(&doc.terminal),
        "scope": "windows-job",
        "rootPid": doc.info.root_pid,
        "foreground": {
            "kind": view.foreground.selection.as_str(),
            "pid": view.foreground.pid,
            "providerId": view.foreground.provider,
        },
        "processes": view.processes.iter().map(|row| {
            let process = row.facts;
            json!({
            "pid": process.pid,
            "parentPid": process.parent_pid,
            "creationTime": process.creation_time.to_string(),
            "image": process.image,
            "commandLine": process.command_line,
            "currentDirectory": process.current_directory,
            "argv": row.argv,
            "providerId": row.provider,
        })}).collect::<Vec<_>>(),
    })
}

impl Projection {
    /// The `result` member of the machine envelope.
    pub(crate) fn result(&self) -> Value {
        match self {
            #[cfg(windows)]
            Self::WorkbenchQuery(doc) => doc.result(),
            // Agent projections serialize the exact wire outcome object —
            // the typed enum is the single serialization truth.
            Self::AgentList(outcome) => json!(outcome),
            Self::AgentClientList(doc) => {
                json!({"kind":"listed","clientId":doc.client_id,"revision":doc.revision,"nextCursor":doc.next_cursor,
                "items":doc.items.iter().map(|(summary,client)| { let mut row=json!(summary);row["client"]=client.document();row }).collect::<Vec<_>>() })
            }
            Self::AgentChanges(outcome) => json!(outcome),
            #[cfg(not(windows))]
            Self::AgentGet(outcome) => json!(outcome),
            #[cfg(windows)]
            Self::AgentTerminalInspection {
                binding,
                terminal,
                management,
                semantic,
                client_view,
            } => {
                let mut doc = json!({
                "kind":"terminal-found", "target":{"kind":"terminal","binding":binding},
                "terminal":terminal, "management":{"source":management.source,"state":management.state}, "semantic":semantic.document(),
                });
                if let Some(view) = client_view {
                    doc["clientView"] = view.document();
                }
                doc
            }
            Self::AgentInspection(doc) => {
                let mut result = json!({"kind":"found","revision":doc.revision,"session":doc.session,"runtime":doc.runtime.document(),"management":doc.management.document()});
                #[cfg(windows)]
                {
                    result["terminal"] = json!(doc.terminal);
                }
                if let Some(client) = &doc.client {
                    result["client"] = client.document();
                }
                result
            }
            Self::AgentExplain(outcome) => json!(outcome),
            #[cfg(windows)]
            Self::AgentTerminalExplanation(terminal) => {
                json!({"kind":"terminal-found","terminal":terminal})
            }
            Self::AgentWait(doc) => {
                // The wait echo carries every effective parameter the poll
                // ran under — the condition list in requested order, the
                // fence only when present, and the honest accounting.
                let mut wait = Map::new();
                wait.insert(
                    "until".to_owned(),
                    json!(
                        doc.until
                            .iter()
                            .map(|condition| condition.as_str())
                            .collect::<Vec<_>>()
                    ),
                );
                if let Some(fence) = doc.after_revision {
                    wait.insert("afterRevision".to_owned(), json!(fence));
                }
                wait.insert("timeoutMs".to_owned(), json!(doc.timeout_ms));
                wait.insert("observations".to_owned(), json!(doc.observations));
                wait.insert("elapsedMs".to_owned(), json!(doc.elapsed_ms));
                let result = json!({
                    "matchedConditions": doc
                        .matched
                        .iter()
                        .map(|condition| condition.as_str())
                        .collect::<Vec<_>>(),
                    "revision": doc.revision,
                    "session": doc.session,
                    "wait": Value::Object(wait),
                });
                #[cfg(windows)]
                let result = {
                    let mut result = result;
                    result["matchedChange"] = json!(doc.matched_change);
                    if let Some(client) = &doc.client {
                        result["client"] = client.document();
                    }
                    result
                };
                result
            }
            #[cfg(windows)]
            Self::AgentTerminalWait(doc) => doc.document(),
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
            // The close read outcome is the single serialization truth — the
            // retained record answers verbatim, tombstone included.
            Self::WorkspaceCloseStatus(outcome) => json!(outcome),
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
                            "workspaceId": row.workspace_id,
                            "label": row.label,
                            "customLabel": row.custom_label,
                            "displayLabel": row.display_label,
                            "rightClick": row.right_click.as_str(),
                            "tabId": row.tab_id,
                            "terminalContextId": row.terminal_context_id,
                            "primaryPresentationId": row.primary_presentation_id,
                            "presentationCount": row.presentation_count,
                            "primaryTerminalId": row.primary_terminal_id,
                        })
                    })
                    .collect::<Vec<_>>(),
            }),
            Self::PanelProcessInfo(doc) => panel_process_result(doc),
            Self::PanelGet(doc) => json!({
                "panel": {
                    "panelId": doc.row.panel_id,
                    "rightClick": doc.row.right_click.as_str(),
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
                // `launchProfile` is its sibling: emitted exactly when the
                // launch named a profile, never for explicit/default launches.
                let mut terminal = terminal_row_json(&doc.terminal);
                if let Value::Object(members) = &mut terminal {
                    members.insert("launch".to_owned(), launch_fact_json(&doc.launch));
                    if let Some(selection) = &doc.launch_selection {
                        members.insert("launchSelection".to_owned(), json!(selection));
                    }
                    if let Some(provenance) = &doc.launch_profile {
                        members.insert(
                            "launchProfile".to_owned(),
                            json!({
                                "profileId": provenance.profile_id(),
                                "catalogRevision": provenance.catalog_revision().to_string(),
                            }),
                        );
                    }
                }
                json!({ "terminal": terminal })
            }
            Self::TerminalOutput(doc) => doc.result(),
            Self::TerminalRead(doc) => {
                let mut content = Map::new();
                // `--unwrap` emits joined `lines` in place of `rows`; both
                // shapes keep the physical `rowCount` and the fetch-bound
                // `omittedRows` facts, and `--lines` adds its own `tail`
                // accounting — presentation omission is never folded into
                // `omittedRows`.
                match &doc.lines {
                    Some(lines) => {
                        content.insert(
                            "lines".to_owned(),
                            json!(
                                lines
                                    .iter()
                                    .map(|line| {
                                        json!({
                                            "firstRow": line.first_row,
                                            "lastRow": line.last_row,
                                            "text": line.text,
                                            "startsMidLine": line.starts_mid_line,
                                            "endsMidLine": line.ends_mid_line,
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            ),
                        );
                        content.insert("lineCount".to_owned(), json!(doc.line_count));
                    }
                    None => {
                        content.insert(
                            "rows".to_owned(),
                            json!(
                                doc.rows
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
                                    .collect::<Vec<_>>()
                            ),
                        );
                    }
                }
                content.insert("rowCount".to_owned(), json!(doc.row_count));
                if let Some(tail) = &doc.tail {
                    content.insert(
                        "tail".to_owned(),
                        json!({
                            "requested": tail.requested,
                            "omitted": tail.omitted,
                            "entries": if doc.lines.is_some() { "lines" } else { "rows" },
                        }),
                    );
                }
                content.insert("truncated".to_owned(), json!(doc.omitted_rows > 0));
                content.insert("omittedRows".to_owned(), json!(doc.omitted_rows));
                content.insert(
                    "limits".to_owned(),
                    json!({
                        "rows": MAXIMUM_FRAME_ROWS,
                        "textBytes": READ_TEXT_BYTES,
                    }),
                );
                json!({
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
                    "content": Value::Object(content),
                })
            }
            Self::TerminalHistory(doc) => {
                let mut content = Map::new();
                match &doc.lines {
                    Some(lines) => {
                        // History row indexes are u64 — decimal strings like
                        // every large coordinate in a machine document.
                        content.insert(
                            "lines".to_owned(),
                            json!(
                                lines
                                    .iter()
                                    .map(|line| {
                                        json!({
                                            "firstRow": line.first_row.to_string(),
                                            "lastRow": line.last_row.to_string(),
                                            "text": line.text,
                                            "startsMidLine": line.starts_mid_line,
                                            "endsMidLine": line.ends_mid_line,
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            ),
                        );
                        content.insert("lineCount".to_owned(), json!(doc.line_count));
                    }
                    None => {
                        content.insert(
                            "rows".to_owned(),
                            json!(
                                doc.rows
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
                                    .collect::<Vec<_>>()
                            ),
                        );
                    }
                }
                content.insert("rowCount".to_owned(), json!(doc.rows.len()));
                if let Some(tail) = &doc.tail {
                    content.insert(
                        "tail".to_owned(),
                        json!({
                            "requested": tail.requested,
                            "omitted": tail.omitted,
                            "entries": if doc.lines.is_some() { "lines" } else { "rows" },
                        }),
                    );
                }
                content.insert("truncated".to_owned(), json!(doc.omitted_rows > 0));
                content.insert("omittedRows".to_owned(), json!(doc.omitted_rows));
                content.insert(
                    "limits".to_owned(),
                    json!({
                        "rows": MAXIMUM_HISTORY_ROWS,
                        "textBytes": READ_TEXT_BYTES,
                    }),
                );
                json!({
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
                    "content": Value::Object(content),
                })
            }
            Self::TerminalWaitOutput(doc) => {
                // A `--regex` hit adds the extended members — `patternKind`,
                // the bounded `matched` text with its honest truncation flag
                // and complete length, and the fragment-edge flags — while a
                // literal `--match` hit keeps the v1 object byte-identical.
                let mut matched = Map::from_iter([
                    ("excerpt".to_owned(), json!(doc.matched.excerpt)),
                    (
                        "excerptTruncated".to_owned(),
                        json!(doc.matched.excerpt_truncated),
                    ),
                    (
                        "excerptOffset".to_owned(),
                        json!(doc.matched.excerpt_offset.to_string()),
                    ),
                    (
                        "matchOffset".to_owned(),
                        json!(doc.matched.match_offset.to_string()),
                    ),
                    (
                        "rowStart".to_owned(),
                        json!(doc.matched.row_start.to_string()),
                    ),
                    ("rowEnd".to_owned(), json!(doc.matched.row_end.to_string())),
                    ("revision".to_owned(), json!(doc.revision.to_string())),
                    ("generation".to_owned(), json!(doc.generation.to_string())),
                ]);
                if let Some(regex) = &doc.matched.regex {
                    matched.insert("patternKind".to_owned(), json!("regex"));
                    matched.insert(
                        "matchLength".to_owned(),
                        json!(regex.match_length.to_string()),
                    );
                    matched.insert("matched".to_owned(), json!(regex.matched));
                    matched.insert(
                        "matchedTruncated".to_owned(),
                        json!(regex.matched_truncated),
                    );
                    matched.insert("startsMidLine".to_owned(), json!(regex.starts_mid_line));
                    matched.insert("endsMidLine".to_owned(), json!(regex.ends_mid_line));
                }
                let mut result = json!({
                    "terminal": terminal_row_json(&doc.terminal),
                    "matched": true,
                    "match": Value::Object(matched),
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
                });
                if let Some(selection) = doc.selection {
                    result["selection"] =
                        json!({ "source": selection.source.token(), "lines": selection.lines });
                }
                result
            }
            Self::WorktreeList(doc) => json!({
                "directoryId": doc.directory_id,
                "path": doc.path,
                "worktrees": doc
                    .entries
                    .iter()
                    .map(worktree_entry_json)
                    .collect::<Vec<_>>(),
            }),
            Self::LaunchProfileCatalog(doc) => {
                let catalog_revision = doc.catalog_revision.to_string();
                match &doc.selection {
                    LaunchProfileCatalogSelection::List(entries) => json!({
                        "catalogRevision": catalog_revision,
                        "profiles": entries
                            .iter()
                            .map(launch_profile_entry_json)
                            .collect::<Vec<_>>(),
                    }),
                    LaunchProfileCatalogSelection::Profile(profile) => json!({
                        "catalogRevision": catalog_revision,
                        "profile": launch_profile_entry_json(profile),
                    }),
                }
            }
            Self::ApiSnapshot(document) => json!({ "snapshot": document }),
        }
    }
}

/// One catalog entry as its machine-document value — the same member names
/// the wire contract serializes (`profileId`, `name`, `launch.exec/args`),
/// shared by the list and get documents and the text name/value block.
pub(crate) fn launch_profile_entry_json(entry: &LaunchProfileEntry) -> Value {
    json!({
        "profileId": entry.profile_id.as_str(),
        "name": entry.name.as_str(),
        "launch": {
            "exec": entry.launch.executable(),
            "args": entry.launch.arguments(),
        },
    })
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
        "number": row.number,
        "tabCount": row.tab_count,
        "panelCount": row.panel_count,
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
        "number": row.number,
        "panelCount": row.panel_count,
        "label": row.label,
        "pinned": row.pinned,
        "groupId": row.group_id,
        "context": context_json(&row.context),
    })
}

pub(super) fn presentation_json(row: &PresentationRow) -> Value {
    json!({
        "presentationId": row.presentation_id,
        "label": row.label,
        "customLabel": row.custom_label,
        "displayLabel": row.display_label,
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

pub(super) fn terminal_row_json(row: &TerminalRow) -> Value {
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
        for workspace in &snapshot.navigation.workspaces {
            owners.insert(
                workspace.terminal_context_id.as_str(),
                workspace.workspace_id.as_str(),
            );
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

/// A typed target hits identity first; a `@<kind>:<suffix>` selector tails
/// the member ids; only a non-ID, non-selector token becomes an exact
/// case-sensitive label search. Multi-match arms share one epilogue: zero is
/// an empty candidate set, one is the answer, several return every matching
/// id for the bounded ambiguity envelope.
fn resolve<'a, Item, Label: AsRef<str>>(
    mut items: impl Iterator<Item = (&'a str, Label, Item)>,
    target: &Target,
) -> Result<Item, Vec<&'a str>> {
    fn settle<Item, Label>(mut matches: Vec<(&str, Label, Item)>) -> Result<Item, Vec<&str>> {
        match matches.len() {
            0 => Err(Vec::new()),
            1 => Ok(matches.pop().expect("len checked").2),
            _ => Err(matches.into_iter().map(|(id, _label, _item)| id).collect()),
        }
    }
    match target {
        Target::Id(identity) => items
            .find(|(id, _label, _item)| *id == identity.as_str())
            .map(|(_id, _label, item)| item)
            .ok_or_else(Vec::new),
        Target::Label(label) => settle(
            items
                .filter(|(_id, item_label, _item)| item_label.as_ref() == label.as_str())
                .collect(),
        ),
        Target::Selector(_) => {
            let suffix = target.suffix().unwrap_or_default();
            settle(
                items
                    .filter(|(id, _label, _item)| id.ends_with(suffix))
                    .collect(),
            )
        }
    }
}

pub(crate) fn not_found(resource: Resource, target: &Target) -> Failure {
    Failure::new(FailureKind::NotFound).details(target_details(resource, target.text()))
}

pub(super) fn ambiguous(
    resource: Resource,
    target: &Target,
    mut candidates: Vec<String>,
) -> Failure {
    let total = candidates.len();
    candidates.truncate(8);
    Failure::new(FailureKind::Ambiguous).details(Map::from_iter([
        ("resource".to_owned(), json!(resource.as_str())),
        ("target".to_owned(), json!(target.text())),
        ("candidates".to_owned(), json!(candidates)),
        ("totalCount".to_owned(), json!(total)),
    ]))
}

pub(crate) fn resolve_or_fail<'a, Item, Label: AsRef<str>>(
    items: impl Iterator<Item = (&'a str, Label, Item)>,
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

pub(super) fn resolve_workspace<'a>(
    snapshot: &'a Snapshot,
    target: &Target,
) -> Result<&'a Workspace, Failure> {
    resolve_or_fail(
        snapshot.navigation.workspaces.iter().map(|workspace| {
            (
                workspace.workspace_id.as_str(),
                workspace.label.as_str(),
                workspace,
            )
        }),
        target,
        Resource::Workspace,
    )
}

pub(super) fn resolve_project<'a>(
    snapshot: &'a Snapshot,
    target: &Target,
) -> Result<&'a Project, Failure> {
    resolve_or_fail(
        snapshot
            .navigation
            .projects
            .iter()
            .map(|project| (project.project_id.as_str(), project.label.as_str(), project)),
        target,
        Resource::Project,
    )
}

/// Run one snapshot-derived command over a validated snapshot.
pub(crate) fn project(command: &QueryCommand, snapshot: &Snapshot) -> Result<Projection, Failure> {
    let index = ContextIndex::new(snapshot);
    match command {
        QueryCommand::Status => Ok(Projection::Status(status(snapshot))),
        QueryCommand::ProjectList => Ok(Projection::ProjectList(
            snapshot
                .navigation
                .ordered_projects()
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                .into_iter()
                .map(|project| project_row(snapshot, project))
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
            Ok(Projection::ProjectGet(project_doc(snapshot, project)))
        }
        QueryCommand::WorkspaceList { project } => {
            let project_id = project
                .as_ref()
                .map(|target| resolve_project(snapshot, target).map(|project| &project.project_id))
                .transpose()?;
            Ok(Projection::WorkspaceList(
                snapshot
                    .navigation
                    .ordered_workspaces()
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (_, workspace))| {
                        project_id.is_none_or(|id| workspace.project_id.as_ref() == Some(id))
                    })
                    .map(|(position, (_, workspace))| {
                        workspace_row(snapshot, workspace, position + 1)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        QueryCommand::WorkspaceGet(target) => {
            let workspace = resolve_workspace(snapshot, target)?;
            Ok(Projection::WorkspaceGet(workspace_doc(
                snapshot, workspace,
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
                    snapshot.navigation.workspaces.iter().map(|workspace| {
                        let project = workspace
                            .project_id
                            .as_ref()
                            .and_then(|id| snapshot.navigation.project(id));
                        (
                            workspace.workspace_id.as_str(),
                            workspace.label.as_str(),
                            (project, workspace),
                        )
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
                            .enumerate()
                            .map(|(position, tab)| tab_row(tab, position, index.reference(context)))
                    })
                    .collect(),
            ))
        }
        QueryCommand::TabGet(target) => {
            let (context, tab, position) = resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context.tabs.iter().enumerate().map(move |(position, tab)| {
                        (
                            tab.tab_id.as_str(),
                            tab.display_label(position),
                            (context, tab, position),
                        )
                    })
                }),
                target,
                Resource::Tab,
            )?;
            Ok(Projection::TabGet(TabDoc {
                row: tab_row(tab, position, index.reference(context)),
                root: serde_json::to_value(&tab.root)
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            }))
        }
        QueryCommand::PanelList { tab } => {
            let tabs: Vec<(&TerminalContext, &Tab)> = match tab {
                Some(target) => {
                    let resolved = resolve_or_fail(
                        snapshot.terminal_contexts.iter().flat_map(|context| {
                            context.tabs.iter().enumerate().map(move |(position, tab)| {
                                (
                                    tab.tab_id.as_str(),
                                    tab.display_label(position),
                                    (context, tab),
                                )
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
                collect_panels(
                    &tab.root,
                    context,
                    tab,
                    index.reference(context).workspace_id.as_deref(),
                    &mut path,
                    &mut |row, _path| {
                        rows.push(row);
                    },
                );
            }
            Ok(Projection::PanelList(rows))
        }
        QueryCommand::PanelGet(target)
        | QueryCommand::PanelCurrent(PanelQueryTarget::Panel(target)) => {
            // The grammar admits only typed IDs and `@panel:` selectors for
            // panel targets; a label cannot arrive here.
            if let Target::Label(_) = target {
                return Err(Failure::new(FailureKind::InternalFailure));
            }
            let (panel_id, context, tab) = resolve_or_fail(
                panel_entries(snapshot)
                    .into_iter()
                    .map(|(id, context, tab)| (id.as_str(), "", (id, context, tab))),
                target,
                Resource::Panel,
            )?;
            let mut path = Vec::new();
            let mut found = None;
            collect_panels(
                &tab.root,
                context,
                tab,
                index.reference(context).workspace_id.as_deref(),
                &mut path,
                &mut |row, slot_path| {
                    if row.panel_id == panel_id.as_str() {
                        found = Some((row, slot_path.to_vec()));
                    }
                },
            );
            let Some((row, slot_path)) = found else {
                return Err(Failure::new(FailureKind::InternalFailure));
            };
            let Some((primary, children)) = panel_presentations(&tab.root, panel_id.as_str())
            else {
                return Err(Failure::new(FailureKind::InternalFailure));
            };
            Ok(Projection::PanelGet(PanelDoc {
                row,
                context: index.reference(context),
                slot_path,
                primary_presentation: primary,
                child_presentations: children,
            }))
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
        // check` never reach a snapshot projection, and Agent reads are
        // answered by the OBAP exchange, never a snapshot projection.
        // `launch-profile list`/`get` ride their own typed catalog read.
        QueryCommand::PanelProcessInfo(_)
        | QueryCommand::PanelCurrent(PanelQueryTarget::Current | PanelQueryTarget::Client(_))
        | QueryCommand::AgentList { .. }
        | QueryCommand::AgentChanges { .. }
        | QueryCommand::AgentGet(_)
        | QueryCommand::AgentClientGet { .. }
        | QueryCommand::AgentRead { .. }
        | QueryCommand::AgentExplain(_)
        | QueryCommand::AgentWait { .. }
        | QueryCommand::AgentDoctor { .. }
        | QueryCommand::TerminalGet(_)
        | QueryCommand::TerminalOutput { .. }
        | QueryCommand::TerminalRead { .. }
        | QueryCommand::TerminalSourceWait { .. }
        | QueryCommand::TerminalWaitOutput { .. }
        | QueryCommand::WorktreeList { .. }
        | QueryCommand::LaunchProfileList
        | QueryCommand::LaunchProfileGet(_)
        | QueryCommand::ApiSchema { .. }
        | QueryCommand::ConfigCheck
        | QueryCommand::WorkspaceCloseStatus { .. } => {
            Err(Failure::new(FailureKind::InternalFailure))
        }
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
        workspaces: snapshot.navigation.workspaces.len(),
        tabs,
        panels,
        bound_terminals,
    }
}

fn project_directories<'a>(
    snapshot: &'a Snapshot,
    project: &Project,
) -> Vec<&'a orchbee_workbench_protocol::Directory> {
    snapshot
        .navigation
        .directories
        .iter()
        .filter(|directory| {
            snapshot
                .navigation
                .workspaces_in_project(Some(&project.project_id))
                .any(|workspace| {
                    workspace.primary_directory_id.as_ref() == Some(&directory.directory_id)
                })
        })
        .collect()
}

fn project_row(snapshot: &Snapshot, project: &Project) -> ProjectRow {
    ProjectRow {
        project_id: project.project_id.to_string(),
        label: project.label.clone(),
        directory_count: project_directories(snapshot, project).len(),
        workspace_count: snapshot
            .navigation
            .workspaces_in_project(Some(&project.project_id))
            .count(),
    }
}

fn project_doc(snapshot: &Snapshot, project: &Project) -> ProjectDoc {
    ProjectDoc {
        row: project_row(snapshot, project),
        directories: project_directories(snapshot, project)
            .into_iter()
            .map(|directory| DirectoryRow {
                directory_id: directory.directory_id.to_string(),
                worker_id: directory.worker_id.to_string(),
                path: directory.path.as_str().to_owned(),
            })
            .collect(),
        workspaces: snapshot
            .navigation
            .workspaces_in_project(Some(&project.project_id))
            .map(|workspace| WorkspaceRef {
                workspace_id: workspace.workspace_id.to_string(),
                label: workspace.label.clone(),
            })
            .collect(),
    }
}

fn workspace_row(
    snapshot: &Snapshot,
    workspace: &Workspace,
    number: usize,
) -> Result<WorkspaceRow, Failure> {
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    Ok(WorkspaceRow {
        number,
        tab_count: context.tabs.len(),
        panel_count: context.tabs.iter().map(|tab| panel_count(&tab.root)).sum(),
        workspace_id: workspace.workspace_id.as_str().to_owned(),
        label: workspace.label.clone(),
        project_id: workspace.project_id.as_ref().map(ToString::to_string),
        primary_directory_id: workspace
            .primary_directory_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        terminal_context_id: workspace.terminal_context_id.as_str().to_owned(),
    })
}

/// Count structural leaves, never presentations or only bound terminals.
fn panel_count(node: &PanelNode) -> usize {
    match node {
        PanelNode::Panel { .. } => 1,
        PanelNode::Split { children, .. } => {
            children.iter().map(|slot| panel_count(&slot.node)).sum()
        }
    }
}

fn workspace_doc(snapshot: &Snapshot, workspace: &Workspace) -> Result<WorkspaceDoc, Failure> {
    let context_id = workspace.terminal_context_id.as_str();
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id.as_str() == context_id)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    Ok(WorkspaceDoc {
        row: workspace_row(
            snapshot,
            workspace,
            snapshot
                .navigation
                .ordered_workspaces()
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                .iter()
                .position(|(_, candidate)| candidate.workspace_id == workspace.workspace_id)
                .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?
                + 1,
        )?,
        tabs: context
            .tabs
            .iter()
            .enumerate()
            .map(|(position, tab)| TabRef {
                tab_id: tab.tab_id.as_str().to_owned(),
                label: tab.display_label(position).into_owned(),
            })
            .collect(),
    })
}

fn tab_row(tab: &Tab, position: usize, context: ContextRef) -> TabRow {
    TabRow {
        number: position + 1,
        panel_count: panel_count(&tab.root),
        tab_id: tab.tab_id.as_str().to_owned(),
        label: tab.display_label(position).into_owned(),
        pinned: tab.pinned,
        group_id: tab.group_id.as_ref().map(|id| id.as_str().to_owned()),
        context,
    }
}

fn presentation_row(presentation: &Presentation) -> PresentationRow {
    PresentationRow {
        presentation_id: presentation.presentation_id.as_str().to_owned(),
        custom_label: presentation
            .custom_label
            .as_ref()
            .map(|value| value.as_str().to_owned()),
        display_label: presentation.display_label().to_owned(),
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
    workspace_id: Option<&str>,
    path: &mut Vec<String>,
    visit: &mut impl FnMut(PanelRow, &[String]),
) {
    match node {
        PanelNode::Panel {
            right_click,
            panel_id,
            primary_presentation,
            child_presentations,
        } => visit(
            PanelRow {
                workspace_id: workspace_id.map(str::to_owned),
                label: primary_presentation.label.clone(),
                custom_label: primary_presentation
                    .custom_label
                    .as_ref()
                    .map(|label| label.as_str().to_owned()),
                display_label: primary_presentation.display_label().to_owned(),
                right_click: *right_click,
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
                collect_panels(&slot.node, context, tab, workspace_id, path, visit);
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
            right_click: _,
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
pub(super) fn terminal_rows(snapshot: &Snapshot) -> Vec<TerminalRow> {
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
            right_click: _,
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
mod tests;
