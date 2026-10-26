//! Link observation owns an explicit immutable viewport and existing viewer reads.
use super::terminal_buffer::{self, BufferReader, ContentFence, Prepared};
use super::viewer_protocol_invalid;
use crate::cli_text::CliText;
use crate::{
    cli::Output,
    invocation::{OutputFormat, TerminalLinkCommand, TerminalTextPoint},
    localization::Locale,
    terminal_links::{self, LinkRead, ResolvedLink},
};
use serde_json::json;
use std::ops::Range;

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
                let resolved = plain(reader, cursor, viewport.clone())?;
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

pub(super) fn plain(
    reader: &mut BufferReader<'_>,
    cursor: TerminalTextPoint,
    viewport: Range<u64>,
) -> Result<ResolvedLink, crate::cli::Failure> {
    let mut start = u64::from(cursor.row);
    let mut end = (start + 2).min(reader.total);
    let mut rows = reader.rows(start..end)?;
    loop {
        match terminal_links::resolve(
            &rows,
            reader.frame.geometry.columns,
            cursor,
            reader.total,
            viewport.clone(),
        ) {
            LinkRead::Complete(result) => return Ok(result),
            LinkRead::LimitExceeded => return Err(terminal_buffer::limit()),
            LinkRead::NeedContext { before, after } => {
                let remaining = 65536_usize.saturating_sub(rows.len());
                if remaining == 0 {
                    return Err(terminal_buffer::limit());
                }
                let extension = rows.len().max(64).min(remaining) as u64;
                if before {
                    let next = start.saturating_sub(extension);
                    let mut incoming = reader.rows(next..start)?;
                    if let (Some(prior), Some(after)) = (incoming.last(), rows.first())
                        && prior.wrap != after.wrap_continuation
                    {
                        return Err(viewer_protocol_invalid());
                    }
                    incoming.extend(rows);
                    rows = incoming;
                    start = next;
                }
                if after {
                    let remaining = 65536_usize.saturating_sub(rows.len()) as u64;
                    if remaining == 0 {
                        return Err(terminal_buffer::limit());
                    }
                    let next = end
                        .saturating_add(extension.min(remaining))
                        .min(reader.total);
                    let incoming = reader.rows(end..next)?;
                    if let (Some(prior), Some(after)) = (rows.last(), incoming.first())
                        && prior.wrap != after.wrap_continuation
                    {
                        return Err(viewer_protocol_invalid());
                    }
                    rows.extend(incoming);
                    end = next;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
