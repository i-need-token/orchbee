//! Viewport coordinates and immutable history offsets are admitted before effects.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, Options, OutputFormat, ParseFailure, Request,
    Resource, Scope, Target, TerminalTextPoint, UsageContext, check_scope, target, terminal_buffer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalLinkCommand {
    pub(crate) activate: bool,
    pub(crate) target: Target,
    pub(crate) cursor: TerminalTextPoint,
    pub(crate) offset: u64,
    pub(crate) revision: Option<u64>,
    pub(crate) generation: Option<u64>,
}
pub(super) fn parse(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
    activate: bool,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path(if activate {
        "terminal link-activate"
    } else {
        "terminal link-resolve"
    });
    check_scope(
        options,
        Scope {
            cursor: true,
            link_offset: true,
            content_revision: true,
            generation: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
    let viewport_error = || {
        terminal_buffer::invalid(
            options.cursor.as_deref().unwrap_or_default(),
            "--cursor",
            ExpectedValue::TerminalViewportPoint,
            context,
            envelope,
        )
    };
    let cursor = terminal_buffer::point(options.cursor.as_deref(), "--cursor", context, envelope)
        .map_err(|failure| {
        if options.cursor.is_some() {
            viewport_error()
        } else {
            failure
        }
    })?;
    if cursor.row > u32::from(u16::MAX) {
        return Err(viewport_error());
    }
    let offset = options
        .link_offset
        .as_deref()
        .map(|raw| terminal_buffer::number(raw, "--offset-from-bottom", false, context, envelope))
        .transpose()?
        .unwrap_or(0);
    let revision = options
        .content_revision
        .as_deref()
        .map(|raw| terminal_buffer::number(raw, "--content-revision", false, context, envelope))
        .transpose()?;
    let generation = options
        .generation
        .as_deref()
        .map(|raw| terminal_buffer::number(raw, "--generation", true, context, envelope))
        .transpose()?;
    Ok(Request::TerminalLink {
        command: TerminalLinkCommand {
            activate,
            target: target(value.to_owned(), Resource::Terminal, envelope)?,
            cursor,
            offset,
            revision,
            generation,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

#[cfg(test)]
mod tests;
