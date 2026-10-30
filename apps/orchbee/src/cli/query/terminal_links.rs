//! Link observation owns an explicit immutable viewport and existing viewer reads.
use super::terminal_buffer::{self, ContentFence, Prepared};
use crate::cli_text::CliText;
use crate::{
    cli::Output,
    invocation::{OutputFormat, TerminalLinkCommand, TerminalTextPoint},
    localization::Locale,
    terminal_links::ResolvedLink,
};
use serde_json::json;

pub(crate) fn run(command: &TerminalLinkCommand, format: OutputFormat, locale: Locale) -> Output {
    terminal_buffer::run(
        &command.target,
        ContentFence {
            revision: command.revision,
            generation: command.generation,
        },
        format,
        locale,
        if command.activate {
            "orchbee.terminal.link-activate"
        } else {
            "orchbee.terminal.link-resolve"
        },
        |reader| {
            let height = u64::from(reader.frame.geometry.rows);
            if u64::from(command.cursor.row) >= height
                || command.cursor.col >= reader.frame.geometry.columns
                || command.offset > reader.frame.scrollback_rows
            {
                return Err(terminal_buffer::limit());
            }
            let start = reader.frame.scrollback_rows - command.offset;
            let viewport = start..start + height;
            let row = start + u64::from(command.cursor.row);
            let cursor = TerminalTextPoint {
                row: u32::try_from(row).map_err(|_| terminal_buffer::limit())?,
                col: command.cursor.col,
            };
            let explicit = reader.hyperlink(row, command.cursor.col)?;
            let (kind, resolved) = if let Some(url) = explicit {
                (
                    "explicit",
                    ResolvedLink {
                        url: Some(url),
                        regions: Vec::new(),
                    },
                )
            } else {
                let resolved = terminal_buffer::plain_link(reader, cursor, viewport.clone())?;
                (
                    if resolved.url.is_some() {
                        "plain-text"
                    } else {
                        "none"
                    },
                    resolved,
                )
            };
            let mut text = CliText::new(locale).cli_terminal_link(
                &reader.terminal.terminal_id,
                reader.frame.route.revision,
                command.cursor,
                command.offset,
                kind,
                &resolved,
            );
            let mut result = json!({
                "origin":{"row":command.cursor.row,"col":command.cursor.col},
                "offsetFromBottom":command.offset.to_string(),
                "viewportStartRow":start.to_string(),
                "kind":kind,"url":&resolved.url,
                "regions":resolved.regions.iter().map(|r|json!({"row":r.row,"startCol":r.start_col,"endCol":r.end_col})).collect::<Vec<_>>(),
            });
            let mut status = 0;
            let mut submitted = false;
            if command.activate {
                let activation = reader.activate_link(
                    resolved.url.as_deref(),
                    row,
                    command.cursor.col,
                    viewport,
                    kind == "explicit",
                )?;
                status = activation.status();
                submitted = activation.submitted();
                text.push_str(
                    &CliText::new(locale)
                        .cli_terminal_link_activation(activation.state(), activation.operation()),
                );
                result["handled"] = json!(activation.handled());
                if let Some(reason) = activation.failure() {
                    text.push_str(CliText::new(locale).plugin_runtime_failure(reason));
                    text.push('\n');
                }
                result["activation"] = json!(activation);
            }
            Ok(Prepared {
                result,
                text,
                status,
                submitted,
            })
        },
    )
}

#[cfg(test)]
mod tests;
