//! Text editing owns printable keys; navigation, editing, and local hints share one key map.
use super::{Edit, Page, View};
use crate::client_view::{Binding, KeyCode, KeyEvent, KeyKind, Modifiers};

#[derive(Clone, Copy)]
struct BindingSpec {
    binding: Binding,
    edit: Edit,
    list_only: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalBinding {
    AdjustLargeBackward,
    AdjustLargeForward,
    Exit,
    Primary,
    PreviousTab,
    NextTab,
    PreviousRow,
    NextRow,
    AdjustBackward,
    AdjustForward,
    Home,
    End,
    HomeEmacs,
    EndEmacs,
    Clear,
    WordBack,
    Backspace,
    Delete,
}

impl LocalBinding {
    const fn spec(self) -> BindingSpec {
        let (code, mods, edit, list_only) = match self {
            Self::AdjustLargeBackward => (
                KeyCode::Left,
                Modifiers::NONE.with_shift(),
                Edit::AdjustLarge(false),
                true,
            ),
            Self::AdjustLargeForward => (
                KeyCode::Right,
                Modifiers::NONE.with_shift(),
                Edit::AdjustLarge(true),
                true,
            ),
            Self::Exit => (KeyCode::Esc, Modifiers::NONE, Edit::Cancel, false),
            Self::Primary => (KeyCode::Enter, Modifiers::NONE, Edit::Activate, false),
            Self::PreviousTab => (
                KeyCode::Tab,
                Modifiers::NONE.with_shift(),
                Edit::Previous,
                false,
            ),
            Self::NextTab => (KeyCode::Tab, Modifiers::NONE, Edit::Next, false),
            Self::PreviousRow => (KeyCode::Up, Modifiers::NONE, Edit::Previous, false),
            Self::NextRow => (KeyCode::Down, Modifiers::NONE, Edit::Next, false),
            Self::AdjustBackward => (KeyCode::Left, Modifiers::NONE, Edit::Adjust(false), false),
            Self::AdjustForward => (KeyCode::Right, Modifiers::NONE, Edit::Adjust(true), false),
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
            Self::Clear => (
                KeyCode::Char('u'),
                Modifiers::NONE.with_ctrl(),
                Edit::Clear,
                false,
            ),
            Self::WordBack => (
                KeyCode::Char('w'),
                Modifiers::NONE.with_ctrl(),
                Edit::WordBack,
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
    LocalBinding::AdjustLargeBackward,
    LocalBinding::AdjustLargeForward,
    LocalBinding::Exit,
    LocalBinding::Primary,
    LocalBinding::PreviousTab,
    LocalBinding::NextTab,
    LocalBinding::PreviousRow,
    LocalBinding::NextRow,
    LocalBinding::AdjustBackward,
    LocalBinding::AdjustForward,
    LocalBinding::Home,
    LocalBinding::End,
    LocalBinding::HomeEmacs,
    LocalBinding::EndEmacs,
    LocalBinding::Clear,
    LocalBinding::WordBack,
    LocalBinding::Backspace,
    LocalBinding::Delete,
];

/// Semantic label paired with one locally effective Settings binding group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettingsHintLabel {
    Close,
    Back,
    Cancel,
    Confirm,
    Accept,
    Clear,
    ActivateEdit,
    Select,
    Change,
}

/// One owner-ordered footer item backed by the same keys used for dispatch.
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
const SELECT: &[LocalBinding] = &[LocalBinding::NextTab];
const CHANGE: &[LocalBinding] = &[LocalBinding::AdjustBackward, LocalBinding::AdjustForward];
const CLEAR: &[LocalBinding] = &[LocalBinding::Clear];

const LOADING_HINTS: &[SettingsFooterBinding] =
    &[SettingsFooterBinding::new(EXIT, SettingsHintLabel::Close)];
const SAVING_HINTS: &[SettingsFooterBinding] =
    &[SettingsFooterBinding::new(EXIT, SettingsHintLabel::Close)];
const GENERAL_HINTS: &[SettingsFooterBinding] = &[
    SettingsFooterBinding::new(EXIT, SettingsHintLabel::Close),
    SettingsFooterBinding::new(PRIMARY, SettingsHintLabel::ActivateEdit),
    SettingsFooterBinding::new(SELECT, SettingsHintLabel::Select),
    SettingsFooterBinding::new(CHANGE, SettingsHintLabel::Change),
];
const PROFILE_HINTS: &[SettingsFooterBinding] = &[
    SettingsFooterBinding::new(EXIT, SettingsHintLabel::Back),
    SettingsFooterBinding::new(PRIMARY, SettingsHintLabel::ActivateEdit),
    SettingsFooterBinding::new(SELECT, SettingsHintLabel::Select),
    SettingsFooterBinding::new(CHANGE, SettingsHintLabel::Change),
];
const CONFIRMATION_HINTS: &[SettingsFooterBinding] = &[
    SettingsFooterBinding::new(EXIT, SettingsHintLabel::Cancel),
    SettingsFooterBinding::new(PRIMARY, SettingsHintLabel::Confirm),
];
const TEXT_HINTS: &[SettingsFooterBinding] = &[
    SettingsFooterBinding::new(EXIT, SettingsHintLabel::Cancel),
    SettingsFooterBinding::new(PRIMARY, SettingsHintLabel::Accept),
    SettingsFooterBinding::new(CLEAR, SettingsHintLabel::Clear),
];

/// Projects the effective local footer groups for the current Settings mode.
pub(crate) fn footer_bindings(view: View<'_>) -> &'static [SettingsFooterBinding] {
    match view {
        View::Loading { .. } => LOADING_HINTS,
        View::Saving { .. } => SAVING_HINTS,
        View::Draft(draft) if draft.confirmation.is_some() => CONFIRMATION_HINTS,
        View::Draft(draft) if draft.text.is_some() => TEXT_HINTS,
        View::Draft(draft) if draft.page == Page::Profiles => PROFILE_HINTS,
        View::Draft(_) => GENERAL_HINTS,
    }
}

pub(crate) fn key_edit(event: KeyEvent, text_entry: bool) -> Option<Edit> {
    if event.kind == KeyKind::Release {
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
