//! One key map serves dispatch and the hints line: text editing owns printable keys, and the
//! hints name the keys the focused row answers.
use super::{Edit, Page, PageView, Pane, Row, SettingControl, View, setting_for_row};
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::terminal_text::line_editor::LineKey;

#[derive(Clone, Copy)]
struct BindingSpec {
    binding: Binding,
    edit: Edit,
    list_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalBinding {
    Search,
    Explain,
    PageUp,
    PageDown,
    Exit,
    Primary,
    PreviousPane,
    NextPane,
    PreviousRow,
    NextRow,
    Left,
    Right,
    Home,
    End,
    HomeEmacs,
    EndEmacs,
    Backspace,
    Delete,
}

impl LocalBinding {
    const fn spec(self) -> BindingSpec {
        let (code, mods, edit, list_only) = match self {
            Self::Search => (KeyCode::Char('/'), Modifiers::NONE, Edit::Search, true),
            Self::Explain => (KeyCode::Char('e'), Modifiers::NONE, Edit::Explain, true),
            Self::PageUp => (KeyCode::PageUp, Modifiers::NONE, Edit::Page(false), true),
            Self::PageDown => (KeyCode::PageDown, Modifiers::NONE, Edit::Page(true), true),
            Self::Exit => (KeyCode::Esc, Modifiers::NONE, Edit::Cancel, false),
            Self::Primary => (KeyCode::Enter, Modifiers::NONE, Edit::Activate, false),
            Self::PreviousPane => (
                KeyCode::Tab,
                Modifiers::NONE.with_shift(),
                Edit::Pane(false),
                true,
            ),
            Self::NextPane => (KeyCode::Tab, Modifiers::NONE, Edit::Pane(true), true),
            Self::PreviousRow => (KeyCode::Up, Modifiers::NONE, Edit::Previous, false),
            Self::NextRow => (KeyCode::Down, Modifiers::NONE, Edit::Next, false),
            Self::Left => (KeyCode::Left, Modifiers::NONE, Edit::Adjust(false), false),
            Self::Right => (KeyCode::Right, Modifiers::NONE, Edit::Adjust(true), false),
            Self::Home => (KeyCode::Home, Modifiers::NONE, Edit::Home, false),
            Self::End => (KeyCode::End, Modifiers::NONE, Edit::End, false),
            Self::HomeEmacs => (
                KeyCode::Char('a'),
                Modifiers::NONE.with_ctrl(),
                Edit::Home,
                false,
            ),
            Self::EndEmacs => (
                KeyCode::Char('e'),
                Modifiers::NONE.with_ctrl(),
                Edit::End,
                false,
            ),
            Self::Backspace => (KeyCode::Backspace, Modifiers::NONE, Edit::Backspace, false),
            Self::Delete => (KeyCode::Delete, Modifiers::NONE, Edit::Delete, false),
        };
        BindingSpec {
            binding: Binding::new(code, mods),
            edit,
            list_only,
        }
    }
}

const DISPATCH_ORDER: [LocalBinding; 18] = [
    LocalBinding::Search,
    LocalBinding::Explain,
    LocalBinding::PageUp,
    LocalBinding::PageDown,
    LocalBinding::Exit,
    LocalBinding::Primary,
    LocalBinding::PreviousPane,
    LocalBinding::NextPane,
    LocalBinding::PreviousRow,
    LocalBinding::NextRow,
    LocalBinding::Left,
    LocalBinding::Right,
    LocalBinding::Home,
    LocalBinding::End,
    LocalBinding::HomeEmacs,
    LocalBinding::EndEmacs,
    LocalBinding::Backspace,
    LocalBinding::Delete,
];

pub(crate) use crate::localization::SettingsHintLabel;

/// One hint backed by the same keys dispatch uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SettingsFooterBinding {
    keys: &'static [LocalBinding],
    label: SettingsHintLabel,
}

impl SettingsFooterBinding {
    const fn new(keys: &'static [LocalBinding], label: SettingsHintLabel) -> Self {
        Self { keys, label }
    }

    pub(crate) const fn label(self) -> SettingsHintLabel {
        self.label
    }

    pub(crate) fn bindings(self) -> impl ExactSizeIterator<Item = Binding> {
        self.keys.iter().map(|key| key.spec().binding)
    }
}

const EXIT: &[LocalBinding] = &[LocalBinding::Exit];
const PRIMARY: &[LocalBinding] = &[LocalBinding::Primary];
const MOVE: &[LocalBinding] = &[LocalBinding::PreviousRow, LocalBinding::NextRow];
const SWITCH: &[LocalBinding] = &[LocalBinding::Left, LocalBinding::Right];
const SEARCH: &[LocalBinding] = &[LocalBinding::Search];
const RESTORE: &[LocalBinding] = &[LocalBinding::Delete];
const EXPLAIN: &[LocalBinding] = &[LocalBinding::Explain];

/// What `Enter` does on a row, as its hint names it.
fn primary_label(page: PageView<'_>, row: Row) -> SettingsHintLabel {
    match row {
        Row::LaunchProfiles | Row::AgentIntegrations => SettingsHintLabel::Open,
        Row::Back => SettingsHintLabel::Back,
        Row::ThemeOption(_) => SettingsHintLabel::Select,
        Row::Agent(_) => SettingsHintLabel::Toggle,
        row if page.checked(row).is_some() => SettingsHintLabel::Toggle,
        row if setting_for_row(row)
            .is_some_and(|metadata| !matches!(metadata.control, SettingControl::Boolean))
            || row.text_limit().is_some() =>
        {
            SettingsHintLabel::Change
        }
        _ => SettingsHintLabel::Select,
    }
}

/// The hints for the page's current state, local return first.
pub(crate) fn footer_bindings(view: View<'_>) -> Vec<SettingsFooterBinding> {
    let hint = SettingsFooterBinding::new;
    let page = match view {
        View::Loading { .. } => return vec![hint(EXIT, SettingsHintLabel::Close)],
        View::Page(page) => page,
    };
    if page.confirm_delete() {
        return vec![
            hint(EXIT, SettingsHintLabel::Cancel),
            hint(PRIMARY, SettingsHintLabel::Confirm),
        ];
    }
    if page.text().is_some() {
        return vec![
            hint(EXIT, SettingsHintLabel::Cancel),
            hint(PRIMARY, SettingsHintLabel::Accept),
        ];
    }
    if page.searching() {
        return vec![
            hint(EXIT, SettingsHintLabel::Back),
            hint(PRIMARY, SettingsHintLabel::GoTo),
        ];
    }
    if page.picker().is_some() {
        return vec![
            hint(EXIT, SettingsHintLabel::Cancel),
            hint(MOVE, SettingsHintLabel::Move),
            hint(PRIMARY, SettingsHintLabel::Accept),
        ];
    }
    let selected = page.selected();
    match page.page() {
        Page::Diagnosis(_) => {
            return vec![
                hint(EXIT, SettingsHintLabel::Back),
                hint(EXPLAIN, SettingsHintLabel::Explain),
            ];
        }
        Page::Integrations => {
            let mut hints = vec![
                hint(EXIT, SettingsHintLabel::Back),
                hint(MOVE, SettingsHintLabel::Move),
                hint(PRIMARY, primary_label(page, selected)),
            ];
            if matches!(selected, Row::Agent(_)) {
                hints.push(hint(EXPLAIN, SettingsHintLabel::Explain));
            }
            return hints;
        }
        Page::Profiles => {
            let mut hints = vec![
                hint(EXIT, SettingsHintLabel::Back),
                hint(MOVE, SettingsHintLabel::Move),
                hint(PRIMARY, primary_label(page, selected)),
            ];
            if matches!(selected, Row::Profile | Row::Argument) {
                hints.push(hint(SWITCH, SettingsHintLabel::Change));
            }
            return hints;
        }
        Page::General => {}
    }
    if page.pane() == Pane::Categories {
        return vec![
            hint(EXIT, SettingsHintLabel::Close),
            hint(MOVE, SettingsHintLabel::Move),
            hint(PRIMARY, SettingsHintLabel::Open),
            hint(SEARCH, SettingsHintLabel::Search),
        ];
    }
    let previewing = matches!(selected, Row::ThemeOption(theme) if !theme.current(page.values()));
    let exit = if previewing {
        SettingsHintLabel::Cancel
    } else if page.narrow() || !page.query().is_empty() {
        SettingsHintLabel::Back
    } else {
        SettingsHintLabel::Close
    };
    let mut hints = vec![hint(EXIT, exit), hint(MOVE, SettingsHintLabel::Move)];
    if !page.narrow() {
        hints.push(hint(SWITCH, SettingsHintLabel::Switch));
    }
    hints.push(hint(PRIMARY, primary_label(page, selected)));
    hints.push(hint(SEARCH, SettingsHintLabel::Search));
    if page.changed(selected) && page.pending_row().is_none() {
        hints.push(hint(RESTORE, SettingsHintLabel::Restore));
    }
    hints
}

/// Applies the current state's key map; a confirmation admits only its two keys.
pub(crate) fn key_edit_for_view(event: KeyEvent, view: View<'_>) -> Option<Edit> {
    let text_entry = matches!(view, View::Page(page) if page.text().is_some() || page.searching());
    let edit = key_edit(event, text_entry)?;
    if matches!(edit, Edit::Explain)
        && !matches!(view, View::Page(page) if page.page().is_integrations())
    {
        return None;
    }
    if matches!(view, View::Page(page) if page.confirm_delete()) {
        match edit {
            Edit::Cancel => Some(edit),
            Edit::Activate if LocalBinding::Primary.spec().binding.matches(event) => Some(edit),
            _ => None,
        }
    } else {
        Some(edit)
    }
}

pub(crate) fn key_edit(event: KeyEvent, text_entry: bool) -> Option<Edit> {
    // Legacy CR arrives as Ctrl+M, but an explicitly modified named Enter is a different key.
    if event.kind == KeyKind::Release
        || (event.code == KeyCode::Enter && event.mods != Modifiers::NONE)
    {
        return None;
    }
    if let Some(edit) = DISPATCH_ORDER.iter().find_map(|key| {
        let spec = key.spec();
        if (!spec.list_only || !text_entry) && spec.binding.matches(event) {
            Some(spec.edit)
        } else {
            None
        }
    }) {
        return Some(edit);
    }
    if text_entry && let Some(key) = LineKey::for_key(event) {
        return Some(Edit::Line(key));
    }
    if event.mods == Modifiers::NONE || event.mods == Modifiers::NONE.with_shift() {
        match event.code {
            KeyCode::Char(character) if text_entry => Some(Edit::Insert(character)),
            KeyCode::Space | KeyCode::Char(' ') => Some(if text_entry {
                Edit::Insert(' ')
            } else {
                Edit::Activate
            }),
            _ => None,
        }
    } else {
        None
    }
}
