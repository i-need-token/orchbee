//! The Settings page: every change is one edit the Server applies at once. Storage, transport
//! and live application remain separate owners.
use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};
use orchbee_agent_integration::IntegrationId;
use orchbee_workbench_protocol::{
    SettingScalar, SettingsFile, SettingsOperation, SettingsSnapshot, UserSettingsValues,
};
mod appearance;
pub(crate) use appearance::AppearancePreview;
mod catalog;
pub(crate) use catalog::{Category, Entry, Group};
mod choices;
pub(crate) use choices::{Choice, ChoicePicker};
mod editing;
mod input;
mod integrations;
pub(crate) use integrations::{Diagnosis, IntegrationJob, Integrations, JobEnd};
mod metadata;
mod page;
use page::{PageState, Pending};
pub(crate) use page::{PageView, Pane};
mod rows;
#[cfg(any(test, feature = "test-support"))]
mod test_support;
#[cfg(test)]
mod tests;
mod text;
mod values;
pub(crate) use input::{footer_bindings, key_edit, key_edit_for_view};
#[cfg(test)]
pub(crate) use metadata::SETTINGS;
#[cfg(test)]
use metadata::SettingEffect;
#[cfg(test)]
use metadata::SettingUnit;
pub(crate) use metadata::{SettingAvailability, SettingControl, setting, setting_for_row};
pub(crate) use rows::{Page, Row};
pub(crate) use values::scrollback_megabytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token(u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsInputReadiness {
    Recovery,
    Interactive,
}
/// Current Settings capture intent projected by the feature owner.
///
/// Pending and suspended surfaces retain recovery capture without claiming
/// ordinary controls. Interactive capture additionally requires proof that
/// the current whole surface completed a paint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingsInputContext {
    pub(crate) token: Token,
    pub(crate) phase: PresentationPhase,
    pub(crate) readiness: SettingsInputReadiness,
    pub(crate) remaining_text_bytes: usize,
}
/// Geometry evidence returned by the Settings renderer for one completed frame: the right
/// column's first line and line count, and whether it showed one column at a time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingsDraw {
    token: Token,
    lines: Option<(usize, usize)>,
    narrow: bool,
}
impl SettingsDraw {
    pub(crate) const fn surface(token: Token) -> Self {
        Self {
            token,
            lines: None,
            narrow: false,
        }
    }
    pub(crate) const fn token(self) -> Token {
        self.token
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Adjustment {
    pub(crate) backward: bool,
    pub(crate) forward: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdjustmentStep {
    Small,
    Large,
}
/// Painted controls act with the same authority as keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Control {
    Adjust {
        row: Row,
        forward: bool,
    },
    Back,
    Close,
    Search,
    Category(Category),
    /// Restores one row's keys to their defaults.
    Restore(Row),
    Choose(Choice),
    /// One Agent row's auxiliary action: open its read-only diagnosis.
    Explain(IntegrationId),
}
/// The newest settings the Server confirmed: the revision orders observations, `values` are the
/// common values the file gives, `set` names the keys the file sets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Confirmed {
    pub(crate) revision: u64,
    pub(crate) values: UserSettingsValues,
    file_path: Option<String>,
    set: Vec<String>,
    texts: Vec<(&'static str, Option<String>)>,
}
impl Confirmed {
    pub(crate) fn of(snapshot: &SettingsSnapshot) -> Self {
        let text = |key: &'static str| {
            let value = orchbee_workbench_protocol::SettingPath::parse(key)
                .ok()
                .and_then(|path| snapshot.values.get(&path).cloned().flatten());
            (
                key,
                match value {
                    Some(SettingScalar::Text(text)) => Some(text),
                    _ => None,
                },
            )
        };
        Self {
            revision: snapshot.revision.value(),
            values: orchbee_local_runtime::snapshot_values(snapshot),
            file_path: snapshot.file_path.clone(),
            set: snapshot
                .set
                .iter()
                .map(|key| key.as_str().to_owned())
                .collect(),
            texts: vec![text("terminal.new_cwd"), text("keys.prefix")],
        }
    }
    fn sets(&self, key: &str) -> bool {
        self.set.iter().any(|set| set == key)
    }
    fn text(&self, key: &str) -> Option<&str> {
        self.texts
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .and_then(|(_, value)| value.as_deref())
    }
}
/// What the newest snapshot says of the settings file, for the page's file line. The values the
/// page shows stay the last valid ones whatever the file holds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum FileState {
    /// The file parses, or there is none and the defaults apply.
    #[default]
    Usable,
    /// The file does not parse at `line` and `column`, both from one.
    SyntaxError {
        path: Option<String>,
        line: u32,
        column: u32,
    },
    /// The file exists but cannot be read, or is not a regular file.
    Unreadable { path: Option<String> },
}
impl FileState {
    pub(crate) fn of(snapshot: &SettingsSnapshot) -> Self {
        let path = snapshot.file_path.clone();
        match snapshot.file {
            SettingsFile::SyntaxError { line, column, .. } => {
                Self::SyntaxError { path, line, column }
            }
            SettingsFile::Unreadable { .. } => Self::Unreadable { path },
            SettingsFile::Ok {} | SettingsFile::Missing {} => Self::Usable,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Edit {
    Next,
    Previous,
    #[cfg(any(test, feature = "test-support"))]
    Select(Row),
    /// `←`/`→`: moves between the columns, cycles a selector, or moves a text cursor.
    Adjust(bool),
    /// `Tab`/`Shift+Tab`: moves focus to the rows or back to the categories.
    Pane(bool),
    Activate,
    Cancel,
    Insert(char),
    Backspace,
    /// Deletes after a text cursor; on a changed row, restores its default.
    Delete,
    Home,
    End,
    /// A shared readline key (`Ctrl+U`/`K`/`W`/`Y`, `Alt+B`/`F`/`D`) in a text field.
    Line(LineKey),
    Search,
    Page(bool),
    /// Opens or reruns the selected Agent's diagnosis.
    Explain,
}
pub(crate) struct Submission {
    pub(crate) token: Token,
    pub(crate) operation: SettingsOperation,
}
#[derive(Clone, Copy)]
pub(crate) enum View<'a> {
    Loading { token: Token },
    Page(PageView<'a>),
}
impl View<'_> {
    pub(crate) const fn token(self) -> Token {
        match self {
            Self::Loading { token } => token,
            Self::Page(page) => page.token(),
        }
    }
}
#[derive(Default)]
enum State {
    #[default]
    Closed,
    Loading {
        token: Token,
        visible: bool,
    },
    Open(Box<PageState>),
}
#[derive(Default)]
pub(crate) struct Editor {
    login_shell_supported: bool,
    reduced_color: bool,
    integrations_available: bool,
    /// Survives closing Settings: an accepted job keeps running and settles here.
    integrations: Integrations,
    /// Followed while Settings is closed, so a page that opens names the file at once.
    file: FileState,
    next: u64,
    state: State,
    presentation: PresentationPhase,
    surface_presented: bool,
}
impl Editor {
    pub(crate) fn new(login_shell_supported: bool) -> Self {
        Self {
            login_shell_supported,
            ..Self::default()
        }
    }
    /// Named themes are unavailable on a host below truecolor; an open list of one closes
    /// without applying its candidate. Returns whether a visible page needs a new paint.
    pub(crate) fn configure_reduced_color(&mut self, reduced_color: bool) -> bool {
        if self.reduced_color == reduced_color {
            return false;
        }
        self.reduced_color = reduced_color;
        if let State::Open(page) = &mut self.state {
            page.reduced_color = reduced_color;
            if reduced_color
                && page.picker.is_some_and(|picker| {
                    setting_for_row(picker.row()).is_some_and(|metadata| {
                        metadata.availability == SettingAvailability::NamedThemes
                    })
                })
            {
                page.picker = None;
            }
        }
        self.redraw();
        self.visible()
    }
    fn allocate(&mut self) -> Option<Token> {
        self.next = self.next.checked_add(1)?;
        Some(Token(self.next))
    }
    pub(crate) fn open(&mut self) -> Option<Token> {
        if !matches!(self.state, State::Closed) {
            return None;
        }
        let token = self.allocate()?;
        self.presentation = PresentationPhase::Pending;
        self.surface_presented = false;
        self.state = State::Loading {
            token,
            visible: true,
        };
        Some(token)
    }
    pub(crate) fn loaded(&mut self, token: Token, snapshot: &SettingsSnapshot) -> bool {
        if !matches!(self.state, State::Loading { token: current, .. } if current == token) {
            return false;
        }
        if matches!(self.state, State::Loading { visible: false, .. }) {
            self.state = State::Closed;
            self.surface_presented = false;
            return false;
        }
        self.surface_presented = false;
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            return false;
        };
        self.file = FileState::of(snapshot);
        let confirmed = Confirmed::of(snapshot);
        let mut page = PageState {
            login_shell_supported: self.login_shell_supported,
            reduced_color: self.reduced_color,
            integrations_available: self.integrations_available,
            token,
            values: confirmed.values.clone(),
            confirmed,
            file: self.file.clone(),
            pending: None,
            category: Category::General,
            pane: Pane::Categories,
            row: Row::Language,
            page: Page::General,
            parents: Vec::new(),
            viewport: 0,
            visible_rows: 0,
            follow_selection: true,
            activation_ready: false,
            drawn: false,
            narrow: false,
            profile: 0,
            argument: 0,
            text: None,
            invalid: false,
            invalid_field: None,
            confirm_delete: false,
            filter: LineEditor::new(String::new(), LengthLimit::Bytes(256)),
            searching: false,
            picker: None,
        };
        page.settle_focus();
        self.state = State::Open(Box::new(page));
        true
    }
    /// Follows a newer snapshot: its values show at once unless an edit is in flight, and its
    /// file state names a file the page cannot use. Returns whether a visible page changed.
    pub(crate) fn observe(&mut self, snapshot: &SettingsSnapshot) -> bool {
        let file = FileState::of(snapshot);
        let file_changed = self.file != file;
        self.file = file;
        let State::Open(page) = &mut self.state else {
            return false;
        };
        page.file = self.file.clone();
        let newer = snapshot.revision.value() > page.confirmed.revision;
        if newer {
            page.confirmed = Confirmed::of(snapshot);
            page.follow_confirmed();
        }
        if !newer && !file_changed {
            return false;
        }
        self.redraw();
        self.visible()
    }
    /// The Server applied the edit `token` names; the page shows the confirmed values.
    pub(crate) fn edited(&mut self, token: Token) -> bool {
        self.settle(token)
    }
    /// A read or an edit failed. A failed read closes the page; a refused or unanswered edit
    /// puts its row back to the confirmed value.
    pub(crate) fn failed(&mut self, token: Token) -> bool {
        if matches!(self.state, State::Loading { token: current, .. } if current == token) {
            self.state = State::Closed;
            self.surface_presented = false;
            return true;
        }
        self.settle(token)
    }
    fn settle(&mut self, token: Token) -> bool {
        let State::Open(page) = &mut self.state else {
            return false;
        };
        if page
            .pending
            .as_ref()
            .is_none_or(|pending| pending.token != token)
        {
            return false;
        }
        page.pending = None;
        page.follow_confirmed();
        self.redraw();
        true
    }
    pub(crate) fn visible(&self) -> bool {
        self.view().is_some()
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.presentation
    }
    pub(crate) fn input_context(&self) -> Option<SettingsInputContext> {
        let token = self.view()?.token();
        let readiness = if self.presentation.interactive() && self.surface_presented {
            SettingsInputReadiness::Interactive
        } else {
            SettingsInputReadiness::Recovery
        };
        Some(SettingsInputContext {
            token,
            phase: self.presentation,
            readiness,
            remaining_text_bytes: if readiness == SettingsInputReadiness::Interactive {
                self.remaining_text_bytes()
            } else {
                0
            },
        })
    }
    /// An edit the Server has not answered.
    pub(crate) fn in_flight(&self) -> bool {
        matches!(&self.state, State::Open(page) if page.pending.is_some())
    }
    pub(crate) fn retire(&mut self) {
        self.state = State::Closed;
        self.surface_presented = false;
    }
    /// The appearance the page shows: the confirmed or requested values, with an open list's
    /// candidate, or a focused theme line not yet in use, previewed over them.
    pub(crate) fn appearance_preview(&self) -> Option<AppearancePreview<'_>> {
        let State::Open(page) = &self.state else {
            return None;
        };
        let mut preview = AppearancePreview::of(&page.values);
        if let Some(picker) = page.picker {
            picker.candidate().preview(&mut preview);
        } else if let Row::ThemeOption(theme) = page.row
            && page.pane == Pane::Rows
            && !page.searching
            && page.text.is_none()
            && !page.reduced_color
            && !theme.current(&page.values)
        {
            Choice::ColorTheme(theme).preview(&mut preview);
        }
        Some(preview)
    }
    pub(crate) fn view(&self) -> Option<View<'_>> {
        match &self.state {
            State::Loading {
                token,
                visible: true,
            } => Some(View::Loading { token: *token }),
            State::Open(page) => Some(View::Page(PageView::new(page, &self.integrations))),
            _ => None,
        }
    }

    pub(crate) fn presented(&mut self, draw: SettingsDraw) {
        if self.view().is_none_or(|view| view.token() != draw.token()) {
            return;
        }
        self.presentation.presented();
        self.surface_presented = true;
        if let State::Open(page) = &mut self.state {
            page.drawn = true;
            page.narrow = draw.narrow;
            if let Some((first, visible)) = draw.lines {
                page.visible_rows = visible;
                page.viewport = first;
                page.activation_ready = page.focus_drawn(first, visible);
            } else {
                page.visible_rows = 0;
                page.activation_ready = false;
            }
        }
    }
    pub(crate) fn not_presented(&mut self) {
        self.surface_presented = false;
        if let State::Open(page) = &mut self.state {
            page.drawn = false;
            page.activation_ready = false;
            page.visible_rows = 0;
        }
    }
    /// Settles an evaluated geometry block for this Settings incarnation.
    ///
    /// A first blocked frame refuses the surface without cancelling an accepted read or edit.
    /// Once a frame has been admitted, the same block only suspends interaction.
    pub(crate) fn geometry_blocked(&mut self) -> Option<GeometrySettlement> {
        self.view()?;
        self.not_presented();
        let settlement = self.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.state = match std::mem::take(&mut self.state) {
                State::Loading { token, .. } => State::Loading {
                    token,
                    visible: false,
                },
                State::Open(_) => State::Closed,
                State::Closed => unreachable!("visible settings state"),
            };
        }
        Some(settlement)
    }
    pub(crate) fn paste(&mut self, text: &str) {
        if self.surface_presented
            && let State::Open(page) = &mut self.state
        {
            let searching = page.searching;
            let editor = if searching {
                Some(&mut page.filter)
            } else {
                page.text.as_mut()
            };
            let Some(editor) = editor else { return };
            page.invalid = text.contains('\0') || editor.insert(text).is_err();
            page.invalid_field = None;
            if searching {
                page.viewport = 0;
                page.follow_selection = true;
                page.settle_focus();
            }
            self.redraw();
        }
    }
    pub(crate) fn reject_paste(&mut self) {
        if self.surface_presented
            && let State::Open(page) = &mut self.state
            && (page.text.is_some() || page.searching)
        {
            page.invalid = true;
            page.invalid_field = None;
            self.redraw();
        }
    }
    pub(crate) fn remaining_text_bytes(&self) -> usize {
        match &self.state {
            State::Open(page) if page.searching => {
                256_usize.saturating_sub(page.filter.text().len())
            }
            State::Open(page) => page.text.as_ref().map_or(0, |text| {
                page.row
                    .text_limit()
                    .unwrap_or(0)
                    .saturating_sub(text.text().len())
            }),
            _ => 0,
        }
    }
    fn redraw(&mut self) {
        let Some(token) = self.allocate() else {
            self.state = State::Closed;
            self.surface_presented = false;
            return;
        };
        if let State::Open(page) = &mut self.state {
            page.token = token;
            page.drawn = false;
            page.activation_ready = page.follow_selection && self.surface_presented;
        }
    }

    /// A remote origin offers no integration management: remote hosts stay manual.
    pub(crate) fn configure_integrations(&mut self, available: bool) -> bool {
        if self.integrations_available == available {
            return false;
        }
        self.integrations_available = available;
        if let State::Open(page) = &mut self.state {
            page.integrations_available = available;
            if !available {
                while page.page.is_integrations() {
                    page.return_to_parent();
                }
                page.settle_focus();
            }
        }
        self.redraw();
        self.visible()
    }

    /// The job the page admitted and the transport has not yet taken.
    pub(crate) fn take_integration_job(&mut self) -> Option<IntegrationJob> {
        self.integrations.take_submission()
    }

    /// Applies a job end even while Settings is closed; returns whether a visible page changed.
    pub(crate) fn settle_integration(&mut self, end: JobEnd) -> bool {
        if !self.integrations.settle(end) {
            return false;
        }
        self.redraw();
        self.visible()
    }

    /// A new Session starts uninspected; a job of the old one is no longer followed.
    pub(crate) fn reset_integrations(&mut self) {
        self.integrations = Integrations::default();
        self.redraw();
    }
}
