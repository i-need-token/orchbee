//! The title each Presentation shows, by the naming rule this Client shares with the GUI: of what
//! this Client's live frames reported and what the Server published for the Presentation's
//! terminal, the title that arrived last. It names no program, localizes nothing and reads no
//! terminal; chrome applies program naming to the result.

use crate::workbench_projection::ClientTerminalBinding;
use orchbee_workbench_protocol::{
    PresentationTitle, PresentationTitleRow, SessionId, TerminalPresentationId,
    project_terminal_title,
};
use std::collections::HashMap;

/// What a Presentation shows by the shared rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShownTitle<'a> {
    /// The program's own name: it cleared its title, or its terminal has shown none here.
    Program,
    /// A live frame's raw title, which chrome strips as it always has.
    Live(&'a str),
    /// A title the shared projection already stripped, from a frame or from the Server. Chrome
    /// never strips it again.
    Projected(&'a str),
}

/// One cursor per Presentation in the Session's latest snapshot, created by its first frame title
/// or row. Every text it holds went through the shared projection, so each is at most
/// [`orchbee_workbench_protocol::MAXIMUM_PRESENTATION_TITLE_BYTES`].
#[derive(Clone, Debug, Default)]
pub(super) struct PresentationTitles {
    session: Option<SessionId>,
    cursors: HashMap<TerminalPresentationId, Cursor>,
}

#[derive(Clone, Debug)]
struct Cursor {
    /// The snapshot's binding, or `None` where the Presentation holds no terminal.
    terminal: Option<ClientTerminalBinding>,
    /// The latest row, kept until the snapshot names the terminal it was read for.
    row: Option<Row>,
    /// `None` is the program's own name.
    shown: Option<Shown>,
    /// What rows last observed for `terminal`; `None` while they observed nothing for it.
    stream: Option<Option<String>>,
    /// What live frames last reported for `terminal`, projected; `None` before one reported.
    surface: Option<Option<String>>,
}

#[derive(Clone, Debug)]
struct Row {
    binding: Option<ClientTerminalBinding>,
    title: PresentationTitle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Shown {
    published: bool,
    text: String,
}

impl Cursor {
    fn new(terminal: Option<&ClientTerminalBinding>) -> Self {
        Self {
            terminal: terminal.cloned(),
            row: None,
            shown: None,
            stream: None,
            surface: None,
        }
    }

    /// Another terminal shows its program until a title arrives for it. Unlike the GUI, which
    /// keeps the old frame title while no row ever arrived, this Client never carried a title
    /// across a binding, so it resets either way.
    fn follow(&mut self, terminal: Option<&ClientTerminalBinding>) {
        if self.terminal.as_ref() != terminal {
            *self = Self {
                row: self.row.take(),
                ..Self::new(terminal)
            };
        }
    }

    /// A frame title arrives only when it differs from the last one for this terminal.
    fn observe_surface(&mut self, title: Option<String>) {
        if self.surface.as_ref() != Some(&title) {
            self.shown = title.clone().map(|text| Shown {
                published: false,
                text,
            });
            self.surface = Some(title);
        }
    }

    /// A row applies while its binding is the snapshot's, and arrives only when its title differs
    /// from the last one rows observed for this terminal, so a group republished for another
    /// member never overrides a newer frame title. `unavailable` keeps what is shown.
    fn apply_row(&mut self) {
        let Some(Row {
            binding: Some(binding),
            title: PresentationTitle::Observed { title },
        }) = &self.row
        else {
            return;
        };
        if self.terminal.as_ref() == Some(binding) && self.stream.as_ref() != Some(title) {
            self.shown = title.clone().map(|text| Shown {
                published: true,
                text,
            });
            self.stream = Some(title.clone());
        }
    }
}

impl PresentationTitles {
    pub(super) fn clear(&mut self) {
        self.cursors.clear();
    }

    /// Cursors captured by an older surface of the same Session, for Presentations this surface
    /// has none for yet. The next publication removes those it no longer holds.
    pub(super) fn inherit_older(&mut self, older: Self) {
        if self.session.is_none() {
            self.session.clone_from(&older.session);
        }
        if older.session != self.session {
            return;
        }
        for (presentation, cursor) in older.cursors {
            self.cursors.entry(presentation).or_insert(cursor);
        }
    }

    /// A publication can move a Presentation to another terminal or remove it; `terminal` is the
    /// Presentation's binding in it, `None` when the Presentation left.
    pub(super) fn follow<'b>(
        &mut self,
        session: &SessionId,
        terminal: impl Fn(&TerminalPresentationId) -> Option<Option<&'b ClientTerminalBinding>>,
    ) {
        self.enter(session);
        self.cursors
            .retain(|presentation, cursor| match terminal(presentation) {
                None => false,
                Some(binding) => {
                    cursor.follow(binding);
                    cursor.apply_row();
                    true
                }
            });
    }

    /// Rows of `presentation-titles` streams, each matched with the binding the snapshot holds
    /// for its Presentation now. Reports whether a shown title changed.
    pub(super) fn receive<'b>(
        &mut self,
        session: &SessionId,
        rows: Vec<PresentationTitleRow>,
        terminal: impl Fn(&TerminalPresentationId) -> Option<&'b ClientTerminalBinding>,
    ) -> bool {
        self.enter(session);
        let mut changed = false;
        for row in rows {
            let binding = terminal(&row.presentation_id);
            let cursor = self
                .cursors
                .entry(row.presentation_id)
                .or_insert_with(|| Cursor::new(binding));
            let before = cursor.shown.clone();
            cursor.follow(binding);
            cursor.row = Some(Row {
                binding: row.binding.as_ref().map(ClientTerminalBinding::from),
                title: row.title,
            });
            cursor.apply_row();
            changed |= cursor.shown != before;
        }
        changed
    }

    /// The raw title of a live frame this Client drew for a Presentation's current terminal.
    pub(super) fn observe_surface(
        &mut self,
        session: &SessionId,
        presentation: &TerminalPresentationId,
        terminal: Option<&ClientTerminalBinding>,
        title: Option<&str>,
    ) {
        self.enter(session);
        let projected = title.and_then(project_terminal_title);
        let cursor = self
            .cursors
            .entry(presentation.clone())
            .or_insert_with(|| Cursor::new(terminal));
        cursor.follow(terminal);
        cursor.observe_surface(projected);
        cursor.apply_row();
    }

    /// What a Presentation bound to `terminal` shows now. `live` is the raw title of a live frame
    /// for that terminal, which may have arrived since the last draw recorded one; the result is
    /// what recording it would show.
    pub(super) fn shown<'s>(
        &'s self,
        presentation: &TerminalPresentationId,
        terminal: Option<&ClientTerminalBinding>,
        live: Option<Option<&'s str>>,
    ) -> ShownTitle<'s> {
        let cursor = self.cursors.get(presentation);
        let current = cursor.filter(|cursor| cursor.terminal.as_ref() == terminal);
        let row = cursor.and_then(|cursor| cursor.row.as_ref());
        let stream = current.and_then(|cursor| cursor.stream.as_ref());
        let mut shown = current
            .and_then(|cursor| cursor.shown.as_ref())
            .map_or(ShownTitle::Program, |shown| {
                ShownTitle::Projected(shown.text.as_str())
            });
        if let Some(live) = live {
            let projected = live.and_then(project_terminal_title);
            let surface = current.and_then(|cursor| cursor.surface.as_ref());
            let recorded = current
                .and_then(|cursor| cursor.shown.as_ref())
                .is_some_and(|shown| !shown.published);
            if surface != Some(&projected) || recorded {
                shown = match (live, projected) {
                    (Some(live), Some(_)) => ShownTitle::Live(live),
                    _ => ShownTitle::Program,
                };
            }
        }
        if let Some(Row {
            binding: Some(binding),
            title: PresentationTitle::Observed { title },
        }) = row
            && terminal == Some(binding)
            && stream != Some(title)
        {
            shown = title
                .as_deref()
                .map_or(ShownTitle::Program, ShownTitle::Projected);
        }
        shown
    }

    /// A cursor names a Presentation of one Session; another Session starts with none.
    fn enter(&mut self, session: &SessionId) {
        if self.session.as_ref() != Some(session) {
            self.cursors.clear();
            self.session = Some(session.clone());
        }
    }
}
