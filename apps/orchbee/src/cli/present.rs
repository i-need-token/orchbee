//! Text presentation for query results: fixed-width tables with escaped,
//! width-bounded labels, and field blocks for `get` commands. Everything is
//! written piece by piece into the bounded preparation sink, so an oversized
//! render aborts at the 8 MiB bound instead of assembling the full body.

use serde_json::Value;

use super::config::CheckReport;
use super::escape::{display_width, escape, quoted, table_label};
use super::output::{Bounded, TooLarge};
use super::results::{ContextRef, PanelDoc, PresentationRow, Projection, StatusDoc, TerminalRow};
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

fn field(out: &mut Bounded, name: &str, value: &str) -> Result<(), TooLarge> {
    out.write(format_args!("{name:<14}{value}\n"))
}

fn context_text(context: &ContextRef) -> String {
    if context.standalone {
        "standalone".to_owned()
    } else {
        format!(
            "workspace {}",
            context.workspace_id.as_deref().unwrap_or("-")
        )
    }
}

fn presentation_cells(row: &PresentationRow) -> Vec<String> {
    vec![
        row.presentation_id.clone(),
        table_label(&row.label),
        row.terminal_id.clone().unwrap_or_else(|| "-".to_owned()),
        row.worker_id.clone().unwrap_or_else(|| "-".to_owned()),
    ]
}

fn terminal_cells(row: &TerminalRow) -> Vec<String> {
    vec![
        row.terminal_id.clone(),
        row.worker_id.clone(),
        row.presentation_id.clone(),
        row.panel_id.clone(),
        row.tab_id.clone(),
        row.terminal_context_id.clone(),
    ]
}

fn terminal_table(
    out: &mut Bounded,
    text: &CliLabels,
    rows: &[TerminalRow],
) -> Result<(), TooLarge> {
    table(out, &text.terminal_headers, rows, terminal_cells)
}

pub(crate) fn render(
    out: &mut Bounded,
    projection: &Projection,
    _command: &QueryCommand,
    locale: Locale,
) -> Result<(), TooLarge> {
    let text = locale.cli_labels();
    match projection {
        Projection::Status(doc) => status(out, doc, text),
        Projection::ProjectList(rows) => table(out, &text.project_headers, rows, |row| {
            vec![
                row.project_id.clone(),
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
        Projection::WorkspaceList(rows) => table(out, &text.workspace_headers, rows, |row| {
            vec![
                row.workspace_id.clone(),
                table_label(&row.label),
                row.project_id.clone(),
                row.primary_directory_id
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                row.terminal_context_id.clone(),
            ]
        }),
        Projection::WorkspaceGet(doc) => {
            field(out, text.workspace, &quoted(&doc.row.label))?;
            field(out, text.id, &doc.row.workspace_id)?;
            field(out, text.project_id, &doc.row.project_id)?;
            field(
                out,
                text.primary_directory,
                doc.row.primary_directory_id.as_deref().unwrap_or("null"),
            )?;
            field(out, text.terminal_context, &doc.row.terminal_context_id)?;
            out.write(format_args!("{}\n", text.tabs))?;
            table(out, &text.tab_ref_headers, &doc.tabs, |tab| {
                vec![tab.tab_id.clone(), table_label(&tab.label)]
            })
        }
        Projection::TabList(rows) => table(out, &text.tab_headers, rows, |row| {
            vec![
                row.tab_id.clone(),
                table_label(&row.label),
                row.pinned.to_string(),
                row.group_id.clone().unwrap_or_else(|| "-".to_owned()),
                context_text(&row.context),
                row.context.terminal_context_id.clone(),
            ]
        }),
        Projection::TabGet(doc) => {
            field(out, text.tab, &quoted(&doc.row.label))?;
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
                row.tab_id.clone(),
                row.terminal_context_id.clone(),
                row.primary_presentation_id.clone(),
                row.presentation_count.to_string(),
                row.primary_terminal_id
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
            ]
        }),
        Projection::PanelGet(doc) => panel_doc(out, doc, text),
        Projection::TerminalList(rows) => terminal_table(out, text, rows),
        // `terminal read` emits the observed content itself: one output line
        // per emitted grid row, each escaped the same way as every other
        // user-controlled value, then a localized marker when the read
        // budget omitted rows.
        Projection::TerminalRead(doc) => {
            for row in &doc.rows {
                out.push(&escape(&row.text))?;
                out.push("\n")?;
            }
            if doc.omitted_rows > 0 {
                out.push(&locale.cli_read_truncated(doc.omitted_rows))?;
            }
            Ok(())
        }
        Projection::TerminalGet(row) => {
            field(out, text.terminal, &row.terminal_id)?;
            field(out, text.worker, &row.worker_id)?;
            out.write(format_args!("{}\n", text.bound_to))?;
            table(
                out,
                &text.terminal_headers,
                std::slice::from_ref(row),
                terminal_cells,
            )
        }
        // `api snapshot` answers a machine document in every format; the text
        // projection is unreachable by contract, not a second surface.
        Projection::ApiSnapshot(_) => {
            unreachable!("api snapshot success always takes the document arm")
        }
    }
}

fn status(out: &mut Bounded, doc: &StatusDoc, text: &CliLabels) -> Result<(), TooLarge> {
    out.write(format_args!(
        "{:<14}{} ({} {})\n",
        text.site, text.reachable, text.revision_label, doc.revision
    ))?;
    field(out, text.site_id, &doc.site_id)?;
    out.write(format_args!(
        "{:<14}{} {} · {} {} · {} {} · {} {} · {} {}\n",
        text.counts,
        text.projects_word,
        doc.projects,
        text.workspaces_word,
        doc.workspaces,
        text.tabs_word,
        doc.tabs,
        text.panels_word,
        doc.panels,
        text.bound_terminals_word,
        doc.bound_terminals,
    ))?;
    field(out, text.worker, text.worker_unknown)
}

fn panel_doc(out: &mut Bounded, doc: &PanelDoc, text: &CliLabels) -> Result<(), TooLarge> {
    field(out, text.id, &doc.row.panel_id)?;
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

/// `config check` text rendering: `name  status  key=value …` per check.
pub(crate) fn config_checks(
    out: &mut Bounded,
    checks: &[CheckReport],
    _locale: Locale,
) -> Result<(), TooLarge> {
    for check in checks {
        out.write(format_args!("{:<18}{}", check.name, check.status))?;
        for (key, value) in &check.details {
            out.write(format_args!("  {key}="))?;
            detail(out, value)?;
        }
        out.push("\n")?;
    }
    Ok(())
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

/// A mutation receipt renders as one `name  value` block using the JSON
/// field names verbatim so text and JSON name identical fields; the label
/// value is the only user-controlled text and is escaped through `quoted`.
pub(crate) fn receipt(
    out: &mut Bounded,
    receipt: &super::mutation::Receipt,
) -> Result<(), TooLarge> {
    let rows = receipt.text_fields();
    let width = rows
        .iter()
        .map(|(name, _)| display_width(name))
        .max()
        .unwrap_or(0);
    for (name, value) in &rows {
        out.push(name)?;
        out.push(&" ".repeat(width.saturating_sub(display_width(name)) + 2))?;
        if *name == "label" {
            out.push(&quoted(value))?;
        } else {
            out.push(value)?;
        }
        out.push("\n")?;
    }
    Ok(())
}
