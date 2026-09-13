//! Text presentation for query results: fixed-width tables with escaped,
//! width-bounded labels, and field blocks for `get` commands.

use serde_json::Value;

use super::config::CheckReport;
use super::escape::{display_width, quoted, table_label};
use super::results::{ContextRef, PanelDoc, PresentationRow, Projection, StatusDoc, TerminalRow};
use crate::invocation::QueryCommand;
use crate::localization::{CliLabels, Locale};

/// Render one table: header plus aligned rows. A table with no rows is just
/// its header — an empty result set is a successful answer.
fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell));
        }
    }
    let mut output = String::new();
    let render_row = |cells: &[String], output: &mut String| {
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                output.push_str("  ");
            }
            output.push_str(cell);
            if index + 1 < cells.len() {
                let padding = widths[index].saturating_sub(display_width(cell));
                output.push_str(&" ".repeat(padding));
            }
        }
        output.push('\n');
    };
    render_row(
        &headers
            .iter()
            .map(|header| (*header).to_owned())
            .collect::<Vec<_>>(),
        &mut output,
    );
    for row in rows {
        render_row(row, &mut output);
    }
    output
}

fn field(name: &str, value: &str) -> String {
    format!("{name:<14}{value}\n")
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

fn terminal_table(text: &CliLabels, rows: &[TerminalRow]) -> String {
    table(
        &text.terminal_headers,
        &rows.iter().map(terminal_cells).collect::<Vec<_>>(),
    )
}

pub(crate) fn render(projection: &Projection, _command: &QueryCommand, locale: Locale) -> String {
    let text = locale.cli_labels();
    match projection {
        Projection::Status(doc) => status(doc, text),
        Projection::ProjectList(rows) => table(
            &text.project_headers,
            &rows
                .iter()
                .map(|row| {
                    vec![
                        row.project_id.clone(),
                        table_label(&row.label),
                        row.directory_count.to_string(),
                        row.workspace_count.to_string(),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Projection::ProjectGet(doc) => {
            let mut output = String::new();
            output.push_str(&field(text.project, &quoted(&doc.row.label)));
            output.push_str(&field(text.id, &doc.row.project_id));
            output.push_str(&format!("{}\n", text.directories));
            output.push_str(&table(
                &text.directory_headers,
                &doc.directories
                    .iter()
                    .map(|directory| {
                        vec![
                            directory.directory_id.clone(),
                            directory.worker_id.clone(),
                            quoted(&directory.path),
                        ]
                    })
                    .collect::<Vec<_>>(),
            ));
            output.push_str(&format!("{}\n", text.workspaces));
            output.push_str(&table(
                &text.workspace_ref_headers,
                &doc.workspaces
                    .iter()
                    .map(|workspace| {
                        vec![
                            workspace.workspace_id.clone(),
                            table_label(&workspace.label),
                        ]
                    })
                    .collect::<Vec<_>>(),
            ));
            output
        }
        Projection::WorkspaceList(rows) => table(
            &text.workspace_headers,
            &rows
                .iter()
                .map(|row| {
                    vec![
                        row.workspace_id.clone(),
                        table_label(&row.label),
                        row.project_id.clone(),
                        row.primary_directory_id
                            .clone()
                            .unwrap_or_else(|| "-".to_owned()),
                        row.terminal_context_id.clone(),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Projection::WorkspaceGet(doc) => {
            let mut output = String::new();
            output.push_str(&field(text.workspace, &quoted(&doc.row.label)));
            output.push_str(&field(text.id, &doc.row.workspace_id));
            output.push_str(&field(text.project_id, &doc.row.project_id));
            output.push_str(&field(
                text.primary_directory,
                doc.row.primary_directory_id.as_deref().unwrap_or("null"),
            ));
            output.push_str(&field(text.terminal_context, &doc.row.terminal_context_id));
            output.push_str(&format!("{}\n", text.tabs));
            output.push_str(&table(
                &text.tab_ref_headers,
                &doc.tabs
                    .iter()
                    .map(|tab| vec![tab.tab_id.clone(), table_label(&tab.label)])
                    .collect::<Vec<_>>(),
            ));
            output
        }
        Projection::TabList(rows) => table(
            &text.tab_headers,
            &rows
                .iter()
                .map(|row| {
                    vec![
                        row.tab_id.clone(),
                        table_label(&row.label),
                        row.pinned.to_string(),
                        row.group_id.clone().unwrap_or_else(|| "-".to_owned()),
                        context_text(&row.context),
                        row.context.terminal_context_id.clone(),
                    ]
                })
                .collect::<Vec<_>>(),
        ),
        Projection::TabGet(doc) => {
            let mut output = String::new();
            output.push_str(&field(text.tab, &quoted(&doc.row.label)));
            output.push_str(&field(text.id, &doc.row.tab_id));
            output.push_str(&field(text.pinned, &doc.row.pinned.to_string()));
            output.push_str(&field(
                text.group,
                doc.row.group_id.as_deref().unwrap_or("null"),
            ));
            output.push_str(&field(
                text.context,
                &format!(
                    "{} {}",
                    context_text(&doc.row.context),
                    doc.row.context.terminal_context_id
                ),
            ));
            output.push_str(&format!("{}\n", text.root));
            output.push_str(
                &serde_json::to_string_pretty(&doc.root).unwrap_or_else(|_| "null".to_owned()),
            );
            output.push('\n');
            output
        }
        Projection::PanelList(rows) => table(
            &text.panel_headers,
            &rows
                .iter()
                .map(|row| {
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
                })
                .collect::<Vec<_>>(),
        ),
        Projection::PanelGet(doc) => panel_doc(doc, text),
        Projection::TerminalList(rows) => terminal_table(text, rows),
        Projection::TerminalGet(row) => {
            let mut output = String::new();
            output.push_str(&field(text.terminal, &row.terminal_id));
            output.push_str(&field(text.worker, &row.worker_id));
            output.push_str(&format!("{}\n", text.bound_to));
            output.push_str(&table(&text.terminal_headers, &[terminal_cells(row)]));
            output
        }
        Projection::ApiSnapshot(document) => {
            // The machine surface is identical in both formats.
            let mut output =
                serde_json::to_string_pretty(document).unwrap_or_else(|_| "null".to_owned());
            output.push('\n');
            output
        }
    }
}

fn status(doc: &StatusDoc, text: &CliLabels) -> String {
    let mut output = String::new();
    output.push_str(&field(
        text.site,
        &format!(
            "{} ({} {})",
            text.reachable, text.revision_label, doc.revision
        ),
    ));
    output.push_str(&field(text.site_id, &doc.site_id));
    output.push_str(&field(
        text.counts,
        &format!(
            "{} {} · {} {} · {} {} · {} {} · {} {}",
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
        ),
    ));
    output.push_str(&field(text.worker, text.worker_unknown));
    output
}

fn panel_doc(doc: &PanelDoc, text: &CliLabels) -> String {
    let mut output = String::new();
    output.push_str(&field(text.id, &doc.row.panel_id));
    output.push_str(&field(text.tab_id, &doc.row.tab_id));
    output.push_str(&field(
        text.context,
        &format!(
            "{} {}",
            context_text(&doc.context),
            doc.context.terminal_context_id
        ),
    ));
    output.push_str(&field(
        text.slot_path,
        &if doc.slot_path.is_empty() {
            "-".to_owned()
        } else {
            doc.slot_path.join(" / ")
        },
    ));
    output.push_str(&format!("{}\n", text.primary_presentation));
    output.push_str(&table(
        &text.presentation_headers,
        &[presentation_cells(&doc.primary_presentation)],
    ));
    output.push_str(&format!("{}\n", text.child_presentations));
    output.push_str(&table(
        &text.presentation_headers,
        &doc.child_presentations
            .iter()
            .map(presentation_cells)
            .collect::<Vec<_>>(),
    ));
    output
}

/// `config check` text rendering: `name  status  key=value …` per check.
pub(crate) fn config_checks(checks: &[CheckReport], _locale: Locale) -> String {
    let mut output = String::new();
    for check in checks {
        output.push_str(&format!("{:<18}{}", check.name, check.status));
        for (key, value) in &check.details {
            output.push_str(&format!("  {key}={}", detail_text(value)));
        }
        output.push('\n');
    }
    output
}

fn detail_text(value: &Value) -> String {
    match value {
        Value::String(text) => quoted(text),
        Value::Object(object) => {
            let inner = object
                .iter()
                .map(|(key, value)| format!("{key}={}", detail_text(value)))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{{{inner}}}")
        }
        other => other.to_string(),
    }
}
