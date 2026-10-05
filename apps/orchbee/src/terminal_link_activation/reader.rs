//! Link-specific interpretation over the shared exact terminal-content reader.

use std::{ops::Range, time::Instant};

use orchbee_terminal_protocol::TerminalTextWindowRow;
use orchbee_workbench_protocol::{PluginInvocationContext, Snapshot};

use crate::{
    local_client::LocalClient,
    terminal_buffer_text::TerminalTextPoint,
    terminal_content::ReadSession as ContentReadSession,
    terminal_link_activation::{Error, Guard, PaintedLinkSource, SnapshotMember, context},
    terminal_links::{self, LinkRead},
};

const MAXIMUM_ROWS: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Resolved {
    pub(crate) url: Option<String>,
    pub(crate) explicit: bool,
}

/// Link-owned semantic wrapper around one shared temporary read-only attachment.
pub(crate) struct ReadSession<'a, G: Guard> {
    content: ContentReadSession<'a, G>,
}

impl<'a, G: Guard> ReadSession<'a, G> {
    /// Reproduce the exact painted source through the shared independent reader.
    pub(crate) fn open(
        client: &'a mut LocalClient,
        snapshot: &Snapshot,
        source: &PaintedLinkSource,
        deadline: Instant,
        guard: &'a mut G,
    ) -> Result<Self, Error<G::Error>> {
        Ok(Self {
            content: ContentReadSession::open(client, snapshot, source, deadline, guard)
                .map_err(Error::from)?,
        })
    }

    /// Resolve OSC 8 metadata first, then a bounded soft-wrapped plain URL at the exact cell.
    pub(crate) fn resolve(
        &mut self,
        column: u16,
        row: u64,
        viewport: Range<u64>,
    ) -> Result<Resolved, Error<G::Error>> {
        let total = self.content.total_rows();
        let columns = self.content.columns();
        if row >= total || column >= columns {
            return Err(Error::LimitExceeded);
        }
        if let Some(url) = self.hyperlink(row, column)? {
            return Ok(Resolved {
                url: Some(url),
                explicit: true,
            });
        }
        let cursor = TerminalTextPoint {
            row: u32::try_from(row).map_err(|_| Error::LimitExceeded)?,
            col: column,
        };
        let mut start = row;
        let mut end = row.saturating_add(2).min(total);
        let mut rows = self.content.rows(start..end).map_err(Error::from)?;
        loop {
            match terminal_links::resolve(&rows, columns, cursor, total, viewport.clone()) {
                LinkRead::Complete(link) => {
                    return Ok(Resolved {
                        url: link.url,
                        explicit: false,
                    });
                }
                LinkRead::LimitExceeded => return Err(Error::LimitExceeded),
                LinkRead::NeedContext { before, after } => {
                    let remaining = MAXIMUM_ROWS.saturating_sub(rows.len());
                    if remaining == 0 {
                        return Err(Error::LimitExceeded);
                    }
                    let extension = rows.len().max(64).min(remaining) as u64;
                    if before {
                        let next = start.saturating_sub(extension);
                        let mut incoming = self.content.rows(next..start).map_err(Error::from)?;
                        validate_join(&incoming, &rows)?;
                        incoming.extend(rows);
                        rows = incoming;
                        start = next;
                    }
                    if after {
                        let remaining = MAXIMUM_ROWS.saturating_sub(rows.len()) as u64;
                        if remaining == 0 {
                            return Err(Error::LimitExceeded);
                        }
                        let next = end.saturating_add(extension.min(remaining)).min(total);
                        let incoming = self.content.rows(end..next).map_err(Error::from)?;
                        validate_join(&rows, &incoming)?;
                        rows.extend(incoming);
                        end = next;
                    }
                }
            }
        }
    }

    /// Observe invocation context while the shared reader retains attachment ownership.
    pub(crate) fn observe_context(
        &mut self,
        snapshot: &Snapshot,
        member: &SnapshotMember,
        provided: PluginInvocationContext,
    ) -> Result<PluginInvocationContext, Error<G::Error>> {
        let pane_directory = self.content.pane_directory().map_err(Error::from)?;
        self.content
            .with_attached_observer(|client, deadline, guard| {
                context::observe(
                    client,
                    snapshot,
                    member,
                    pane_directory,
                    provided,
                    deadline,
                    guard,
                )
            })
    }

    /// Submit the shared reader's sole pre-allocated Detach attempt.
    pub(crate) fn detach(&mut self) -> bool {
        self.content.detach()
    }

    fn hyperlink(&mut self, row: u64, column: u16) -> Result<Option<String>, Error<G::Error>> {
        self.content
            .inspect_render_row(row, |window| {
                let projection = window.projection();
                let row = projection.rows().next().ok_or(())?;
                terminal_links::explicit_url(row, column)
            })
            .map_err(Error::from)
    }
}

fn validate_join<E>(
    before: &[TerminalTextWindowRow],
    after: &[TerminalTextWindowRow],
) -> Result<(), Error<E>> {
    if let (Some(before), Some(after)) = (before.last(), after.first())
        && before.wrap != after.wrap_continuation
    {
        return Err(Error::ProtocolInvalid);
    }
    Ok(())
}
