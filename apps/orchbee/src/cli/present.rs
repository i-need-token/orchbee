//! Text presentation for query results: fixed-width tables with escaped,
//! width-bounded labels, and field blocks for `get` commands. Everything is
//! written piece by piece into the bounded preparation sink, so an oversized
//! render aborts at the 8 MiB bound instead of assembling the full body.

use serde_json::Value;
#[cfg(windows)]
mod agent_screen;

#[cfg(not(windows))]
use orchbee_agent_observation_protocol::AgentListOutcome;
use orchbee_agent_observation_protocol::{
    AgentExplainOutcome, AgentGetOutcome, AgentSessionDetail, AgentSessionExplanation,
};
use orchbee_workbench_protocol::{
    LaunchProfileEntry, WorkbenchTabCloseBlockedReason, WorkbenchTabClosePhase,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, WorkbenchWorkspaceCloseMember,
    WorkbenchWorkspaceCloseMemberStatus, WorkbenchWorkspaceCloseReadOutcome, WorktreeEntry,
    WorktreeEntryText,
};
use unicode_segmentation::UnicodeSegmentation;

use super::config::CheckReport;
use super::escape::{display_width, escape, quoted, table_label, table_label_width, unquoted};
use super::output::{Bounded, TooLarge};
use super::results::{
    AgentWaitDoc, ContextRef, LaunchProfileCatalogSelection, PanelDoc, PresentationRow, Projection,
    StatusDoc, TerminalRow, hex_encode,
};
use crate::invocation::QueryCommand;
use crate::localization::{CliLabels, Locale};

/// Render one table under the bound: header plus aligned rows. Column widths
/// are measured on per-row cells — the only intermediate allocation is one
/// row at a time, never the whole body. A table with no rows is just its
/// header — an empty result set is a successful answer.
fn table<Row>(
    out: &mut Bounded,
    headers: &[&str],
    rows: &[Row],
    cells: impl Fn(&Row) -> Vec<String>,
) -> Result<(), TooLarge> {
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in rows {
        for (index, cell) in cells(row).iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell));
        }
    }
    // Full IDs remain copyable. Wide tables degrade to labeled records instead
    // of pushing the important columns beyond an ordinary 80-cell terminal.
    if widths.iter().sum::<usize>() + headers.len().saturating_sub(1) * 2 > 80 && !rows.is_empty() {
        for (index, row) in rows.iter().enumerate() {
            if index > 0 {
                out.push("\n")?;
            }
            for (header, cell) in headers.iter().zip(cells(row)) {
                field(out, header, &cell)?;
            }
        }
        return Ok(());
    }
    let render_row = |cells: &[String], out: &mut Bounded| -> Result<(), TooLarge> {
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                out.push("  ")?;
            }
            out.push(cell)?;
            if index + 1 < cells.len() {
                let padding = widths[index].saturating_sub(display_width(cell));
                out.push(&" ".repeat(padding))?;
            }
        }
        out.push("\n")
    };
    render_row(
        &headers
            .iter()
            .map(|header| (*header).to_owned())
            .collect::<Vec<_>>(),
        out,
    )?;
    for row in rows {
        render_row(&cells(row), out)?;
    }
    Ok(())
}

/// Render a compact, localized list table. Callers reserve the full width of
/// selector cells and bound all display labels before they reach this sink.
pub(super) fn compact_table<Row>(
    out: &mut Bounded,
    headers: &[&str],
    widths: &[usize],
    rows: &[Row],
    cells: impl Fn(&Row) -> Vec<String>,
) -> Result<(), TooLarge> {
    let render_row = |cells: &[String], out: &mut Bounded| -> Result<(), TooLarge> {
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                out.push("  ")?;
            }
            out.push(cell)?;
            if index + 1 < cells.len() {
                out.push(&" ".repeat(widths[index].saturating_sub(display_width(cell))))?;
            }
        }
        out.push("\n")
    };
    render_row(
        &headers
            .iter()
            .map(|header| (*header).to_owned())
            .collect::<Vec<_>>(),
        out,
    )?;
    for row in rows {
        render_row(&cells(row), out)?;
    }
    Ok(())
}

fn workspace_list(
    out: &mut Bounded,
    rows: &[super::results::WorkspaceRow],
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    const NAME_WIDTH: usize = 18;
    const PROJECT_WIDTH: usize = 14;
    let handle_width = rows
        .iter()
        .map(|row| display_width(&row.handle))
        .max()
        .unwrap_or(0)
        .max(display_width(text.workspace_headers[0]));
    let widths = [handle_width, NAME_WIDTH, PROJECT_WIDTH];
    compact_table(out, &text.workspace_headers, &widths, rows, |row| {
        let project = match (&row.project_id, &row.project_label) {
            (None, _) => locale.cli_ungrouped(),
            (Some(_), Some(label)) => label.as_str(),
            (Some(_), None) => "-",
        };
        vec![
            row.handle.clone(),
            table_label_width(&row.label, NAME_WIDTH),
            table_label_width(project, PROJECT_WIDTH),
        ]
    })
}

fn tab_list(
    out: &mut Bounded,
    rows: &[super::results::TabRow],
    text: &CliLabels,
) -> Result<(), TooLarge> {
    const NAME_WIDTH: usize = 18;
    const PANELS_WIDTH: usize = 6;
    const WORKSPACE_WIDTH: usize = 13;
    let handle_width = rows
        .iter()
        .map(|row| display_width(&row.handle))
        .max()
        .unwrap_or(0)
        .max(display_width(text.tab_headers[0]));
    let widths = [handle_width, NAME_WIDTH, PANELS_WIDTH, WORKSPACE_WIDTH];
    compact_table(out, &text.tab_headers, &widths, rows, |row| {
        let owner = table_label_width(&row.context.workspace_label, WORKSPACE_WIDTH);
        vec![
            row.handle.clone(),
            table_label_width(&row.label, NAME_WIDTH),
            row.panel_count.to_string(),
            owner,
        ]
    })
}

/// Label/value rows aligned on the widest label.
fn aligned(out: &mut Bounded, rows: &[(&str, String)]) -> Result<(), TooLarge> {
    let width = rows
        .iter()
        .map(|(label, _)| display_width(label))
        .max()
        .unwrap_or(0);
    for (label, value) in rows {
        out.write(format_args!(
            "{label}{}{value}\n",
            " ".repeat(width - display_width(label) + 2)
        ))?;
    }
    Ok(())
}

fn field(out: &mut Bounded, name: &str, value: &str) -> Result<(), TooLarge> {
    let padding = 14_usize.saturating_sub(display_width(name)).max(2);
    out.write(format_args!("{name}{}{value}\n", " ".repeat(padding)))
}

fn context_text(context: &ContextRef) -> String {
    format!("workspace {}", context.workspace_id)
}

fn presentation_cells(row: &PresentationRow) -> Vec<String> {
    vec![
        row.presentation_id.clone(),
        table_label(&row.display_label),
        row.terminal_id.clone().unwrap_or_else(|| "-".to_owned()),
        row.worker_id.clone().unwrap_or_else(|| "-".to_owned()),
    ]
}

/// The human terminal table: location handles and the path are never cut;
/// only the title, Agent and state labels shrink toward an 80-cell line.
fn terminal_table(
    out: &mut Bounded,
    text: &CliLabels,
    doc: &super::results::TerminalListDocument,
    locale: Locale,
) -> Result<(), TooLarge> {
    const MAXIMUM_WIDTH: usize = 80;
    const LABEL_LIMITS: [(usize, usize); 3] = [(1, 24), (2, 14), (3, 12)];
    const CWD: usize = 6;
    let headers = &text.terminal_list_headers;
    let fallback = terminal_handles(&doc.rows);
    let rows: Vec<Vec<String>> = doc
        .rows
        .iter()
        .enumerate()
        .map(|(index, _row)| terminal_list_cells(&fallback[index], doc.details.get(index), locale))
        .collect();
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for cells in &rows {
        for (column, cell) in cells.iter().enumerate() {
            let limit = LABEL_LIMITS
                .iter()
                .find(|(label, _)| *label == column)
                .map_or(usize::MAX, |(_, limit)| *limit);
            widths[column] = widths[column].max(terminal_cell_width(cell).min(limit));
        }
    }
    // The path is the last column and is never truncated; it only counts
    // toward the label budget up to a short visible prefix.
    let fixed = widths[CWD].min(24);
    let mut total = widths[..CWD].iter().sum::<usize>() + fixed + (headers.len() - 1) * 2;
    for (column, _) in LABEL_LIMITS {
        if total <= MAXIMUM_WIDTH {
            break;
        }
        let reduction = widths[column]
            .saturating_sub(display_width(headers[column]))
            .min(total - MAXIMUM_WIDTH);
        widths[column] -= reduction;
        total -= reduction;
    }

    let render_row = |cells: &[String], out: &mut Bounded| -> Result<(), TooLarge> {
        for (index, (cell, width)) in cells.iter().zip(&widths).enumerate() {
            if index > 0 {
                out.push("  ")?;
            }
            let cell = if LABEL_LIMITS.iter().any(|(label, _)| *label == index) {
                truncate_terminal_cell(cell, *width)
            } else {
                unquoted(cell)
            };
            out.push(&cell)?;
            if index + 1 < cells.len() {
                out.push(&" ".repeat(width.saturating_sub(display_width(&cell))))?;
            }
        }
        out.push("\n")
    };

    render_row(
        &headers
            .iter()
            .map(|header| (*header).to_owned())
            .collect::<Vec<_>>(),
        out,
    )?;
    for cells in &rows {
        render_row(cells, out)?;
    }
    Ok(())
}

fn terminal_list_cells(
    fallback_handle: &str,
    details: Option<&super::results::TerminalListDetails>,
    locale: Locale,
) -> Vec<String> {
    let empty = super::results::TerminalListDetails::default();
    let details = details.unwrap_or(&empty);
    let state = details
        .state
        .map(|state| locale.cli_terminal_list_state(state))
        .unwrap_or("-");
    let placement = details.placement.as_ref();
    vec![
        placement.map_or_else(|| fallback_handle.to_owned(), |p| p.handle.clone()),
        details
            .title
            .as_deref()
            .map_or_else(|| "-".to_owned(), terminal_title),
        details.agent.clone().unwrap_or_else(|| "-".to_owned()),
        state.to_owned(),
        placement.map_or_else(|| "-".to_owned(), |p| p.workspace.clone()),
        placement.map_or_else(|| "-".to_owned(), |p| p.panel.clone()),
        details.cwd.clone().unwrap_or_else(|| "-".to_owned()),
    ]
}

/// A title that is only the launched program's absolute path shows the
/// program name; every other title is kept as the terminal set it.
fn terminal_title(title: &str) -> String {
    let absolute = title.starts_with('/')
        || title.starts_with("\\\\")
        || matches!(title.as_bytes(), [drive, b':', b'\\' | b'/', ..] if drive.is_ascii_alphabetic());
    let name = title.rsplit(['\\', '/']).next().unwrap_or(title);
    match name.rsplit_once('.') {
        Some((stem, extension))
            if absolute
                && !stem.is_empty()
                && ["exe", "com", "cmd", "bat", "ps1"]
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known)) =>
        {
            stem.to_owned()
        }
        _ => title.to_owned(),
    }
}

/// Print stable compact identities when no snapshot placement was attached.
fn terminal_handles(rows: &[TerminalRow]) -> Vec<String> {
    super::handles::stable(
        "terminal",
        &rows
            .iter()
            .map(|r| r.terminal_id.as_str())
            .collect::<Vec<_>>(),
    )
}

pub(super) fn truncate_terminal_cell(value: &str, width: usize) -> String {
    if terminal_cell_width(value) <= width {
        return unquoted(value);
    }
    if width == 0 {
        return String::new();
    }

    let mut rendered = String::new();
    let mut used = 0;
    for grapheme in value.graphemes(true) {
        let fragment = unquoted(grapheme);
        let fragment_width = display_width(&fragment);
        if used + fragment_width > width.saturating_sub(1) {
            rendered.push('…');
            return rendered;
        }
        rendered.push_str(&fragment);
        used += fragment_width;
    }
    rendered
}

fn terminal_cell_width(value: &str) -> usize {
    value
        .graphemes(true)
        .map(|grapheme| display_width(&unquoted(grapheme)))
        .sum()
}

/// Non-UTF-8 values render as `hex:`-prefixed lowercase hex; UTF-8 paths and
/// refs pass through the same escaping as every other user-controlled value.
fn worktree_path(value: &WorktreeEntryText) -> String {
    match value {
        WorktreeEntryText::Utf8(text) => super::escape::native_path(text),
        WorktreeEntryText::Bytes(bytes) => format!("hex:{}", hex_encode(bytes)),
    }
}

fn worktree_text(value: &WorktreeEntryText) -> String {
    match value {
        WorktreeEntryText::Utf8(text) => quoted(text),
        WorktreeEntryText::Bytes(bytes) => format!("hex:{}", hex_encode(bytes)),
    }
}

fn worktree_state(entry: &WorktreeEntry) -> String {
    let mut flags = Vec::new();
    if entry.bare {
        flags.push("bare");
    }
    if entry.detached {
        flags.push("detached");
    }
    if entry.locked {
        flags.push("locked");
    }
    if entry.prunable {
        flags.push("prunable");
    }
    if flags.is_empty() {
        "-".to_owned()
    } else {
        flags.join(",")
    }
}

/// One catalog row: the minted id, the escaped name, and the launch spelled
/// as the exec plus its verbatim arguments on one escaped cell.
fn launch_profile_cells(entry: &LaunchProfileEntry) -> Vec<String> {
    let mut launch = entry.launch.executable().to_owned();
    for argument in entry.launch.arguments() {
        launch.push(' ');
        launch.push_str(argument);
    }
    vec![
        entry.profile_id.as_str().to_owned(),
        table_label(entry.name.as_str()),
        escape(&launch),
    ]
}

fn worktree_cells(entry: &WorktreeEntry) -> Vec<String> {
    vec![
        worktree_path(&entry.path),
        entry.head.clone().unwrap_or_else(|| "-".to_owned()),
        entry
            .branch
            .as_ref()
            .map(worktree_text)
            .unwrap_or_else(|| "-".to_owned()),
        worktree_state(entry),
    ]
}

/// `workspace close-status`: `not-closing` is the honest empty answer; an
/// `available` record prints operation identity, the lifecycle status, the
/// aggregate counts, and the captured members in frozen admission order.
fn workspace_close_status(
    out: &mut Bounded,
    outcome: &WorkbenchWorkspaceCloseReadOutcome,
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    let WorkbenchWorkspaceCloseReadOutcome::Available {
        close_request_id,
        workspace_id,
        terminal_context_id,
        attempt_generation,
        status,
        counts,
        members,
    } = outcome
    else {
        return out.push(&format!("{}\n", text.close_not_closing));
    };
    out.push(locale.cli_close_status_verdict(close_status_token(status)))?;
    field(out, text.close_request_id, close_request_id.as_str())?;
    field(out, text.workspace_id, workspace_id.as_str())?;
    field(out, text.terminal_context, terminal_context_id.as_str())?;
    field(
        out,
        text.attempt_generation,
        &attempt_generation.value().to_string(),
    )?;
    field(out, text.status, &close_status_text(status))?;
    field(
        out,
        text.counts,
        &format!(
            "closing={} cleanup-blocked={} outcome-unknown={} disposed-awaiting-publication={}",
            counts.closing,
            counts.cleanup_blocked,
            counts.outcome_unknown,
            counts.disposed_awaiting_publication
        ),
    )?;
    field(out, text.members, &members.len().to_string())?;
    // A member keeps the status it was captured with; once the whole scope
    // is closed, its last intermediate state is history, not a contradiction.
    let closed = matches!(status, WorkbenchTabCloseStatus::Closed { .. });
    table(out, &text.close_member_headers, members, |member| {
        close_member_cells(member, closed)
    })
}

fn close_member_cells(member: &WorkbenchWorkspaceCloseMember, closed: bool) -> Vec<String> {
    vec![
        member.tab_id.as_str().to_owned(),
        member.panel_id.as_str().to_owned(),
        member.presentation_id.as_str().to_owned(),
        if closed {
            "closed".to_owned()
        } else {
            member_status_text(&member.status)
        },
    ]
}

const fn close_status_token(status: &WorkbenchTabCloseStatus) -> &'static str {
    match status {
        WorkbenchTabCloseStatus::Closing { .. } => "closing",
        WorkbenchTabCloseStatus::CleanupBlocked { .. } => "cleanup-blocked",
        WorkbenchTabCloseStatus::OutcomeUnknown { .. } => "outcome-unknown",
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. } => {
            "disposed-awaiting-publication"
        }
        WorkbenchTabCloseStatus::Closed { .. } => "closed",
    }
}

/// The scope-level lifecycle status, spelled with the wire's own tokens —
/// the closed tombstone shows the publication revision it recorded.
fn close_status_text(status: &WorkbenchTabCloseStatus) -> String {
    match status {
        WorkbenchTabCloseStatus::Closing { phase } => {
            format!("closing ({})", closing_phase_text(*phase))
        }
        WorkbenchTabCloseStatus::CleanupBlocked { reason, resumable } => format!(
            "cleanup-blocked ({}, resumable={})",
            blocked_reason_text(*reason),
            resumable
        ),
        WorkbenchTabCloseStatus::OutcomeUnknown { phase, reason } => format!(
            "outcome-unknown ({}, {})",
            unknown_phase_text(*phase),
            unknown_reason_text(*reason)
        ),
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { reason, resumable } => format!(
            "disposed-awaiting-publication ({}, resumable={})",
            publication_reason_text(*reason),
            resumable
        ),
        WorkbenchTabCloseStatus::Closed { revision } => {
            format!("closed (revision {})", revision.value())
        }
    }
}

fn member_status_text(status: &WorkbenchWorkspaceCloseMemberStatus) -> String {
    match status {
        WorkbenchWorkspaceCloseMemberStatus::Closing { phase } => {
            format!("closing ({})", closing_phase_text(*phase))
        }
        WorkbenchWorkspaceCloseMemberStatus::CleanupBlocked { reason, resumable } => format!(
            "cleanup-blocked ({}, resumable={})",
            blocked_reason_text(*reason),
            resumable
        ),
        WorkbenchWorkspaceCloseMemberStatus::OutcomeUnknown { phase, reason } => format!(
            "outcome-unknown ({}, {})",
            unknown_phase_text(*phase),
            unknown_reason_text(*reason)
        ),
        WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication { reason, resumable } => {
            format!(
                "disposed-awaiting-publication ({}, resumable={})",
                publication_reason_text(*reason),
                resumable
            )
        }
    }
}

fn closing_phase_text(phase: WorkbenchTabClosePhase) -> &'static str {
    match phase {
        WorkbenchTabClosePhase::Queued => "queued",
        WorkbenchTabClosePhase::WaitingGrace => "waiting-grace",
        WorkbenchTabClosePhase::WaitingOutcome => "waiting-outcome",
        WorkbenchTabClosePhase::Disposing => "disposing",
    }
}

fn unknown_phase_text(phase: WorkbenchTabCloseUnknownPhase) -> &'static str {
    match phase {
        WorkbenchTabCloseUnknownPhase::Terminate => "terminate",
        WorkbenchTabCloseUnknownPhase::Force => "force",
        WorkbenchTabCloseUnknownPhase::Dispose => "dispose",
    }
}

fn blocked_reason_text(reason: WorkbenchTabCloseBlockedReason) -> &'static str {
    match reason {
        WorkbenchTabCloseBlockedReason::WorkerUnavailable => "worker-unavailable",
        WorkbenchTabCloseBlockedReason::HostRefused => "host-refused",
        WorkbenchTabCloseBlockedReason::CleanupIncomplete => "cleanup-incomplete",
        WorkbenchTabCloseBlockedReason::InventoryIncomplete => "inventory-incomplete",
        WorkbenchTabCloseBlockedReason::IdentityConflict => "identity-conflict",
        WorkbenchTabCloseBlockedReason::ContinuationBudgetExhausted => {
            "continuation-budget-exhausted"
        }
        WorkbenchTabCloseBlockedReason::ResumeGenerationExhausted => "resume-generation-exhausted",
    }
}

fn unknown_reason_text(reason: WorkbenchTabCloseUnknownReason) -> &'static str {
    match reason {
        WorkbenchTabCloseUnknownReason::TransportLost => "transport-lost",
        WorkbenchTabCloseUnknownReason::Deadline => "deadline",
        WorkbenchTabCloseUnknownReason::InvalidResponse => "invalid-response",
    }
}

fn publication_reason_text(reason: WorkbenchTabClosePublicationReason) -> &'static str {
    match reason {
        WorkbenchTabClosePublicationReason::TargetCleanupPending => "target-cleanup-pending",
        WorkbenchTabClosePublicationReason::Preparing => "preparing",
        WorkbenchTabClosePublicationReason::PreparationFailed => "preparation-failed",
        WorkbenchTabClosePublicationReason::AuthorityRetired => "authority-retired",
        WorkbenchTabClosePublicationReason::WorkspaceRetained => "workspace-retained",
    }
}

pub(crate) fn render(
    out: &mut Bounded,
    projection: &Projection,
    command: &QueryCommand,
    locale: Locale,
) -> Result<(), TooLarge> {
    let text = locale.cli_labels();
    match projection {
        Projection::WorkbenchQuery(doc) => doc.render(out, command, locale),
        Projection::Status(doc) => status(out, doc, text),
        Projection::ProjectList(rows) => table(out, &text.project_headers, rows, |row| {
            vec![
                row.handle.clone(),
                table_label(&row.label),
                row.directory_count.to_string(),
                row.workspace_count.to_string(),
            ]
        }),
        Projection::ProjectGet(doc) => {
            field(out, text.project, &quoted(&doc.row.label))?;
            field(out, text.id, &doc.row.project_id)?;
            out.write(format_args!("{}\n", text.directories))?;
            table(
                out,
                &text.directory_headers,
                &doc.directories,
                |directory| {
                    vec![
                        directory.directory_id.clone(),
                        directory.worker_id.clone(),
                        quoted(&directory.path),
                    ]
                },
            )?;
            out.write(format_args!("{}\n", text.workspaces))?;
            table(
                out,
                &text.workspace_ref_headers,
                &doc.workspaces,
                |workspace| {
                    vec![
                        workspace.workspace_id.clone(),
                        table_label(&workspace.label),
                    ]
                },
            )
        }
        Projection::WorkspaceList(rows) => workspace_list(out, rows, text, locale),
        Projection::WorkspaceGet(doc) => {
            use crate::localization::DetailLabel;
            let row = &doc.row;
            out.push(&locale.cli_workspace_summary(
                &quoted(&row.label),
                row.number,
                row.tab_count,
                row.panel_count,
            ))?;
            let project = match (&row.project_id, &row.project_label) {
                (Some(id), Some(label)) => format!("{} ({id})", quoted(label)),
                (Some(id), None) => id.clone(),
                (None, _) => locale.cli_ungrouped().to_owned(),
            };
            aligned(
                out,
                &[
                    (
                        locale.cli_detail_label(DetailLabel::Id),
                        row.workspace_id.clone(),
                    ),
                    (locale.cli_detail_label(DetailLabel::Project), project),
                    (
                        locale.cli_detail_label(DetailLabel::Directory),
                        doc.primary_directory_path
                            .as_deref()
                            .map_or_else(|| "-".to_owned(), unquoted),
                    ),
                ],
            )?;
            out.write(format_args!(
                "{}\n",
                locale.cli_detail_label(DetailLabel::Tabs)
            ))?;
            table(out, &text.tab_ref_headers, &doc.tabs, |tab| {
                vec![tab.tab_id.clone(), table_label(&tab.label)]
            })
        }
        Projection::TabList(rows) => tab_list(out, rows, text),
        Projection::TabGet(doc) => {
            field(out, text.tab, &quoted(&doc.row.label))?;
            field(out, text.number, &doc.row.number.to_string())?;
            field(out, text.panels_word, &doc.row.panel_count.to_string())?;
            field(out, text.id, &doc.row.tab_id)?;
            field(out, text.pinned, &doc.row.pinned.to_string())?;
            field(
                out,
                text.group,
                doc.row.group_id.as_deref().unwrap_or("null"),
            )?;
            out.write(format_args!(
                "{:<14}{} {}\n",
                text.context,
                context_text(&doc.row.context),
                doc.row.context.terminal_context_id
            ))?;
            out.write(format_args!("{}\n", text.root))?;
            // The subtree re-encode streams straight into the bounded sink;
            // `Value` cannot fail serialization, so an io error is the quota.
            serde_json::to_writer_pretty(&mut *out, &doc.root).map_err(|_| TooLarge)?;
            out.push("\n")
        }
        Projection::PanelList(rows) => table(out, &text.panel_headers, rows, |row| {
            vec![
                row.panel_id.clone(),
                table_label(&row.display_label),
                row.workspace_id.clone(),
                row.tab_id.clone(),
                row.terminal_context_id.clone(),
                row.primary_presentation_id.clone(),
                row.presentation_count.to_string(),
                row.primary_terminal_id
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                row.right_click.as_str().to_owned(),
            ]
        }),
        Projection::PanelGet(doc) => panel_doc(out, doc, text),
        Projection::PanelProcessInfo(doc) => panel_process(out, doc, locale),
        Projection::TerminalList(doc) | Projection::TerminalListHuman(doc) => {
            terminal_table(out, text, doc, locale)
        }
        // `terminal read` emits the observed content itself: one output line
        // per emitted grid row — or, under `--source recent-unwrapped`, per joined logical
        // line — preserving printable text and escaping controls, followed by
        // the localized markers for tail-omitted entries and
        // budget-omitted rows.
        Projection::TerminalOutput(doc) => {
            if matches!(
                command,
                QueryCommand::TerminalOutput { plain: true, .. }
                    | QueryCommand::TerminalOutput { ansi: true, .. }
            ) {
                return out.push(&doc.text);
            }
            for line in doc.text.lines() {
                out.push(&unquoted(line))?;
                out.push("\n")?;
            }
            if doc.range.capacity_omitted > 0 {
                out.push(&locale.cli_read_truncated(
                    usize::try_from(doc.range.capacity_omitted).unwrap_or(usize::MAX),
                ))?;
            }
            Ok(())
        }
        #[cfg(test)]
        Projection::TerminalRead(doc) => {
            match &doc.lines {
                Some(lines) => {
                    for line in lines {
                        out.push(&unquoted(&line.text))?;
                        out.push("\n")?;
                    }
                }
                None => {
                    for row in &doc.rows {
                        out.push(&unquoted(&row.text))?;
                        out.push("\n")?;
                    }
                }
            }
            if let Some(tail) = &doc.tail
                && tail.omitted > 0
            {
                out.push(&locale.cli_read_tail_omitted(tail.omitted, doc.lines.is_some()))?;
            }
            if doc.omitted_rows > 0 {
                out.push(&locale.cli_read_truncated(doc.omitted_rows))?;
            }
            Ok(())
        }
        // `terminal read --source recent` prints the retained rows the same way:
        // escaped text, one line per emitted row (or joined logical line
        // under `--source recent-unwrapped`), then the localized markers.
        #[cfg(test)]
        Projection::TerminalHistory(doc) => {
            match &doc.lines {
                Some(lines) => {
                    for line in lines {
                        out.push(&unquoted(&line.text))?;
                        out.push("\n")?;
                    }
                }
                None => {
                    for row in &doc.rows {
                        out.push(&unquoted(&row.text))?;
                        out.push("\n")?;
                    }
                }
            }
            if let Some(tail) = &doc.tail
                && tail.omitted > 0
            {
                out.push(&locale.cli_read_tail_omitted(tail.omitted, doc.lines.is_some()))?;
            }
            if doc.omitted_rows > 0 {
                out.push(
                    &locale.cli_read_truncated(
                        usize::try_from(doc.omitted_rows).unwrap_or(usize::MAX),
                    ),
                )?;
            }
            Ok(())
        }
        // `terminal wait-output` prints the matching logical line's excerpt:
        // the bounded slice that always contains the complete pattern,
        // escaped like every other user-controlled value.
        Projection::TerminalWaitOutput(doc) => {
            out.push(&unquoted(&doc.matched.excerpt))?;
            out.push("\n")?;
            Ok(())
        }
        Projection::TerminalGet(doc) => {
            use crate::localization::DetailLabel;
            let row = &doc.terminal;
            out.push(&locale.cli_terminal_summary(&row.panel_id))?;
            let launch = super::results::launch_fact_json(&doc.launch);
            let state = locale.cli_launch_state(launch["state"].as_str().unwrap_or_default());
            let launch = match (
                launch["geometry"]["columns"].as_u64(),
                launch["geometry"]["rows"].as_u64(),
            ) {
                (Some(columns), Some(rows)) => format!("{state} ({columns}x{rows})"),
                _ => state.to_owned(),
            };
            let mut rows = vec![
                (
                    locale.cli_detail_label(DetailLabel::Id),
                    row.terminal_id.clone(),
                ),
                (
                    locale.cli_detail_label(DetailLabel::Panel),
                    row.panel_id.clone(),
                ),
                (
                    locale.cli_detail_label(DetailLabel::Tab),
                    row.tab_id.clone(),
                ),
                (locale.cli_detail_label(DetailLabel::Launch), launch),
            ];
            if let Some(selection) = &doc.launch_selection {
                rows.push((text.launch_program, unquoted(selection.executable())));
                rows.push((
                    text.launch_arguments,
                    serde_json::json!(selection.arguments()).to_string(),
                ));
            }
            // A profile-launched terminal names the profile it came from.
            if let Some(provenance) = &doc.launch_profile {
                rows.push((text.launch_profile, provenance.profile_id().to_owned()));
            }
            aligned(out, &rows)
        }
        // `workspace close-status` prints the retained operation's identity
        // block, its aggregate counts, then one row per captured member —
        // the same record the machine document carries, tombstone included.
        Projection::WorkspaceCloseStatus(outcome) => {
            workspace_close_status(out, outcome, text, locale)
        }
        // `worktree list` prints the resolved identity block, then one row
        // per observed Git worktree: the escaped path (or its hex byte
        // form), the verbatim HEAD (an all-zero value stays exactly as Git
        // reported it), the verbatim branch ref, and the flag column.
        Projection::WorktreeList(doc) => {
            field(out, text.directory_id, &doc.directory_id)?;
            field(out, text.path, &super::escape::native_path(&doc.path))?;
            table(out, &text.worktree_headers, &doc.entries, worktree_cells)
        }
        // `launch-profile list` prints one row per entry, or how to create
        // the first one; the catalog revision stays in JSON. `get` renders the resolved entry as the same
        // name/value members the machine document carries.
        Projection::LaunchProfileCatalog(doc) => match &doc.selection {
            LaunchProfileCatalogSelection::List(entries) if entries.is_empty() => {
                out.push(locale.cli_launch_profile_empty())
            }
            LaunchProfileCatalogSelection::List(entries) => table(
                out,
                &text.launch_profile_headers,
                entries,
                launch_profile_cells,
            ),
            LaunchProfileCatalogSelection::Profile(entry) => {
                name_value_block(out, &super::results::launch_profile_entry_json(entry))
            }
        },
        // `api snapshot` answers a machine document in every format; the text
        // projection is unreachable by contract, not a second surface.
        Projection::ApiSnapshot(_) => {
            unreachable!("api snapshot success always takes the document arm")
        }
        // Agent text renders only ever see the success outcome — refusals
        // leave the query layer as failures before projection.
        #[cfg(not(windows))]
        Projection::AgentList(outcome) => agent_list(out, outcome, text, locale),
        Projection::UnifiedAgentList(doc) => super::agent_projection::render_list(out, doc, locale),
        Projection::UnifiedAgentInspection(doc) => {
            super::agent_projection::render_inspection(out, doc, command, locale)
        }
        Projection::AgentChanges(outcome) => agent_changes(out, outcome, locale),
        #[cfg(not(windows))]
        Projection::AgentGet(outcome) => agent_get(out, outcome, text, locale),
        Projection::AgentTerminalInspection {
            terminal,
            semantic,
            client_view,
            ..
        } => {
            super::terminal_observation::render(out, terminal, locale)?;
            super::panel_metadata_query::render_report(
                out,
                &serde_json::json!({"view":semantic.observation.view}),
                locale,
            )?;
            if let Some(client) = client_view {
                out.push(&locale.cli_terminal_agent_client_view(
                    client.client_id.as_str(),
                    client.status,
                    client.seen,
                ))?;
            }
            Ok(())
        }
        Projection::AgentInspection(doc) => {
            agent_get(
                out,
                &AgentGetOutcome::Found {
                    revision: doc.revision,
                    session: doc.session.clone(),
                },
                text,
                locale,
            )?;
            out.push(&locale.cli_agent_management(&doc.management.management))?;
            if let Some(client) = &doc.client {
                out.push(&locale.cli_agent_client_view(client))?;
            }
            if let Some(terminal) = &doc.terminal {
                super::terminal_observation::render(out, terminal, locale)?;
                super::panel_metadata_query::render_report(
                    out,
                    &terminal["reportedAgent"],
                    locale,
                )?;
            }
            match &doc.runtime {
                super::results::AgentRuntimeDoc::Unavailable(reason) => {
                    out.push(&locale.cli_agent_runtime(None, Some(reason)))
                }
                super::results::AgentRuntimeDoc::Observed {
                    processes,
                    prompt_ready,
                    ..
                } => {
                    out.push(&locale.cli_agent_runtime(Some(*prompt_ready), None))?;
                    panel_process(out, processes, locale)
                }
            }
        }
        Projection::AgentExplain(outcome) => agent_explain(out, outcome, text, locale),
        #[cfg(windows)]
        Projection::AgentTerminalExplanation(terminal) => {
            agent_screen::render(out, terminal, locale)
        }
        Projection::AgentWait(doc) => agent_wait(out, doc, locale),
        #[cfg(windows)]
        Projection::AgentTerminalWait(doc) => out.push(
            &locale.cli_agent_terminal_wait_matched(
                doc.read.observation.binding.presentation_id.as_str(),
                &doc.conditions
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                doc.change
                    .as_ref()
                    .map_or(doc.read.observation.sequence, |c| c.sequence)
                    .value(),
                doc.elapsed_ms,
            ),
        ),
    }
}

/// `agent wait`: the single matched line — session identity, the matched
/// conditions in requested order, the satisfied revision, and the honest
/// elapsed milliseconds. The session detail itself stays in the machine
/// document; the text surface is the wait's own verdict.
fn agent_wait(out: &mut Bounded, doc: &AgentWaitDoc, locale: Locale) -> Result<(), TooLarge> {
    #[cfg(windows)]
    let revision = doc
        .matched_change
        .as_ref()
        .map_or(doc.revision, |change| change.revision);
    #[cfg(not(windows))]
    let revision = doc.revision;
    let conditions = doc
        .matched
        .iter()
        .map(|condition| condition.as_str())
        .collect::<Vec<_>>()
        .join(",");
    out.push(&locale.cli_agent_wait_matched(
        doc.session.snapshot.session_id.as_str(),
        &conditions,
        revision.value(),
        doc.elapsed_ms,
    ))?;
    #[cfg(windows)]
    if let Some(client) = &doc.client {
        out.push(&locale.cli_agent_client_view(client))?;
    }
    Ok(())
}

/// `agent list`: the SESSION/PROVIDER/STATUS/FRESHNESS/LOCATION table, an
/// empty notice instead of an empty table, and a pagination hint whenever the
/// page is not the last one.
#[cfg(not(windows))]
fn agent_list(
    out: &mut Bounded,
    outcome: &AgentListOutcome,
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    let AgentListOutcome::Listed {
        items, next_cursor, ..
    } = outcome
    else {
        unreachable!("agent list projection holds only the listed outcome")
    };
    if items.is_empty() {
        out.push(locale.cli_agent_empty())?;
    } else {
        table(out, &text.agent_headers, items, |row| {
            vec![
                row.session_id.as_str().to_owned(),
                row.provider_id.as_str().to_owned(),
                locale.cli_agent_status(&row.status),
                locale
                    .cli_agent_freshness(row.observation.freshness)
                    .to_owned(),
                row.execution_location
                    .execution_location_id
                    .as_str()
                    .to_owned(),
            ]
        })?;
    }
    if let Some(cursor) = next_cursor {
        out.push(&locale.cli_agent_next_cursor(cursor.as_str()))?;
    }
    Ok(())
}

/// `agent get`: labeled fields for the session's own snapshot plus its
/// derived status — identity, provider, lifecycle, status, freshness,
/// location, observation coordinates, capabilities, pending requests, and
/// presentation attachment rows.
fn agent_get(
    out: &mut Bounded,
    outcome: &AgentGetOutcome,
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    let AgentGetOutcome::Found { session, .. } = outcome else {
        unreachable!("agent get projection holds only the found outcome")
    };
    agent_session_fields(out, session, text, locale)
}

fn agent_session_fields(
    out: &mut Bounded,
    session: &AgentSessionDetail,
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    let snapshot = &session.snapshot;
    field(out, text.agent_session, snapshot.session_id.as_str())?;
    field(out, text.agent_provider, snapshot.provider_id.as_str())?;
    field(
        out,
        text.agent_lifecycle,
        locale.cli_agent_lifecycle(snapshot.lifecycle),
    )?;
    field(
        out,
        text.agent_status,
        &locale.cli_agent_status(&session.status),
    )?;
    field(
        out,
        text.agent_freshness,
        locale.cli_agent_freshness(snapshot.observation.freshness),
    )?;
    field(
        out,
        text.agent_location,
        snapshot.execution_location.execution_location_id.as_str(),
    )?;
    out.write(format_args!(
        "{:<14}{} {} · {} {} · {} {}\n",
        text.agent_observation,
        text.agent_generation,
        snapshot.observation.generation.value(),
        text.agent_sequence,
        snapshot.observation.sequence.value(),
        locale.cli_agent_fidelity(snapshot.observation.fidelity),
        locale.cli_agent_source(snapshot.observation.source),
    ))?;
    if snapshot.capabilities.is_empty() {
        field(out, text.agent_capabilities, text.agent_none)?;
    } else {
        out.write(format_args!("{:<14}", text.agent_capabilities))?;
        for (index, capability) in snapshot.capabilities.iter().enumerate() {
            if index > 0 {
                out.push(", ")?;
            }
            out.push(locale.cli_agent_capability(*capability))?;
        }
        out.push("\n")?;
    }
    match &snapshot.last_outcome {
        Some(outcome) => out.write(format_args!(
            "{:<14}{} {} {}\n",
            text.agent_last_outcome,
            locale.cli_agent_outcome_kind(outcome.kind),
            outcome.turn_id.as_str(),
            outcome.event_id.as_str(),
        ))?,
        None => field(out, text.agent_last_outcome, text.agent_none)?,
    }
    out.write(format_args!(
        "{:<14}{}\n",
        text.agent_pending_requests,
        snapshot.pending_requests.len()
    ))?;
    table(
        out,
        &text.agent_request_headers,
        &snapshot.pending_requests,
        |request| {
            vec![
                locale.cli_agent_attention_kind(request.kind).to_owned(),
                request.request_id.as_str().to_owned(),
                request
                    .turn_id
                    .as_ref()
                    .map(|turn| turn.as_str().to_owned())
                    .unwrap_or_else(|| "null".to_owned()),
            ]
        },
    )?;
    out.write(format_args!("{}\n", text.agent_attachments))?;
    table(
        out,
        &text.agent_attachment_headers,
        &snapshot.presentation_attachments,
        |attachment| {
            vec![
                attachment.presentation_id.as_str().to_owned(),
                attachment.generation.as_str().to_owned(),
                locale.cli_agent_bool(attachment.navigable).to_owned(),
                locale
                    .cli_agent_visibility(attachment.visibility)
                    .to_owned(),
            ]
        },
    )
}

/// `agent explain`: the derived view — canonical status basis, freshness
/// meaning, capability ceiling, observation source and fidelity, and the
/// navigate-only action boundary.
fn agent_explain(
    out: &mut Bounded,
    outcome: &AgentExplainOutcome,
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    let AgentExplainOutcome::Found {
        session,
        explanation,
        ..
    } = outcome
    else {
        unreachable!("agent explain projection holds only the found outcome")
    };
    agent_explanation_fields(out, session, explanation, text, locale)
}

fn agent_explanation_fields(
    out: &mut Bounded,
    session: &AgentSessionDetail,
    explanation: &AgentSessionExplanation,
    text: &CliLabels,
    locale: Locale,
) -> Result<(), TooLarge> {
    field(
        out,
        text.agent_session,
        session.snapshot.session_id.as_str(),
    )?;
    field(
        out,
        text.agent_status,
        &locale.cli_agent_status(&explanation.status),
    )?;
    field(
        out,
        text.agent_status_basis,
        locale.cli_agent_status_basis(explanation.status_basis),
    )?;
    field(
        out,
        text.agent_freshness_meaning,
        locale.cli_agent_freshness_meaning(explanation.freshness_meaning),
    )?;
    field(
        out,
        text.agent_freshness,
        locale.cli_agent_freshness(explanation.freshness),
    )?;
    out.write(format_args!(
        "{:<14}{} {} · {} {}\n",
        text.agent_observation,
        text.agent_generation,
        explanation.generation.value(),
        text.agent_sequence,
        explanation.sequence.value(),
    ))?;
    if explanation.capability_ceiling.is_empty() {
        field(out, text.agent_capability_ceiling, text.agent_none)?;
    } else {
        out.write(format_args!("{:<14}", text.agent_capability_ceiling))?;
        for (index, capability) in explanation.capability_ceiling.iter().enumerate() {
            if index > 0 {
                out.push(", ")?;
            }
            out.push(locale.cli_agent_capability(*capability))?;
        }
        out.push("\n")?;
    }
    field(
        out,
        text.agent_source,
        locale.cli_agent_source(explanation.source),
    )?;
    field(
        out,
        text.agent_fidelity,
        locale.cli_agent_fidelity(explanation.fidelity),
    )?;
    out.write(format_args!("{:<14}", text.agent_navigate_only))?;
    out.push(locale.cli_agent_navigate_only_facts())
}

fn status_field(
    out: &mut Bounded,
    indent: usize,
    label: &str,
    value: &str,
    label_width: usize,
) -> Result<(), TooLarge> {
    let padding = label_width.saturating_sub(display_width(label)) + 2;
    out.write(format_args!(
        "{}{label}{}{value}\n",
        " ".repeat(indent),
        " ".repeat(padding)
    ))
}

fn status(out: &mut Bounded, doc: &StatusDoc, text: &CliLabels) -> Result<(), TooLarge> {
    let label_width = [
        text.status_session,
        text.status_session_name,
        text.status_client_version,
        text.projects_word,
        text.workspaces_word,
        text.tabs_word,
        text.panels_word,
        text.bound_terminals_word,
    ]
    .into_iter()
    .map(display_width)
    .max()
    .unwrap_or(0);

    status_field(
        out,
        0,
        text.status_session,
        text.status_running,
        label_width,
    )?;
    if let Some(session_name) = &doc.session_name {
        status_field(out, 0, text.status_session_name, session_name, label_width)?;
    }
    status_field(
        out,
        0,
        text.status_client_version,
        &doc.client_version,
        label_width,
    )?;
    out.write(format_args!("  {}\n", text.counts))?;
    status_field(
        out,
        2,
        text.projects_word,
        &doc.projects.to_string(),
        label_width,
    )?;
    status_field(
        out,
        2,
        text.workspaces_word,
        &doc.workspaces.to_string(),
        label_width,
    )?;
    status_field(out, 2, text.tabs_word, &doc.tabs.to_string(), label_width)?;
    status_field(
        out,
        2,
        text.panels_word,
        &doc.panels.to_string(),
        label_width,
    )?;
    status_field(
        out,
        2,
        text.bound_terminals_word,
        &doc.bound_terminals.to_string(),
        label_width,
    )
}

fn panel_doc(out: &mut Bounded, doc: &PanelDoc, text: &CliLabels) -> Result<(), TooLarge> {
    field(out, text.id, &doc.row.panel_id)?;
    field(out, text.right_click, doc.row.right_click.as_str())?;
    field(out, text.tab_id, &doc.row.tab_id)?;
    out.write(format_args!(
        "{:<14}{} {}\n",
        text.context,
        context_text(&doc.context),
        doc.context.terminal_context_id
    ))?;
    // The slot path joins without a whole-field temporary.
    out.write(format_args!("{:<14}", text.slot_path))?;
    if doc.slot_path.is_empty() {
        out.push("-\n")?;
    } else {
        for (index, segment) in doc.slot_path.iter().enumerate() {
            if index > 0 {
                out.push(" / ")?;
            }
            out.push(segment)?;
        }
        out.push("\n")?;
    }
    out.write(format_args!("{}\n", text.primary_presentation))?;
    table(
        out,
        &text.presentation_headers,
        std::slice::from_ref(&doc.primary_presentation),
        presentation_cells,
    )?;
    out.write(format_args!("{}\n", text.child_presentations))?;
    table(
        out,
        &text.presentation_headers,
        &doc.child_presentations,
        presentation_cells,
    )
}

/// Aligned columns with a heading row and no width degradation, for text
/// views assembled outside a bounded sink. The last column is never padded.
pub(crate) fn columns(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(display_width(cell));
        }
    }
    let mut text = String::new();
    let heading = headers
        .iter()
        .map(|header| (*header).to_owned())
        .collect::<Vec<_>>();
    for row in std::iter::once(&heading).chain(rows) {
        for (index, cell) in row.iter().enumerate() {
            if index > 0 {
                text.push_str("  ");
            }
            text.push_str(cell);
            if index + 1 < row.len() {
                text.push_str(&" ".repeat(widths[index].saturating_sub(display_width(cell))));
            }
        }
        text.push('\n');
    }
    text
}

/// `config check` text: the conclusion, one aligned row per check, any
/// settings warnings, the file in use, then where the full detail lives.
pub(crate) fn config_checks(
    out: &mut Bounded,
    checks: &[CheckReport],
    locale: Locale,
) -> Result<(), TooLarge> {
    let count = |status| checks.iter().filter(|check| check.status == status).count();
    out.push(&locale.cli_check_verdict(checks.len(), count("warning"), count("fail")))?;
    let rows = checks
        .iter()
        .map(|check| {
            vec![
                check.name.to_owned(),
                locale.cli_check_status(check.status).to_owned(),
                check_summary(check),
            ]
        })
        .collect::<Vec<_>>();
    table(out, &locale.cli_check_headers(), &rows, Vec::clone)?;
    for check in checks {
        for diagnostic in check
            .details
            .get("diagnostics")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            out.write(format_args!(
                "  {}: {} ({})\n",
                diagnostic["field"].as_str().unwrap_or("?"),
                diagnostic["reason"].as_str().unwrap_or("?"),
                diagnostic["disposition"].as_str().unwrap_or("?"),
            ))?;
        }
    }
    if let Some(path) = checks
        .iter()
        .find(|check| check.name == "user-config")
        .and_then(|check| check.details.get("path"))
        .and_then(Value::as_str)
    {
        out.push(&locale.cli_config_file_line(&super::escape::native_path(path)))?;
    }
    out.push(locale.cli_check_json_hint())
}

/// The one fact a check row shows: a failure's field and reason, else its
/// path, else its locale and source.
fn check_summary(check: &CheckReport) -> String {
    let text = |key| check.details.get(key).and_then(Value::as_str);
    if let Some(reason) = text("reason") {
        return text("field")
            .map_or_else(|| reason.to_owned(), |field| format!("{field}: {reason}"));
    }
    if let Some(path) = text("path") {
        return super::escape::native_path(path);
    }
    match (text("locale"), text("source")) {
        (Some(locale), Some(source)) => format!("{locale} ({source})"),
        _ => String::new(),
    }
}

/// `server diagnose` rows stream in the same `name  state  k=v` shape as
/// `config check`: one line per leaf row whose name is the machine
/// document's own dotted field path (`locks.activation`,
/// `endpoints.client`), and advisory tokens append their localized note
/// lines after the rows.
pub(crate) fn server_diagnose(
    out: &mut Bounded,
    document: &super::server_diagnose::DiagnoseDocument,
    locale: Locale,
) -> Result<(), TooLarge> {
    for (name, row) in &document.rows {
        let nested = row.values().all(|value| matches!(value, Value::Object(_)));
        if nested && !row.is_empty() {
            for (inner_key, inner) in row {
                if let Value::Object(inner_row) = inner {
                    diagnose_row(out, &format!("{name}.{inner_key}"), inner_row)?;
                }
            }
        } else {
            diagnose_row(out, name, row)?;
        }
    }
    for advisory in &document.advisories {
        out.push(locale.cli_server_diagnose_advisory(advisory))?;
    }
    Ok(())
}

/// One `server diagnose` leaf row: `name  key=value …` on its own line.
fn diagnose_row(
    out: &mut Bounded,
    name: &str,
    row: &serde_json::Map<String, Value>,
) -> Result<(), TooLarge> {
    out.write(format_args!("{name:<28}"))?;
    for (key, value) in row {
        out.write(format_args!("{key}="))?;
        detail(out, value)?;
        out.push(" ")?;
    }
    out.push("\n")
}

/// A detail value written recursively: strings are quoted and objects nest —
/// the pieces stream into the sink, no joined intermediate per level.
fn detail(out: &mut Bounded, value: &Value) -> Result<(), TooLarge> {
    match value {
        Value::String(text) => out.push(&quoted(text)),
        Value::Object(object) => {
            out.push("{")?;
            for (index, (key, value)) in object.iter().enumerate() {
                if index > 0 {
                    out.push(" ")?;
                }
                out.write(format_args!("{key}="))?;
                detail(out, value)?;
            }
            out.push("}")
        }
        other => out.push(&other.to_string()),
    }
}

/// A mutation receipt renders through the shared localized receipt: one
/// sentence, then the affected resources; internal identities stay in JSON.
pub(super) fn receipt(
    out: &mut Bounded,
    receipt: &super::mutation::Receipt,
    locale: Locale,
) -> Result<(), TooLarge> {
    out.push(&locale.cli_receipt_with(
        &receipt.document()["result"],
        receipt.closed_summary(locale),
    ))?;
    if let Some(hint) = receipt.target_hint(locale) {
        out.push(hint)?;
    }
    Ok(())
}

/// Confirm input submission using the admitted terminal identity, without
/// exposing transport receipt fields or claiming command completion.
pub(crate) fn send_result(
    out: &mut Bounded,
    result: &Value,
    locale: Locale,
) -> Result<(), TooLarge> {
    let terminal_id = result["terminalId"]
        .as_str()
        .expect("typed send terminal identity");
    out.push(&locale.cli_send_success(terminal_id))
}

/// One `name  value` block over a JSON object's own fields; nested values go
/// through the shared detail writer.
fn name_value_block(out: &mut Bounded, result: &Value) -> Result<(), TooLarge> {
    let Value::Object(map) = result else {
        return Ok(());
    };
    let width = map
        .keys()
        .map(|name| display_width(name))
        .max()
        .unwrap_or(0);
    for (name, value) in map {
        out.push(name)?;
        out.push(&" ".repeat(width.saturating_sub(display_width(name)) + 2))?;
        detail(out, value)?;
        out.push("\n")?;
    }
    Ok(())
}

/// `agent doctor` text rendering: one block per provider (executable, node,
/// hook rows), then the endpoint, environment, asset-root, and hint
/// sections. Field names render verbatim like `config check`; hint text
/// comes from the locale, and every fact streams through the shared detail
/// writer into the bounded sink.
#[cfg(any(target_os = "linux", windows))]
pub(crate) fn agent_doctor(
    out: &mut Bounded,
    result: &Value,
    locale: Locale,
) -> Result<(), TooLarge> {
    let text = locale.cli_labels();
    out.push(&doctor_summary(result, locale))?;
    out.push("\n")?;
    let providers = result["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|provider| {
            vec![
                provider["id"].as_str().unwrap_or("?").to_owned(),
                doctor_fact(&provider["executable"]),
                doctor_fact(&provider["runtime"]["node"]),
                doctor_fact(&provider["hook"]),
            ]
        })
        .collect::<Vec<_>>();
    table(out, &locale.cli_doctor_headers(), &providers, Vec::clone)?;
    out.push("\n")?;
    aligned(
        out,
        &[
            (
                text.agent_doctor_endpoint,
                doctor_fact(&result["endpoint"]["socket"]),
            ),
            (
                text.agent_doctor_environment,
                format!(
                    "ORCHBEE_AGENT_OBSERVATION_SOCKET {}",
                    doctor_fact(&result["environment"]["observationSocketEnv"])
                ),
            ),
        ],
    )?;
    if let Some(roots) = result["asset"]["roots"].as_array()
        && !roots.is_empty()
    {
        out.push(text.agent_doctor_asset_roots)?;
        out.push("\n")?;
        for root in roots {
            let path = root["path"]
                .as_str()
                .map_or_else(|| "-".to_owned(), super::escape::native_path);
            out.write(format_args!(
                "  {path}  {}  {}\n",
                root["origin"].as_str().unwrap_or("?"),
                root["state"].as_str().unwrap_or("?"),
            ))?;
        }
    }
    if let Some(hints) = result["hints"].as_array()
        && !hints.is_empty()
    {
        out.push(text.agent_doctor_hints)?;
        out.push("\n")?;
        for hint in hints {
            out.push("  ")?;
            let id = hint["id"].as_str().unwrap_or_default();
            out.push(&locale.cli_agent_doctor_hint_text(id, &doctor_hint_subjects(result, hint)))?;
            out.push("\n")?;
        }
    }
    out.push(locale.cli_doctor_json_hint())
}

/// The closing conclusion: a provider is ready with both its executable and
/// hook present; an installed provider whose hook is absent, disabled or
/// outdated needs `enable`; any other hook state needs `explain`.
#[cfg(any(target_os = "linux", windows))]
fn doctor_summary(result: &Value, locale: Locale) -> String {
    let mut counts = [0; 4];
    let mut enable = None;
    let mut explain = None;
    for provider in result["providers"].as_array().into_iter().flatten() {
        let id = provider["id"].as_str().unwrap_or_default();
        let slot = match (
            provider["executable"]["state"].as_str(),
            provider["hook"]["state"].as_str(),
        ) {
            (Some("present"), Some("present")) => 0,
            (Some("present"), Some("absent" | "disabled" | "update-required")) => {
                enable.get_or_insert(id);
                1
            }
            (Some("present"), _) => {
                explain.get_or_insert(id);
                2
            }
            _ => 3,
        };
        counts[slot] += 1;
    }
    let next = enable
        .map(|id| ("enable", id))
        .or_else(|| explain.map(|id| ("explain", id)));
    locale.cli_integration_summary(counts, next)
}

/// Re-derives a hint's subject list from its `evidence` rows for localized
/// text rendering — `providers.<id>.<field>` rows name their provider and
/// `asset.roots[<i>]` rows name their root path (or `origin` when the path
/// is not UTF-8). Mirrors the subjects the emitter cited.
#[cfg(any(target_os = "linux", windows))]
fn doctor_hint_subjects(result: &Value, hint: &Value) -> Vec<String> {
    let mut subjects = Vec::new();
    for row in hint["evidence"].as_array().into_iter().flatten() {
        let Some(row) = row.as_str() else { continue };
        if let Some(rest) = row.strip_prefix("providers.") {
            if let Some(id) = rest.split('.').next() {
                subjects.push(id.to_owned());
            }
        } else if let Some(index) = row
            .strip_prefix("asset.roots[")
            .and_then(|rest| rest.strip_suffix(']'))
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            let root = &result["asset"]["roots"][index];
            subjects.push(
                root["path"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| root["origin"].as_str().unwrap_or_default().to_owned()),
            );
        }
    }
    subjects
}

/// One doctor fact cell: `state`, with its `reason` when present. Detail
/// fields stay in the JSON document.
#[cfg(any(target_os = "linux", windows))]
fn doctor_fact(fact: &Value) -> String {
    let state = fact["state"].as_str().unwrap_or("unknown");
    fact["reason"]
        .as_str()
        .map_or_else(|| state.to_owned(), |reason| format!("{state} ({reason})"))
}

fn agent_changes(
    out: &mut Bounded,
    outcome: &orchbee_agent_observation_protocol::AgentChangesOutcome,
    locale: Locale,
) -> Result<(), TooLarge> {
    use orchbee_agent_observation_protocol::AgentChangesOutcome;
    let cursor = match outcome {
        AgentChangesOutcome::Changes {
            changes,
            cursor,
            observed_through_revision,
            runtime_revision,
            ..
        } => {
            table(out, &locale.cli_agent_change_headers(), changes, |row| {
                vec![
                    row.revision.value().to_string(),
                    row.session.session_id.as_str().to_owned(),
                    locale.cli_agent_status(&row.session.status),
                    locale
                        .cli_agent_freshness(row.session.observation.freshness)
                        .to_owned(),
                ]
            })?;
            out.push(&locale.cli_agent_changes_progress(
                observed_through_revision.value(),
                runtime_revision.value(),
            ))?;
            cursor
        }
        AgentChangesOutcome::Current { cursor, revision }
        | AgentChangesOutcome::ResetRequired { cursor, revision } => {
            out.push(&locale.cli_agent_changes_checkpoint(
                revision.value(),
                matches!(outcome, AgentChangesOutcome::ResetRequired { .. }),
            ))?;
            cursor
        }
        AgentChangesOutcome::Rejected { .. } | AgentChangesOutcome::Unavailable { .. } => {
            unreachable!("refusals do not reach successful change projection")
        }
    };
    out.push(&locale.cli_agent_next_cursor(cursor.as_str()))
}

fn panel_process(
    out: &mut Bounded,
    doc: &super::results::PanelProcessInfoDoc,
    locale: Locale,
) -> Result<(), TooLarge> {
    use super::processes::{filetime_utc, program_name, readable_command};
    use crate::localization::DetailLabel;
    let view = super::processes::interpret(&doc.info);
    let label = |label| locale.cli_detail_label(label);
    out.push(&locale.cli_process_summary(view.foreground.selection, view.foreground.provider))?;
    let mut rows = vec![(
        label(DetailLabel::Terminal),
        doc.terminal.terminal_id.clone(),
    )];
    if let Some(process) = view
        .foreground
        .pid
        .and_then(|pid| view.processes.iter().find(|row| row.facts.pid == pid))
    {
        let facts = process.facts;
        rows.extend([
            (label(DetailLabel::Process), unquoted(&program_name(facts))),
            (label(DetailLabel::Pid), facts.pid.to_string()),
            (
                label(DetailLabel::Started),
                filetime_utc(facts.creation_time),
            ),
            (
                label(DetailLabel::Directory),
                facts
                    .current_directory
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), unquoted),
            ),
            (
                label(DetailLabel::Command),
                readable_command(process)
                    .map_or_else(|| "-".to_owned(), |command| unquoted(&command)),
            ),
        ]);
        if let Some(provider) = process.provider {
            rows.push((label(DetailLabel::Agent), provider.to_owned()));
        }
    }
    aligned(out, &rows)?;
    out.write(format_args!("{}\n", label(DetailLabel::Processes)))?;
    table(out, &locale.cli_process_headers(), &view.processes, |row| {
        let process = row.facts;
        vec![
            process.pid.to_string(),
            process
                .parent_pid
                .map_or_else(|| "-".to_owned(), |pid| pid.to_string()),
            unquoted(&program_name(process)),
            row.provider.unwrap_or("-").to_owned(),
            filetime_utc(process.creation_time),
            unquoted(process.current_directory.as_deref().unwrap_or("-")),
        ]
    })
}

#[cfg(test)]
mod terminal_list_tests {
    use super::truncate_terminal_cell;

    #[test]
    fn truncation_never_splits_a_visible_control_escape() {
        assert_eq!(truncate_terminal_cell("1234567\u{001b}x", 15), "1234567…");
        assert_eq!(
            truncate_terminal_cell("1234567\u{001b}x", 16),
            "1234567\\u{001b}x"
        );
    }
}

#[cfg(all(test, any(target_os = "linux", windows)))]
mod doctor_summary_tests {
    use super::doctor_summary;
    use crate::localization::Locale;
    use serde_json::json;

    #[test]
    fn summary_counts_providers_and_names_the_first_enable_step() {
        let row = |id: &str, executable: &str, hook: &str| json!({"id": id, "executable": {"state": executable}, "hook": {"state": hook}});
        let result = json!({"providers": [
            row("claude-code", "present", "present"),
            row("codex", "present", "absent"),
            row("grok", "present", "conflict"),
            row("kilo", "absent", "absent"),
        ]});
        assert_eq!(
            doctor_summary(&result, Locale::EnUs),
            "Summary: 1 ready, 1 need setup, 1 need attention, 1 not installed.\nNext: orchbee agent integration enable --provider codex\n"
        );
        let result = json!({"providers": [row("grok", "present", "unknown")]});
        assert!(
            doctor_summary(&result, Locale::ZhCn)
                .ends_with("下一步：orchbee agent integration explain --provider grok\n")
        );
        let result = json!({"providers": [row("codex", "present", "present")]});
        assert!(!doctor_summary(&result, Locale::EnUs).contains("Next:"));
    }
}
