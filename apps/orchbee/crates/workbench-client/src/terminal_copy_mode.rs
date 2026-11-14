//! Deterministic keyboard-copy state over one admitted terminal content surface.
//!
//! Pane membership, viewport transport, queries, Host selection commands and
//! clipboard effects stay in the native UI adapter. This core owns only the
//! volatile episode's cursor, search draft, selection intent and busy gates.

use std::num::NonZeroU64;

use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};
use crate::{
    terminal_content::TerminalScreen,
    terminal_copy::{CopyMotion, CopySearchDirection, CopySearchResult, TerminalTextPoint},
    terminal_text::is_forbidden_terminal_scalar,
};

const MAXIMUM_SEARCH_SCALARS: usize = 512;

macro_rules! identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub(crate) struct $name(NonZeroU64);

        impl $name {
            pub(crate) const fn new(value: NonZeroU64) -> Self {
                Self(value)
            }
        }
    };
}

identity!(EpisodeId);
identity!(QueryId);

impl EpisodeId {
    pub(crate) const fn get(self) -> NonZeroU64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Surface {
    pub(crate) screen: TerminalScreen,
    pub(crate) generation: NonZeroU64,
    pub(crate) revision: u64,
    pub(crate) columns: u16,
    pub(crate) total_rows: u64,
    pub(crate) start_row: u64,
    pub(crate) row_count: u16,
}

impl Surface {
    pub(crate) fn checked(
        screen: TerminalScreen,
        generation: NonZeroU64,
        revision: u64,
        columns: u16,
        total_rows: u64,
        start_row: u64,
        row_count: u16,
    ) -> Option<Self> {
        let end = start_row.checked_add(u64::from(row_count))?;
        if columns == 0
            || row_count == 0
            || total_rows == 0
            || total_rows > u64::from(u32::MAX) + 1
            || start_row >= total_rows
            || end > total_rows
        {
            return None;
        }
        Some(Self {
            screen,
            generation,
            revision,
            columns,
            total_rows,
            start_row,
            row_count,
        })
    }

    pub(crate) fn contains(self, point: TerminalTextPoint) -> bool {
        u64::from(point.row) >= self.start_row
            && u64::from(point.row) < self.start_row + u64::from(self.row_count)
            && u64::from(point.row) < self.total_rows
            && point.col < self.columns
    }

    pub(crate) fn local(self, point: TerminalTextPoint) -> Option<CopyModeCursor> {
        if !self.contains(point) {
            return None;
        }
        Some(CopyModeCursor {
            column: point.col,
            row: u16::try_from(u64::from(point.row) - self.start_row).ok()?,
        })
    }

    pub(crate) fn point(self, cursor: CopyModeCursor) -> Option<TerminalTextPoint> {
        if cursor.column >= self.columns || cursor.row >= self.row_count {
            return None;
        }
        let row = self.start_row.checked_add(u64::from(cursor.row))?;
        if row >= self.total_rows {
            return None;
        }
        Some(TerminalTextPoint {
            row: u32::try_from(row).ok()?,
            col: cursor.column,
        })
    }

    pub(crate) fn bottom_left(self) -> TerminalTextPoint {
        let row = (self.start_row + u64::from(self.row_count) - 1).min(self.total_rows - 1);
        TerminalTextPoint {
            row: row as u32,
            col: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyModeCursor {
    pub(crate) column: u16,
    pub(crate) row: u16,
}

pub(crate) use crate::localization::CopyModeStatus;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CopyModeSearchView<'a> {
    pub(crate) direction: CopySearchDirection,
    pub(crate) draft: &'a str,
    /// UTF-8 byte offset, always an extended-grapheme boundary.
    pub(crate) cursor: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CopyModeView<'a> {
    pub(crate) status: CopyModeStatus,
    pub(crate) cursor: Option<CopyModeCursor>,
    pub(crate) selecting: bool,
    pub(crate) search: Option<CopyModeSearchView<'a>>,
    pub(crate) pointer: Option<CopyModeToken>,
}

/// A painted control may act only on this episode and local state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyModeToken {
    episode: EpisodeId,
    surface: Option<Surface>,
    desired: Option<TerminalTextPoint>,
    status: CopyModeStatus,
    selecting: bool,
    searching: Option<CopySearchDirection>,
}

/// Selection acknowledgements can advance during a drag; content identity cannot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopySurfaceToken {
    pub(crate) episode: EpisodeId,
    pub(crate) surface: Surface,
}

/// The local crop origin one presented frame painted for its copy episode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CopyCrop {
    pub(crate) episode: EpisodeId,
    pub(crate) origin: CopyModeCursor,
}

impl CopyModeToken {
    pub(crate) const fn crop(self, origin: CopyModeCursor) -> CopyCrop {
        CopyCrop {
            episode: self.episode,
            origin,
        }
    }

    /// Painting retains the admitted surface while search or effects suspend pointer input.
    pub(crate) const fn content_surface(self) -> Option<Surface> {
        self.surface
    }

    pub(crate) fn surface(self) -> Option<CopySurfaceToken> {
        (self.searching.is_none()
            && matches!(
                self.status,
                CopyModeStatus::Ready | CopyModeStatus::Selecting
            ))
        .then(|| {
            self.surface.map(|surface| CopySurfaceToken {
                episode: self.episode,
                surface,
            })
        })
        .flatten()
    }
    pub(crate) fn pending(&mut self) {
        self.status = CopyModeStatus::Moving;
    }
}

impl CopyModeView<'_> {
    pub(crate) fn entering(episode: EpisodeId) -> Self {
        Self {
            status: CopyModeStatus::Entering,
            cursor: None,
            selecting: false,
            search: None,
            pointer: Some(CopyModeToken {
                episode,
                surface: None,
                desired: None,
                status: CopyModeStatus::Entering,
                selecting: false,
                searching: None,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyControl {
    Copy,
    ToggleSelection,
    Search,
    RepeatSearch { reverse: bool },
    Exit,
    Help,
    SubmitSearch,
    CancelSearch,
    ReverseSearch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchEdit<'a> {
    Insert(&'a str),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    /// A shared readline key (`Ctrl+U`/`K`/`W`/`Y`, `Alt+B`/`F`/`D`).
    Line(LineKey),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionEffect {
    Start(TerminalTextPoint),
    Update(TerminalTextPoint),
    End(TerminalTextPoint),
    Cancel,
    ExportActive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Navigation {
    Reveal(TerminalTextPoint),
    Oldest,
    Newest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Effect {
    Navigate(Navigation),
    Selection(SelectionEffect),
    Motion {
        identity: QueryId,
        cursor: TerminalTextPoint,
        motion: CopyMotion,
    },
    Search {
        identity: QueryId,
        cursor: TerminalTextPoint,
        query: Box<str>,
        direction: CopySearchDirection,
        previous: Option<crate::terminal_copy::TerminalTextRange>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    Busy,
    Invalid,
    Limit,
    IdentityExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Flight {
    None,
    Viewport,
    Query(QueryId),
    Selection,
    Export,
    Clipboard(NonZeroU64),
}

#[derive(Clone, Debug)]
struct SearchDraft {
    direction: CopySearchDirection,
    editor: LineEditor,
}

#[derive(Clone, Debug)]
struct LastSearch {
    query: Box<str>,
    base_direction: CopySearchDirection,
    range: Option<crate::terminal_copy::TerminalTextRange>,
}

/// One admitted copy episode. Every point is an absolute retained-buffer
/// point; rendering projects it through the current exact surface.
pub(crate) struct Mode {
    episode: EpisodeId,
    surface: Surface,
    cursor: TerminalTextPoint,
    desired: TerminalTextPoint,
    selecting: bool,
    selection_point: Option<TerminalTextPoint>,
    flight: Flight,
    pending_effect: Option<Effect>,
    search: Option<SearchDraft>,
    last_search: Option<LastSearch>,
    pending_search_base: Option<CopySearchDirection>,
    next_query: u64,
}

impl Mode {
    pub(crate) fn new(
        episode: EpisodeId,
        surface: Surface,
        cursor: TerminalTextPoint,
    ) -> Option<Self> {
        surface.contains(cursor).then_some(Self {
            episode,
            surface,
            cursor,
            desired: cursor,
            selecting: false,
            selection_point: None,
            flight: Flight::None,
            pending_effect: None,
            search: None,
            last_search: None,
            pending_search_base: None,
            next_query: 0,
        })
    }

    pub(crate) const fn episode(&self) -> EpisodeId {
        self.episode
    }

    pub(crate) const fn surface(&self) -> Surface {
        self.surface
    }

    #[cfg(test)]
    pub(crate) const fn cursor(&self) -> TerminalTextPoint {
        self.cursor
    }

    pub(crate) const fn desired(&self) -> TerminalTextPoint {
        self.desired
    }

    pub(crate) fn view(&self) -> CopyModeView<'_> {
        let status = match self.flight {
            Flight::None => CopyModeStatus::Ready,
            Flight::Selection => CopyModeStatus::Selecting,
            Flight::Viewport => CopyModeStatus::Moving,
            Flight::Query(_) => CopyModeStatus::Querying,
            Flight::Export => CopyModeStatus::Exporting,
            Flight::Clipboard(_) => CopyModeStatus::Clipboard,
        };
        CopyModeView {
            status,
            cursor: (!matches!(self.flight, Flight::Viewport))
                .then(|| self.surface.local(self.cursor))
                .flatten(),
            selecting: self.selecting,
            search: self.search.as_ref().map(|search| CopyModeSearchView {
                direction: search.direction,
                draft: search.editor.text(),
                cursor: search.editor.cursor(),
            }),
            pointer: Some(CopyModeToken {
                episode: self.episode,
                surface: Some(self.surface),
                desired: Some(self.desired),
                status,
                selecting: self.selecting,
                searching: self.search.as_ref().map(|search| search.direction),
            }),
        }
    }

    pub(crate) fn take_effect(&mut self) -> Option<Effect> {
        self.pending_effect.take()
    }

    pub(crate) fn request_local_target(
        &mut self,
        target: TerminalTextPoint,
    ) -> Result<(), Refusal> {
        if self.search.is_some()
            || matches!(
                self.flight,
                Flight::Query(_) | Flight::Export | Flight::Clipboard(_)
            )
        {
            return Err(Refusal::Busy);
        }
        if target.col >= self.surface.columns || u64::from(target.row) >= self.surface.total_rows {
            return Err(Refusal::Invalid);
        }
        self.desired = target;
        if matches!(self.flight, Flight::Viewport | Flight::Selection) {
            return Ok(());
        }
        self.drive_desired()
    }

    pub(crate) fn request_page(&mut self, older: bool) -> Result<(), Refusal> {
        let distance = u32::from(self.surface.row_count);
        let row = if older {
            self.desired.row.saturating_sub(distance)
        } else {
            self.desired
                .row
                .saturating_add(distance)
                .min((self.surface.total_rows - 1) as u32)
        };
        self.request_local_target(TerminalTextPoint {
            row,
            col: self.desired.col,
        })
    }

    pub(crate) fn request_oldest(&mut self) -> Result<(), Refusal> {
        if self.busy_for_incompatible() {
            return Err(Refusal::Busy);
        }
        self.desired = TerminalTextPoint {
            row: 0,
            col: self.desired.col.min(self.surface.columns - 1),
        };
        if self.surface.contains(self.desired) {
            self.drive_desired()
        } else if matches!(self.flight, Flight::None) {
            self.flight = Flight::Viewport;
            self.pending_effect = Some(Effect::Navigate(Navigation::Oldest));
            Ok(())
        } else {
            Ok(())
        }
    }

    pub(crate) fn request_newest(&mut self) -> Result<(), Refusal> {
        if self.busy_for_incompatible() {
            return Err(Refusal::Busy);
        }
        self.desired = TerminalTextPoint {
            row: (self.surface.total_rows - 1) as u32,
            col: self.desired.col.min(self.surface.columns - 1),
        };
        if self.surface.contains(self.desired) {
            self.drive_desired()
        } else if matches!(self.flight, Flight::None) {
            self.flight = Flight::Viewport;
            self.pending_effect = Some(Effect::Navigate(Navigation::Newest));
            Ok(())
        } else {
            Ok(())
        }
    }

    pub(crate) fn request_motion(&mut self, motion: CopyMotion) -> Result<(), Refusal> {
        if self.search.is_some() || !matches!(self.flight, Flight::None) {
            return Err(Refusal::Busy);
        }
        let identity = self.next_query()?;
        self.flight = Flight::Query(identity);
        self.pending_effect = Some(Effect::Motion {
            identity,
            cursor: self.cursor,
            motion,
        });
        Ok(())
    }

    pub(crate) fn begin_search(&mut self, direction: CopySearchDirection) -> Result<(), Refusal> {
        if !matches!(self.flight, Flight::None) || self.search.is_some() {
            return Err(Refusal::Busy);
        }
        self.search = Some(SearchDraft {
            direction,
            editor: LineEditor::new(String::new(), LengthLimit::Scalars(MAXIMUM_SEARCH_SCALARS)),
        });
        Ok(())
    }

    pub(crate) fn edit_search(&mut self, edit: SearchEdit<'_>) -> Result<bool, Refusal> {
        let Some(search) = self.search.as_mut() else {
            return Err(Refusal::Invalid);
        };
        let editor = &mut search.editor;
        let changed = match edit {
            SearchEdit::Insert(value) => {
                if value.chars().any(|character| {
                    character.is_control() || is_forbidden_terminal_scalar(character)
                }) {
                    return Err(Refusal::Limit);
                }
                editor.insert(value).map_err(|_| Refusal::Limit)?;
                true
            }
            SearchEdit::Backspace => editor.backspace(),
            SearchEdit::Delete => editor.delete(),
            SearchEdit::Left => editor.left(),
            SearchEdit::Right => editor.right(),
            SearchEdit::Home => editor.home(),
            SearchEdit::End => editor.end(),
            SearchEdit::Line(key) => editor.apply(key).map_err(|_| Refusal::Limit)?,
        };
        Ok(changed)
    }

    /// Submit the draft. Empty search is a local no-op with no content read.
    pub(crate) fn submit_search(&mut self) -> Result<bool, Refusal> {
        if !matches!(self.flight, Flight::None) {
            return Err(Refusal::Busy);
        }
        let Some(search) = self.search.take() else {
            return Err(Refusal::Invalid);
        };
        if search.editor.text().is_empty() {
            return Ok(false);
        }
        self.issue_search(
            search.editor.text().into(),
            search.direction,
            search.direction,
            None,
        )?;
        Ok(true)
    }

    pub(crate) fn repeat_search(&mut self, reverse: bool) -> Result<(), Refusal> {
        if self.search.is_some() || !matches!(self.flight, Flight::None) {
            return Err(Refusal::Busy);
        }
        let Some(last) = self.last_search.clone() else {
            return Err(Refusal::Invalid);
        };
        let direction = if reverse {
            reverse_direction(last.base_direction)
        } else {
            last.base_direction
        };
        self.issue_search(last.query, direction, last.base_direction, last.range)
    }

    fn issue_search(
        &mut self,
        query: Box<str>,
        direction: CopySearchDirection,
        base_direction: CopySearchDirection,
        previous: Option<crate::terminal_copy::TerminalTextRange>,
    ) -> Result<(), Refusal> {
        let identity = self.next_query()?;
        self.flight = Flight::Query(identity);
        self.pending_search_base = Some(base_direction);
        self.pending_effect = Some(Effect::Search {
            identity,
            cursor: self.cursor,
            query,
            direction,
            previous,
        });
        Ok(())
    }

    pub(crate) fn query_motion_completed(
        &mut self,
        identity: QueryId,
        point: TerminalTextPoint,
    ) -> Result<(), Refusal> {
        if self.flight != Flight::Query(identity) {
            return Err(Refusal::Invalid);
        }
        if self.pending_search_base.is_some() {
            return Err(Refusal::Invalid);
        }
        self.flight = Flight::None;
        self.request_local_target(point)
    }

    pub(crate) fn query_search_completed(
        &mut self,
        identity: QueryId,
        query: Box<str>,
        result: CopySearchResult,
    ) -> Result<bool, Refusal> {
        if self.flight != Flight::Query(identity) {
            return Err(Refusal::Invalid);
        }
        let base_direction = self.pending_search_base.take().ok_or(Refusal::Invalid)?;
        self.flight = Flight::None;
        let range = result
            .current
            .and_then(|index| result.matches.get(index).copied());
        self.last_search = Some(LastSearch {
            query,
            base_direction,
            range,
        });
        let Some(found) = range else {
            return Ok(false);
        };
        self.request_local_target(found.start)?;
        Ok(true)
    }

    pub(crate) fn query_failed(&mut self, identity: QueryId) -> bool {
        if self.flight != Flight::Query(identity) {
            return false;
        }
        self.pending_search_base = None;
        self.flight = Flight::None;
        let _ = self.drive_desired();
        true
    }

    pub(crate) fn toggle_selection(&mut self) -> Result<(), Refusal> {
        if self.search.is_some() || !matches!(self.flight, Flight::None) {
            return Err(Refusal::Busy);
        }
        if self.selection_point.is_some() {
            self.selecting = false;
            self.selection_point = None;
            self.flight = Flight::Selection;
            self.pending_effect = Some(Effect::Selection(SelectionEffect::Cancel));
        } else {
            self.selecting = true;
            self.selection_point = Some(self.cursor);
            self.flight = Flight::Selection;
            self.pending_effect = Some(Effect::Selection(SelectionEffect::Start(self.cursor)));
        }
        Ok(())
    }

    /// A press anchors a semantic selection; further presses extend that selection.
    pub(crate) fn start_pointer_selection(
        &mut self,
        target: TerminalTextPoint,
    ) -> Result<(), Refusal> {
        if self.selecting {
            return self.request_local_target(target);
        }
        if self.search.is_some() || !matches!(self.flight, Flight::None) {
            return Err(Refusal::Busy);
        }
        if !self.surface.contains(target) {
            return Err(Refusal::Invalid);
        }
        self.cursor = target;
        self.desired = target;
        self.selecting = true;
        self.selection_point = Some(target);
        self.flight = Flight::Selection;
        self.pending_effect = Some(Effect::Selection(SelectionEffect::Start(target)));
        Ok(())
    }

    pub(crate) fn cancel_search(&mut self) {
        self.search = None;
    }

    pub(crate) fn reverse_search(&mut self) {
        if let Some(search) = &mut self.search {
            search.direction = match search.direction {
                CopySearchDirection::Forward => CopySearchDirection::Backward,
                CopySearchDirection::Backward => CopySearchDirection::Forward,
            };
        }
    }

    pub(crate) fn request_copy(&mut self, host_selection_active: bool) -> Result<(), Refusal> {
        if self.search.is_some() || !matches!(self.flight, Flight::None) {
            return Err(Refusal::Busy);
        }
        if self.selecting {
            self.flight = Flight::Export;
            self.pending_effect = Some(Effect::Selection(SelectionEffect::End(self.cursor)));
            return Ok(());
        }
        if host_selection_active {
            self.flight = Flight::Export;
            self.pending_effect = Some(Effect::Selection(SelectionEffect::ExportActive));
            return Ok(());
        }
        Err(Refusal::Invalid)
    }

    pub(crate) fn selection_settled(&mut self, active: bool) -> Result<(), Refusal> {
        if !matches!(self.flight, Flight::Selection) {
            return Ok(());
        }
        self.flight = Flight::None;
        if !active {
            self.selecting = false;
            self.selection_point = None;
            return self.drive_desired();
        }
        if self.selecting {
            self.drive_desired()?;
        }
        Ok(())
    }

    pub(crate) fn export_ready(&mut self, sequence: NonZeroU64) -> Result<(), Refusal> {
        if !matches!(self.flight, Flight::Export) {
            return Err(Refusal::Invalid);
        }
        self.flight = Flight::Clipboard(sequence);
        Ok(())
    }

    pub(crate) fn export_failed(&mut self, retain_selection: bool) -> bool {
        if !matches!(self.flight, Flight::Export | Flight::Clipboard(_)) {
            return false;
        }
        self.flight = Flight::None;
        self.selecting = false;
        if !retain_selection {
            self.selection_point = None;
        }
        true
    }

    pub(crate) fn clipboard_identity(&self) -> Option<NonZeroU64> {
        match self.flight {
            Flight::Clipboard(sequence) => Some(sequence),
            _ => None,
        }
    }

    pub(crate) fn awaiting_selection(&self) -> bool {
        matches!(self.flight, Flight::Selection)
    }

    pub(crate) fn selection_effect_pending(&self) -> bool {
        matches!(self.pending_effect.as_ref(), Some(Effect::Selection(_)))
    }

    pub(crate) fn awaiting_export(&self) -> bool {
        matches!(self.flight, Flight::Export)
    }

    /// A matching delivered effect is the only effect outcome that exits.
    pub(crate) fn clipboard_not_delivered(&mut self, sequence: NonZeroU64) -> bool {
        if self.flight != Flight::Clipboard(sequence) {
            return false;
        }
        self.flight = Flight::None;
        self.selecting = false;
        true
    }

    pub(crate) fn viewport_settled(&mut self, surface: Surface) -> Result<(), Refusal> {
        if surface.screen != self.surface.screen
            || surface.generation != self.surface.generation
            || surface.columns != self.surface.columns
            || surface.row_count != self.surface.row_count
            || !matches!(self.flight, Flight::Viewport)
        {
            return Err(Refusal::Invalid);
        }
        self.surface = surface;
        self.last_search = None;
        self.flight = Flight::None;
        if self.desired.col >= surface.columns || u64::from(self.desired.row) >= surface.total_rows
        {
            return Err(Refusal::Invalid);
        }
        self.drive_desired()
    }

    /// Adopt a revision-only repaint. Primary history keeps the prior local
    /// row offset; alternate keeps the same local cell. A structural change is
    /// rejected so the wrapper can cancel the episode.
    pub(crate) fn revision_points(
        &self,
        surface: Surface,
    ) -> Result<(TerminalTextPoint, TerminalTextPoint), Refusal> {
        if surface.screen != self.surface.screen
            || surface.generation != self.surface.generation
            || surface.columns != self.surface.columns
            || surface.row_count != self.surface.row_count
            || surface.total_rows < 1
        {
            return Err(Refusal::Invalid);
        }
        let local = self.surface.local(self.cursor).ok_or(Refusal::Invalid)?;
        let desired_local = self.surface.local(self.desired);
        let cursor = surface.point(local).ok_or(Refusal::Invalid)?;
        let desired = desired_local
            .and_then(|cursor| surface.point(cursor))
            .unwrap_or(cursor);
        Ok((cursor, desired))
    }

    pub(crate) fn revision(
        &mut self,
        surface: Surface,
        cursor: TerminalTextPoint,
        desired: TerminalTextPoint,
    ) -> Result<(), Refusal> {
        self.revision_points(surface)?;
        if !surface.contains(cursor) || !surface.contains(desired) {
            return Err(Refusal::Invalid);
        }
        self.surface = surface;
        self.cursor = cursor;
        self.desired = desired;
        self.last_search = None;
        self.drive_desired()
    }

    fn drive_desired(&mut self) -> Result<(), Refusal> {
        if !matches!(self.flight, Flight::None) {
            return Ok(());
        }
        if !self.surface.contains(self.desired) {
            self.flight = Flight::Viewport;
            self.pending_effect = Some(Effect::Navigate(Navigation::Reveal(self.desired)));
            return Ok(());
        }
        self.cursor = self.desired;
        if self.selecting && self.selection_point != Some(self.cursor) {
            self.selection_point = Some(self.cursor);
            self.flight = Flight::Selection;
            self.pending_effect = Some(Effect::Selection(SelectionEffect::Update(self.cursor)));
        }
        Ok(())
    }

    fn next_query(&mut self) -> Result<QueryId, Refusal> {
        self.next_query = self
            .next_query
            .checked_add(1)
            .ok_or(Refusal::IdentityExhausted)?;
        NonZeroU64::new(self.next_query)
            .map(QueryId::new)
            .ok_or(Refusal::IdentityExhausted)
    }

    fn busy_for_incompatible(&self) -> bool {
        self.search.is_some()
            || matches!(
                self.flight,
                Flight::Query(_) | Flight::Export | Flight::Clipboard(_)
            )
    }
}

const fn reverse_direction(direction: CopySearchDirection) -> CopySearchDirection {
    match direction {
        CopySearchDirection::Forward => CopySearchDirection::Backward,
        CopySearchDirection::Backward => CopySearchDirection::Forward,
    }
}

#[cfg(test)]
mod tests;
