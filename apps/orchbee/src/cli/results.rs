//! Pure projection of a validated Workbench `Snapshot` into the CLI result
//! model. Everything here is total over a snapshot the transport already
//! verified; no fabricated fields appear where no authoritative source exists.

use std::collections::HashMap;

use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentGetOutcome, AgentListOutcome, AgentSafeInteger, AgentSessionDetail,
    ObapRequestId,
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
use crate::invocation::{AgentWaitCondition, QueryCommand, Resource, Target};

pub(crate) fn schema_id(command: &QueryCommand) -> &'static str {
    match command {
        QueryCommand::Status => "orchbee.status",
        QueryCommand::AgentList { .. } => "orchbee.agent.list",
        QueryCommand::AgentGet(_) => "orchbee.agent.get",
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
        QueryCommand::TerminalSourceWait { .. } => 3,
        QueryCommand::TerminalOutput { ansi: true, .. } => 2,
        QueryCommand::PanelGet(_) | QueryCommand::PanelCurrent(_) => 3,
        QueryCommand::ApiSnapshot => 4,
        QueryCommand::PanelList { .. } | QueryCommand::TabGet(_) => 2,
        QueryCommand::TerminalGet(_) => 3,
        QueryCommand::AgentWait { .. } | QueryCommand::PanelProcessInfo(_) => 2,
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
    Status(StatusDoc),
    /// The typed `listed` outcome, verbatim from the wire.
    AgentList(AgentListOutcome),
    /// The typed `found` outcome, verbatim from the wire.
    AgentGet(AgentGetOutcome),
    /// The typed `found` outcome carrying session plus explanation.
    AgentExplain(AgentExplainOutcome),
    /// The `agent wait` matched answer: the satisfying session detail
    /// verbatim plus the requested-order matched conditions and the
    /// effective wait echo.
    AgentWait(AgentWaitDoc),
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
    pub(crate) until: Vec<AgentWaitCondition>,
    pub(crate) after_revision: Option<AgentSafeInteger>,
    pub(crate) timeout_ms: Option<u32>,
    pub(crate) observations: u64,
    pub(crate) elapsed_ms: u64,
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

impl Projection {
    /// The `result` member of the machine envelope.
    pub(crate) fn result(&self) -> Value {
        match self {
            // Agent projections serialize the exact wire outcome object —
            // the typed enum is the single serialization truth.
            Self::AgentList(outcome) => json!(outcome),
            Self::AgentGet(outcome) => json!(outcome),
            Self::AgentExplain(outcome) => json!(outcome),
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
                json!({
                    "matchedConditions": doc
                        .matched
                        .iter()
                        .map(|condition| condition.as_str())
                        .collect::<Vec<_>>(),
                    "revision": doc.revision,
                    "session": doc.session,
                    "wait": Value::Object(wait),
                })
            }
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
            Self::PanelProcessInfo(doc) => {
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
                "projectId": doc.project_id,
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
                collect_panels(&tab.root, context, tab, &mut path, &mut |row, _path| {
                    rows.push(row);
                });
            }
            Ok(Projection::PanelList(rows))
        }
        QueryCommand::PanelGet(target) | QueryCommand::PanelCurrent(Some(target)) => {
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
            collect_panels(&tab.root, context, tab, &mut path, &mut |row, slot_path| {
                if row.panel_id == panel_id.as_str() {
                    found = Some((row, slot_path.to_vec()));
                }
            });
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
        | QueryCommand::PanelCurrent(None)
        | QueryCommand::AgentList { .. }
        | QueryCommand::AgentGet(_)
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

    #[test]
    fn automatic_tab_queries_use_context_position_and_numeric_name_collisions_stay_ambiguous() {
        let mut state = snapshot();
        for context in &mut state.terminal_contexts {
            for tab in &mut context.tabs {
                tab.automatic_label = true;
            }
        }
        state.terminal_contexts[0].tabs[0].automatic_label = false;
        state.terminal_contexts[0].tabs[0].label = "2".into();
        let list = project(
            &QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
            &state,
        )
        .unwrap()
        .result();
        assert_eq!(list["tabs"][0]["label"], "2");
        assert_eq!(list["tabs"][1]["label"], "2");
        assert_eq!(list["tabs"][9]["label"], "1");
        for name in ["1", "2"] {
            let error =
                project(&QueryCommand::TabGet(Target::Label(name.into())), &state).unwrap_err();
            assert_eq!(error.kind, FailureKind::Ambiguous);
        }
        let id = state.terminal_contexts[0].tabs[1].tab_id.to_string();
        state.terminal_contexts[0].tabs.rotate_left(1);
        let get = project(&QueryCommand::TabGet(Target::Id(id.clone())), &state)
            .unwrap()
            .result();
        assert_eq!(get["tab"]["label"], "1");
        assert_eq!(get["tab"]["tabId"], id);
        let workspace = project(&QueryCommand::WorkspaceGet(Target::Id(hex(0x40))), &state)
            .unwrap()
            .result();
        assert_eq!(workspace["workspace"]["tabs"][0]["label"], "1");
        assert_eq!(
            project(&QueryCommand::TabGet(Target::Label("dup".into())), &state)
                .unwrap_err()
                .kind,
            FailureKind::NotFound
        );
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

    /// A 32-hex id whose high half and trailing hex tail are controlled —
    /// the selector surface is the tail, so `tid(0, 0xaa)` and
    /// `tid(1, 0xaa)` share tail `aa` while staying distinct identities.
    fn tid(high: u64, tail: u64) -> String {
        format!("{high:016x}{tail:016x}")
    }

    /// Selector fixture: projects `…00aa`/`…10aa`/`…00bb`, workspaces
    /// `…00cc`/`…10cc`/`…00dd`, standalone tabs `…00ee`/`…10ee`, panels
    /// `…00ff`/`…10ff` (bound terminals `…00210`/`…11210`), `…00f1`, a
    /// workspace tab `…00e1` whose split holds `…00f2`/`…10f2`, and a second
    /// workspace tab `…00e3` whose right-deep split holds nine panels ending
    /// in `42` plus one panel `…00f4`.
    fn selector_snapshot() -> Snapshot {
        let leaf = |panel: String, pres: String, terminal: Option<String>| {
            json!({
                "kind": "panel",
                "panelId": panel,
                "primaryPresentation": {
                    "presentationId": pres,
                    "label": "p",
                    "binding": terminal.map(|terminal| json!({
                        "terminalId": terminal,
                        "workerId": WORKER,
                    })),
                },
                "childPresentations": [],
            })
        };
        fn deep(tails: &[u64], seed: &mut u64) -> Value {
            let node = |panel: u64, pres: u64| {
                json!({
                    "kind": "panel",
                    "panelId": tid(0, panel),
                    "primaryPresentation": {
                        "presentationId": tid(0xa, pres),
                        "label": "p",
                        "binding": null,
                    },
                    "childPresentations": [],
                })
            };
            *seed += 1;
            let slot = *seed;
            if tails.len() == 1 {
                return node(tails[0], slot + 0x100);
            }
            json!({
                "kind": "split",
                "splitId": tid(9, slot),
                "orientation": "horizontal",
                "children": [
                    {"slotId": tid(8, slot), "node": node(tails[0], slot + 0x100)},
                    {"slotId": tid(8, slot + 0x100), "node": deep(&tails[1..], seed)},
                ],
            })
        }
        let mut seed = 0_u64;
        let document = json!({
            "kind": "workbench.snapshot",
            "protocolVersion": 1,
            "revision": "9",
            "siteId": tid(0, 0xaa00),
            "navigation": {
                "projects": [
                    {
                        "projectId": tid(0, 0xaa),
                        "label": "p-a",
                        "directories": [],
                        "workspaces": [{
                            "workspaceId": tid(0, 0xcc),
                            "label": "w",
                            "primaryDirectoryId": null,
                            "terminalContextId": tid(0, 0x50),
                        }],
                    },
                    {
                        "projectId": tid(0, 0xbb),
                        "label": "face",
                        "directories": [],
                        "workspaces": [{
                            "workspaceId": tid(0, 0xdd),
                            "label": "w3",
                            "primaryDirectoryId": null,
                            "terminalContextId": tid(0, 0x60),
                        }],
                    },
                    {
                        "projectId": tid(1, 0xaa),
                        "label": "p-b",
                        "directories": [],
                        "workspaces": [{
                            "workspaceId": tid(1, 0xcc),
                            "label": "w",
                            "primaryDirectoryId": null,
                            "terminalContextId": tid(0, 0x51),
                        }],
                    },
                ],
                "standaloneTerminalContextId": tid(0, 0x01),
            },
            "terminalContexts": [
                {
                    "terminalContextId": tid(0, 0x01),
                    "groups": [],
                    "tabs": [
                        {
                            "tabId": tid(0, 0xee),
                            "label": "t",
                            "pinned": false,
                            "groupId": null,
                            "root": {
                                "kind": "split",
                                "splitId": tid(9, 0x90),
                                "orientation": "horizontal",
                                "children": [
                                    {"slotId": tid(8, 0x90), "node": leaf(tid(0, 0xff), tid(0xa, 0x90), Some(tid(0, 0x210)))},
                                    {"slotId": tid(8, 0x91), "node": leaf(tid(1, 0xff), tid(0xa, 0x91), Some(tid(1, 0x1210)))},
                                ],
                            },
                        },
                        {
                            "tabId": tid(1, 0xee),
                            "label": "t",
                            "pinned": false,
                            "groupId": null,
                            "root": leaf(tid(0, 0xf1), tid(0xa, 0xf1), None),
                        },
                    ],
                },
                {
                    "terminalContextId": tid(0, 0x50),
                    "groups": [],
                    "tabs": [{
                        "tabId": tid(0, 0xe8),
                        "label": "w1",
                        "pinned": false,
                        "groupId": null,
                        "root": leaf(tid(0, 0xf8), tid(0xa, 0xf8), None),
                    }],
                },
                {
                    "terminalContextId": tid(0, 0x51),
                    "groups": [],
                    "tabs": [{
                        "tabId": tid(0, 0xe9),
                        "label": "w2",
                        "pinned": false,
                        "groupId": null,
                        "root": leaf(tid(0, 0xf9), tid(0xa, 0xf9), None),
                    }],
                },
                {
                    "terminalContextId": tid(0, 0x60),
                    "groups": [],
                    "tabs": [
                        {
                            "tabId": tid(0, 0xe1),
                            "label": "t3",
                            "pinned": false,
                            "groupId": null,
                            "root": {
                                "kind": "split",
                                "splitId": tid(9, 0x92),
                                "orientation": "horizontal",
                                "children": [
                                    {"slotId": tid(8, 0x92), "node": leaf(tid(0, 0xf2), tid(0xa, 0xf2), None)},
                                    {"slotId": tid(8, 0x93), "node": leaf(tid(1, 0xf2), tid(0xa, 0xf3), None)},
                                ],
                            },
                        },
                        {
                            "tabId": tid(0, 0xe3),
                            "label": "t4",
                            "pinned": false,
                            "groupId": null,
                            "root": {
                                "kind": "split",
                                "splitId": tid(9, 0x94),
                                "orientation": "horizontal",
                                "children": [
                                    {"slotId": tid(8, 0x94), "node": leaf(tid(0, 0xf4), tid(0xa, 0xf4), None)},
                                    {"slotId": tid(8, 0x95), "node": deep(&[0x42, 0x142, 0x242, 0x342, 0x442, 0x542, 0x642, 0x742, 0x842], &mut seed)},
                                ],
                            },
                        },
                    ],
                },
            ],
        });
        parse_snapshot(&serde_json::to_vec(&document).expect("fixture encodes"))
            .expect("selector fixture snapshot validates")
    }

    #[test]
    fn selectors_resolve_unique_tails_to_full_ids() {
        let snapshot = selector_snapshot();
        // One unique-tail hit per selector kind; the resolved document carries
        // the full canonical id, never the selector token.
        let projection = project(
            &QueryCommand::ProjectGet(Target::Selector("@project:bb".into())),
            &snapshot,
        )
        .expect("project selector");
        let Projection::ProjectGet(document) = projection else {
            panic!("project get projection");
        };
        assert_eq!(document.row.project_id, tid(0, 0xbb));

        let projection = project(
            &QueryCommand::WorkspaceGet(Target::Selector("@workspace:dd".into())),
            &snapshot,
        )
        .expect("workspace selector");
        let Projection::WorkspaceGet(document) = projection else {
            panic!("workspace get projection");
        };
        assert_eq!(document.row.workspace_id, tid(0, 0xdd));

        let projection = project(
            &QueryCommand::TabGet(Target::Selector("@tab:e1".into())),
            &snapshot,
        )
        .expect("tab selector");
        let Projection::TabGet(document) = projection else {
            panic!("tab get projection");
        };
        assert_eq!(document.row.tab_id, tid(0, 0xe1));

        let projection = project(
            &QueryCommand::PanelGet(Target::Selector("@panel:f4".into())),
            &snapshot,
        )
        .expect("panel selector");
        let Projection::PanelGet(document) = projection else {
            panic!("panel get projection");
        };
        assert_eq!(document.row.panel_id, tid(0, 0xf4));

        // Terminals resolve over the snapshot's complete bound set.
        let row = terminal_select(&snapshot, &Target::Selector("@terminal:00210".into()))
            .expect("terminal selector");
        assert_eq!(row.terminal_id, tid(0, 0x210));
    }

    #[test]
    fn selectors_match_inside_their_own_kind_only() {
        let snapshot = selector_snapshot();
        // Tail `aa` exists on two projects but no workspace — a foreign-kind
        // selector is a miss, never a cross-kind hit.
        let failure = project(
            &QueryCommand::WorkspaceGet(Target::Selector("@workspace:aa".into())),
            &snapshot,
        )
        .expect_err("cross-kind tail must not resolve");
        assert_eq!(failure.kind, FailureKind::NotFound);
        assert_eq!(details(&failure)["resource"], json!("workspace"));
        assert_eq!(details(&failure)["target"], json!("@workspace:aa"));
        // The same tail the terminals carry (`210`) is not a panel tail.
        let failure = project(
            &QueryCommand::PanelGet(Target::Selector("@panel:210".into())),
            &snapshot,
        )
        .expect_err("terminal tails are not panel candidates");
        assert_eq!(failure.kind, FailureKind::NotFound);
        // A hex-shaped label still resolves as a label; a selector with the
        // same spelling only looks at identifier tails.
        let projection = project(
            &QueryCommand::ProjectGet(Target::Label("face".into())),
            &snapshot,
        )
        .expect("hex-looking label");
        let Projection::ProjectGet(document) = projection else {
            panic!("project get projection");
        };
        assert_eq!(document.row.project_id, tid(0, 0xbb));
        let failure = project(
            &QueryCommand::ProjectGet(Target::Selector("@project:face".into())),
            &snapshot,
        )
        .expect_err("no project id ends in `face`");
        assert_eq!(failure.kind, FailureKind::NotFound);
    }

    #[test]
    fn selector_ambiguity_reports_bounded_candidates_and_the_true_total() {
        let snapshot = selector_snapshot();
        // Two-member collisions per kind, in canonical traversal order.
        for (command, resource, expected) in [
            (
                QueryCommand::ProjectGet(Target::Selector("@project:aa".into())),
                "project",
                vec![tid(0, 0xaa), tid(1, 0xaa)],
            ),
            (
                QueryCommand::WorkspaceGet(Target::Selector("@workspace:cc".into())),
                "workspace",
                vec![tid(0, 0xcc), tid(1, 0xcc)],
            ),
            (
                QueryCommand::TabGet(Target::Selector("@tab:ee".into())),
                "tab",
                vec![tid(0, 0xee), tid(1, 0xee)],
            ),
            (
                QueryCommand::PanelGet(Target::Selector("@panel:ff".into())),
                "panel",
                vec![tid(0, 0xff), tid(1, 0xff)],
            ),
        ] {
            let failure = project(&command, &snapshot).expect_err("colliding selector tail");
            assert_eq!(failure.kind, FailureKind::Ambiguous, "{resource}");
            assert_eq!(details(&failure)["resource"], json!(resource));
            assert_eq!(details(&failure)["totalCount"], json!(2));
            assert_eq!(candidates(&failure), expected, "{resource}");
        }
        // Terminals share the same rule over bound rows.
        let failure = terminal_select(&snapshot, &Target::Selector("@terminal:210".into()))
            .expect_err("colliding terminal tail");
        assert_eq!(failure.kind, FailureKind::Ambiguous);
        assert_eq!(candidates(&failure), vec![tid(0, 0x210), tid(1, 0x1210)],);
        // Nine panels end in `42`: the envelope caps the candidate list at
        // eight while the total stays truthful.
        let failure = project(
            &QueryCommand::PanelGet(Target::Selector("@panel:42".into())),
            &snapshot,
        )
        .expect_err("nine-way tail collision");
        assert_eq!(failure.kind, FailureKind::Ambiguous);
        assert_eq!(details(&failure)["totalCount"], json!(9));
        assert_eq!(
            candidates(&failure),
            [0x42, 0x142, 0x242, 0x342, 0x442, 0x542, 0x642, 0x742]
                .iter()
                .map(|&tail| tid(0, tail))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn selector_misses_are_not_found_without_candidates() {
        let snapshot = selector_snapshot();
        for (command, resource, target) in [
            (
                QueryCommand::TabGet(Target::Selector("@tab:999".into())),
                "tab",
                "@tab:999",
            ),
            (
                QueryCommand::PanelGet(Target::Selector("@panel:999".into())),
                "panel",
                "@panel:999",
            ),
        ] {
            let failure = project(&command, &snapshot).expect_err("selector miss");
            assert_eq!(failure.kind, FailureKind::NotFound, "{resource}");
            assert_eq!(details(&failure)["resource"], json!(resource));
            assert_eq!(details(&failure)["target"], json!(target));
            assert!(details(&failure).get("candidates").is_none());
        }
        let failure = terminal_select(&snapshot, &Target::Selector("@terminal:999".into()))
            .expect_err("terminal selector miss");
        assert_eq!(failure.kind, FailureKind::NotFound);
        // A full 32-hex id is still an exact identity lookup, unchanged.
        let projection = project(&QueryCommand::TabGet(Target::Id(tid(0, 0xe1))), &snapshot)
            .expect("full id resolves");
        let Projection::TabGet(document) = projection else {
            panic!("tab get projection");
        };
        assert_eq!(document.row.tab_id, tid(0, 0xe1));
    }

    #[test]
    fn selector_filters_share_operand_semantics() {
        let snapshot = selector_snapshot();
        // `--project` on `workspace list` resolves the same suffix grammar.
        let projection = project(
            &QueryCommand::WorkspaceList {
                project: Some(Target::Selector("@project:bb".into())),
            },
            &snapshot,
        )
        .expect("filtered workspace list");
        let Projection::WorkspaceList(rows) = projection else {
            panic!("workspace list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.workspace_id.clone())
                .collect::<Vec<_>>(),
            vec![tid(0, 0xdd)],
        );
        // `--workspace` on `tab list` and `--tab` on `panel list` likewise.
        let projection = project(
            &QueryCommand::TabList {
                workspace: Some(Target::Selector("@workspace:dd".into())),
                standalone: false,
            },
            &snapshot,
        )
        .expect("filtered tab list");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.tab_id.clone())
                .collect::<Vec<_>>(),
            vec![tid(0, 0xe1), tid(0, 0xe3)],
        );
        let projection = project(
            &QueryCommand::PanelList {
                tab: Some(Target::Selector("@tab:e1".into())),
            },
            &snapshot,
        )
        .expect("filtered panel list");
        let Projection::PanelList(rows) = projection else {
            panic!("panel list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.panel_id.clone())
                .collect::<Vec<_>>(),
            vec![tid(0, 0xf2), tid(1, 0xf2)],
        );
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
    #[cfg(any(unix, windows))]
    mod terminal_read {
        use super::super::{Projection, ReadPresentation, TerminalRow, terminal_read_doc};
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
            let doc = terminal_read_doc(terminal(), &frame, ReadPresentation::default())
                .expect("document");
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
            let doc = terminal_read_doc(terminal(), &frame, ReadPresentation::default())
                .expect("document");
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
            let doc = terminal_read_doc(terminal(), &frame, ReadPresentation::default())
                .expect("document");
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
            let doc = terminal_read_doc(terminal(), &frame, ReadPresentation::default())
                .expect("document");
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
            let doc = terminal_read_doc(terminal(), &frame, ReadPresentation::default())
                .expect("document");
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
                unwrap: false,
                lines: None,
            };
            let body = render_text(|out| present::render(out, &projection, &command, Locale::EnUs))
                .expect("text output fits the preparation bound");
            let body = String::from_utf8(body).expect("utf8 output");
            assert!(body.contains("head\\u{001b}row"));
            assert!(!body.contains("tail-marker-row"));
            assert!(body.ends_with("… 2 more rows truncated at the read limit\n"));
        }

        /// A narrow-column frame of short text rows with explicit wrap bits:
        /// bit 0 `intoNext`, bit 1 `fromPrevious`.
        fn narrow_frame(rows: &[(u16, u8, &str)]) -> TerminalTextFrame {
            let blank: Arc<str> = Arc::from("");
            frame_with_columns(
                rows.iter()
                    .map(|(index, flags, text)| {
                        let mut cells = vec![cell(&Arc::from(*text), 0)];
                        cells.extend((1..3).map(|_| cell(&blank, 0)));
                        TerminalTextRow {
                            index: *index,
                            flags: *flags,
                            cells,
                        }
                    })
                    .collect(),
                3,
            )
        }

        /// `--unwrap`: only verified soft-wrap chains join — hard breaks and
        /// empty rows stay their own logical lines, physical row indexes
        /// span the join, and a window opening or ending inside a wrap marks
        /// the fragment instead of claiming a complete line.
        #[test]
        fn unwrap_joins_wraps_and_marks_fragments() {
            let frame = narrow_frame(&[
                (0, 2, "tail of wrapped"), // continues from an unfetched row
                (1, 0, "hard"),
                (2, 1, "soft-"),
                (3, 3, "wrap-"),
                (4, 2, "done"),
                (5, 0, ""),
                (6, 1, "open"),
            ]);
            let doc = terminal_read_doc(
                terminal(),
                &frame,
                ReadPresentation {
                    unwrap: true,
                    tail: None,
                },
            )
            .expect("document");
            // Physical facts stay: 7 rows applied, 0 omitted.
            assert_eq!(doc.row_count, 7);
            assert_eq!(doc.rows.len(), 7);
            let lines = doc.lines.as_ref().expect("lines emitted");
            assert_eq!(doc.line_count, Some(5));
            assert_eq!(
                lines
                    .iter()
                    .map(|line| (
                        line.first_row,
                        line.last_row,
                        line.text.as_str(),
                        line.starts_mid_line,
                        line.ends_mid_line,
                    ))
                    .collect::<Vec<_>>(),
                [
                    (0, 0, "tail of wrapped", true, false),
                    (1, 1, "hard", false, false),
                    (2, 4, "soft-wrap-done", false, false),
                    (5, 5, "", false, false),
                    (6, 6, "open", false, true),
                ]
            );
            let content = &Projection::TerminalRead(doc).result()["content"];
            assert_eq!(content["rowCount"], 7);
            assert_eq!(content["lineCount"], 5);
            assert!(content.get("rows").is_none());
            let emitted = content["lines"].as_array().expect("lines array");
            assert_eq!(emitted[0]["firstRow"], 0);
            assert_eq!(emitted[0]["startsMidLine"], true);
            assert_eq!(emitted[4]["endsMidLine"], true);
        }

        /// `--lines` without `--unwrap` tails physical rows and reports the
        /// presentation omission separately from the budget's `omittedRows`.
        #[test]
        fn lines_tails_physical_rows() {
            let frame = narrow_frame(&[(0, 0, "a"), (1, 0, "b"), (2, 0, "c")]);
            let doc = terminal_read_doc(
                terminal(),
                &frame,
                ReadPresentation {
                    unwrap: false,
                    tail: Some(2),
                },
            )
            .expect("document");
            assert_eq!(doc.row_count, 3);
            assert_eq!(
                doc.rows.iter().map(|row| row.index).collect::<Vec<_>>(),
                [1, 2]
            );
            let tail = doc.tail.as_ref().expect("tail accounting");
            assert_eq!((tail.requested, tail.omitted), (2, 1));
            let content = &Projection::TerminalRead(doc).result()["content"];
            assert_eq!(content["tail"]["requested"], 2);
            assert_eq!(content["tail"]["omitted"], 1);
            assert_eq!(content["tail"]["entries"], "rows");
            assert_eq!(content["rowCount"], 3);
            assert_eq!(content["omittedRows"], 0);
            // A tail wider than the window omits nothing.
            let doc = terminal_read_doc(
                terminal(),
                &frame,
                ReadPresentation {
                    unwrap: false,
                    tail: Some(9),
                },
            )
            .expect("document");
            assert_eq!(doc.rows.len(), 3);
            assert_eq!(doc.tail.expect("tail").omitted, 0);
        }

        /// `--unwrap --lines`: the tail counts logical lines, including the
        /// observed edge fragments, never the retained-history total.
        #[test]
        fn lines_tails_logical_lines() {
            let frame = narrow_frame(&[
                (0, 1, "wrap "),
                (1, 2, "head"),
                (2, 0, "mid"),
                (3, 0, "last"),
            ]);
            let doc = terminal_read_doc(
                terminal(),
                &frame,
                ReadPresentation {
                    unwrap: true,
                    tail: Some(2),
                },
            )
            .expect("document");
            let lines = doc.lines.as_ref().expect("lines");
            assert_eq!(doc.line_count, Some(3));
            assert_eq!(
                lines
                    .iter()
                    .map(|line| line.text.as_str())
                    .collect::<Vec<_>>(),
                ["mid", "last"]
            );
            let tail = doc.tail.as_ref().expect("tail");
            assert_eq!((tail.requested, tail.omitted), (2, 1));
            let content = &Projection::TerminalRead(doc).result()["content"];
            assert_eq!(content["tail"]["entries"], "lines");
            assert_eq!(content["tail"]["omitted"], 1);
            assert_eq!(content["lineCount"], 3);
        }
    }

    /// `terminal wait-output` matching: soft-wrapped rows join into logical
    /// lines before the literal substring check, the excerpt always contains
    /// the complete pattern within 4096 UTF-8 bytes on char boundaries, and
    /// the observed-window projection reports the honest truncation flag.
    #[cfg(any(unix, windows))]
    mod terminal_wait {
        use super::super::{
            MAXIMUM_EXCERPT_BYTES, TerminalRow, WaitMatch, WaitMatcher, WaitObservation,
            match_rows, match_window, terminal_wait_output_doc,
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
                &WaitMatcher::Literal("abcdef"),
            )
            .expect("joined line matches");
            assert_eq!(hit.row_start, 5);
            assert_eq!(hit.row_end, 6);
            assert_eq!(hit.excerpt, "prompt$ echo abcdef done");
            assert!(!hit.excerpt_truncated);
            assert_eq!(hit.match_offset, 13);
            assert_eq!(hit.excerpt_offset, 0);
        }

        #[test]
        fn physical_source_does_not_join_wraps_and_keeps_original_edge_facts() {
            let rows = [(5, "HEAD", false, true), (6, "TAIL", true, false)];
            let matcher = WaitMatcher::Literal("HEADTAIL");
            assert!(match_rows(rows, &matcher, false).is_none());
            let joined = match_rows(rows, &matcher, true).unwrap();
            assert_eq!((joined.row_start, joined.row_end), (5, 6));
            let regex = regex::Regex::new("^HEAD$").unwrap();
            let physical = match_rows(rows, &WaitMatcher::Regex(&regex), false).unwrap();
            assert_eq!((physical.row_start, physical.row_end), (5, 5));
            let detail = physical.regex.unwrap();
            assert!(!detail.starts_mid_line && detail.ends_mid_line);
            assert!(
                match_rows(
                    [],
                    &WaitMatcher::Regex(&regex::Regex::new("^$").unwrap()),
                    true
                )
                .is_none()
            );
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
                &WaitMatcher::Literal("hit"),
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
            assert!(match_window(&window, &WaitMatcher::Literal("foo")).is_none());
            assert!(match_window(&window, &WaitMatcher::Literal("a.b")).is_some());
            assert!(match_window(&window, &WaitMatcher::Literal("a[x]b")).is_none());
            assert!(match_window(&window, &WaitMatcher::Literal("a.*b")).is_none());
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
                &WaitMatcher::Literal("earlier marker"),
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
                regex: None,
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
                        timeout_ms: Some(30_000),
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

        fn program(pattern: &str) -> regex::Regex {
            regex::Regex::new(pattern).expect("regex compiles")
        }

        /// `--regex` searches the same joined fragments: the hit is the
        /// engine's leftmost match inside the first matching fragment, and
        /// the extended members carry the complete length, the bounded
        /// matched text, and the honest fragment-edge flags.
        #[test]
        fn regex_matches_joined_fragment() {
            let program = program("[a-z]+ done");
            let hit = match_window(
                &window(
                    vec![
                        row(5, true, false, "prompt$ echo abc"),
                        row(6, false, true, "def done"),
                        row(7, false, false, "unrelated"),
                    ],
                    8,
                ),
                &WaitMatcher::Regex(&program),
            )
            .expect("joined fragment matches");
            assert_eq!((hit.row_start, hit.row_end), (5, 6));
            assert_eq!(hit.match_offset, 13);
            let detail = hit.regex.expect("regex detail");
            assert_eq!(detail.match_length, 11);
            assert_eq!(detail.matched, "abcdef done");
            assert!(!detail.matched_truncated);
            assert!(!detail.starts_mid_line && !detail.ends_mid_line);
        }

        /// A window-edge fragment flags both ends: `^`/`$` anchors bind only
        /// the observed fragment, never unseen text outside it.
        #[test]
        fn regex_fragment_edges_flagged() {
            let program = program("^earlier marker$");
            let hit = match_window(
                &window(
                    vec![
                        row(9, true, true, "earlier "),
                        row(10, true, true, "marker"),
                    ],
                    12,
                ),
                &WaitMatcher::Regex(&program),
            )
            .expect("anchored fragment matches");
            let detail = hit.regex.expect("regex detail");
            assert!(detail.starts_mid_line && detail.ends_mid_line);
            assert_eq!(hit.excerpt, "earlier marker");
        }

        /// An empty observed line is a real fragment: `^$` matches it at
        /// offset 0 with length 0 — zero-length matches are hits.
        #[test]
        fn regex_empty_match_on_empty_line() {
            let program = program("^$");
            let hit = match_window(
                &window(
                    vec![
                        row(0, false, false, "content"),
                        row(1, false, false, ""),
                        row(2, false, false, "later"),
                    ],
                    3,
                ),
                &WaitMatcher::Regex(&program),
            )
            .expect("empty line matches ^$");
            assert_eq!((hit.row_start, hit.row_end), (1, 1));
            assert_eq!(hit.match_offset, 0);
            let detail = hit.regex.expect("regex detail");
            assert_eq!(detail.match_length, 0);
            assert_eq!(detail.matched, "");
            assert!(!detail.matched_truncated);
        }

        /// Unicode text: offsets are UTF-8 byte offsets into the joined
        /// fragment, and a first-fragment hit wins ordering.
        #[test]
        fn regex_unicode_offsets_and_first_fragment() {
            let program = program("目标\\d+");
            let hit = match_window(
                &window(
                    vec![
                        row(0, false, false, "前面目标42后面"),
                        row(1, false, false, "目标99 second"),
                    ],
                    2,
                ),
                &WaitMatcher::Regex(&program),
            )
            .expect("first fragment wins");
            assert_eq!(hit.row_start, 0);
            // "前面" is 6 bytes; the match starts at byte 6.
            assert_eq!(hit.match_offset, 6);
            let detail = hit.regex.expect("regex detail");
            assert_eq!(detail.matched, "目标42");
            assert_eq!(detail.match_length, 8);
        }

        /// A regex match longer than the excerpt bound reports its complete
        /// length honestly, a bounded `matched` prefix, and an excerpt that
        /// leads with the match — never an underflow or a false whole.
        #[test]
        fn regex_oversized_match_is_bounded() {
            let program = program("x+");
            let line = format!("pre{}", "x".repeat(8000));
            let hit = match_window(
                &window(vec![row(0, false, false, &line)], 1),
                &WaitMatcher::Regex(&program),
            )
            .expect("long match hits");
            assert_eq!(hit.match_offset, 3);
            let detail = hit.regex.expect("regex detail");
            assert_eq!(detail.match_length, 8000);
            assert!(detail.matched_truncated);
            assert!(detail.matched.len() <= MAXIMUM_EXCERPT_BYTES);
            assert!(detail.matched.bytes().all(|byte| byte == b'x'));
            // The excerpt leads with the match start, explicitly truncated.
            assert!(hit.excerpt_truncated);
            assert_eq!(hit.excerpt_offset, 3);
            assert!(hit.excerpt.len() <= MAXIMUM_EXCERPT_BYTES);
        }
    }
}
