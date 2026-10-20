//! Closed public copy requests preserve literals and require their content fences.
use super::{
    CopyMotion, CopySearchDirection, ExpectedValue, FailureEnvelope, NO_OPTIONS, Options,
    OutputFormat, ParseFailure, Request, Resource, Scope, Target, TerminalTextPoint,
    TerminalTextRange, UsageContext, check_scope, missing_required, target, terminal_buffer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalCopyCommand {
    pub(crate) target: Target,
    pub(crate) generation: Option<u64>,
    pub(crate) action: TerminalCopyAction,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalCopyAction {
    Motion {
        cursor: TerminalTextPoint,
        motion: CopyMotion,
        revision: Option<u64>,
    },
    Search {
        cursor: TerminalTextPoint,
        query: String,
        direction: CopySearchDirection,
        previous: Option<TerminalTextRange>,
        revision: u64,
    },
}
impl TerminalCopyCommand {
    pub(crate) fn revision(&self) -> Option<u64> {
        match self.action {
            TerminalCopyAction::Motion { revision, .. } => revision,
            TerminalCopyAction::Search { revision, .. } => Some(revision),
        }
    }
    pub(crate) fn cursor(&self) -> TerminalTextPoint {
        match self.action {
            TerminalCopyAction::Motion { cursor, .. }
            | TerminalCopyAction::Search { cursor, .. } => cursor,
        }
    }
}
pub(super) fn parse(
    value: &str,
    options: &Options,
    search: bool,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(if search {
        "terminal copy-search"
    } else {
        "terminal copy-motion"
    });
    check_scope(
        options,
        Scope {
            copy_query: search,
            copy_previous: search,
            search_direction: search,
            copy_motion: !search,
            cursor: true,
            content_revision: true,
            generation: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let cursor = terminal_buffer::point(options.cursor.as_deref(), "--cursor", context, envelope)?;
    let revision = options
        .content_revision
        .as_deref()
        .map(|value| terminal_buffer::number(value, "--content-revision", false, context, envelope))
        .transpose()?;
    let generation = options
        .generation
        .as_deref()
        .map(|value| terminal_buffer::number(value, "--generation", true, context, envelope))
        .transpose()?;
    let action = if search {
        let revision =
            revision.ok_or_else(|| missing_required(context, "--content-revision", envelope))?;
        let query = options
            .copy_query
            .as_ref()
            .ok_or_else(|| missing_required(context, "--query", envelope))?;
        if query.len() > 4096 {
            return Err(terminal_buffer::invalid(
                query,
                "--query",
                ExpectedValue::TerminalCopyQuery,
                context,
                envelope,
            ));
        }
        let raw = options
            .search_direction
            .as_deref()
            .ok_or_else(|| missing_required(context, "--search-direction", envelope))?;
        let direction = match raw {
            "forward" => CopySearchDirection::Forward,
            "backward" => CopySearchDirection::Backward,
            _ => {
                return Err(terminal_buffer::invalid(
                    raw,
                    "--search-direction",
                    ExpectedValue::TerminalCopyDirection,
                    context,
                    envelope,
                ));
            }
        };
        let previous = options
            .copy_previous
            .as_deref()
            .map(|raw| {
                let invalid = || {
                    terminal_buffer::invalid(
                        raw,
                        "--previous",
                        ExpectedValue::TerminalCopyPrevious,
                        context,
                        envelope,
                    )
                };
                let (a, b) = raw.split_once('-').ok_or_else(invalid)?;
                let a = terminal_buffer::point(Some(a), "--previous", context, envelope)
                    .map_err(|_| invalid())?;
                let b = terminal_buffer::point(Some(b), "--previous", context, envelope)
                    .map_err(|_| invalid())?;
                Ok(TerminalTextRange {
                    start: a.min(b),
                    end: a.max(b),
                })
            })
            .transpose()?;
        TerminalCopyAction::Search {
            cursor,
            query: query.clone(),
            direction,
            previous,
            revision,
        }
    } else {
        let raw = options
            .copy_motion
            .as_deref()
            .ok_or_else(|| missing_required(context, "--motion", envelope))?;
        let motion = CopyMotion::ALL
            .into_iter()
            .find(|motion| motion.token() == raw)
            .ok_or_else(|| {
                terminal_buffer::invalid(
                    raw,
                    "--motion",
                    ExpectedValue::TerminalCopyMotion,
                    context,
                    envelope,
                )
            })?;
        TerminalCopyAction::Motion {
            cursor,
            motion,
            revision,
        }
    };
    Ok(Request::TerminalCopy {
        command: TerminalCopyCommand {
            target: target(value.to_owned(), Resource::Terminal, envelope)?,
            generation,
            action,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests;
