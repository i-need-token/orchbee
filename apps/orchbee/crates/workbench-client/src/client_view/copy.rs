//! Copy-mode bindings are the single source for dispatch and localized discovery.
use super::{Binding, KeyCode, KeyEvent, KeyKind, LabelStyle, Modifiers, PrefixDesc};
use crate::terminal_copy::{CopyMotion, CopySearchDirection};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopyModeKey {
    Left,
    Right,
    Up,
    Down,
    Motion(CopyMotion),
    Page { older: bool },
    Oldest,
    Newest,
    Search(CopySearchDirection),
    RepeatSearch { reverse: bool },
    ToggleSelection,
    Copy,
    Exit,
    Help,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopySearchCommand {
    Back,
    Submit,
    Reverse,
}

pub(crate) use crate::localization::CopyHintLabel;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalBinding {
    Left,
    LeftVim,
    Down,
    DownVim,
    Up,
    UpVim,
    Right,
    RightVim,
    NextWord,
    PreviousWord,
    NextWordEnd,
    NextBigWord,
    PreviousBigWord,
    NextBigWordEnd,
    PreviousParagraph,
    NextParagraph,
    PageUp,
    PageDown,
    Oldest,
    Newest,
    SearchForward,
    SearchBackward,
    RepeatForward,
    RepeatReverse,
    ToggleSelection,
    ToggleSelectionSpace,
    Copy,
    CopyEnter,
    Exit,
    ExitEscape,
    Help,
    SearchBack,
    SearchReverse,
}

#[derive(Clone, Copy)]
struct BindingSpec {
    binding: Binding,
    normal: Option<CopyModeKey>,
    search: Option<CopySearchCommand>,
}

impl LocalBinding {
    const fn spec(self) -> BindingSpec {
        use CopyModeKey as K;
        use CopyMotion as M;
        use CopySearchCommand as C;
        use CopySearchDirection as S;

        let (code, modifiers, normal, search) = match self {
            Self::Left => (KeyCode::Left, Modifiers::NONE, Some(K::Left), None),
            Self::LeftVim => (KeyCode::Char('h'), Modifiers::NONE, Some(K::Left), None),
            Self::Down => (KeyCode::Down, Modifiers::NONE, Some(K::Down), None),
            Self::DownVim => (KeyCode::Char('j'), Modifiers::NONE, Some(K::Down), None),
            Self::Up => (KeyCode::Up, Modifiers::NONE, Some(K::Up), None),
            Self::UpVim => (KeyCode::Char('k'), Modifiers::NONE, Some(K::Up), None),
            Self::Right => (KeyCode::Right, Modifiers::NONE, Some(K::Right), None),
            Self::RightVim => (KeyCode::Char('l'), Modifiers::NONE, Some(K::Right), None),
            Self::NextWord => (
                KeyCode::Char('w'),
                Modifiers::NONE,
                Some(K::Motion(M::NextWordStart)),
                None,
            ),
            Self::PreviousWord => (
                KeyCode::Char('b'),
                Modifiers::NONE,
                Some(K::Motion(M::PreviousWordStart)),
                None,
            ),
            Self::NextWordEnd => (
                KeyCode::Char('e'),
                Modifiers::NONE,
                Some(K::Motion(M::NextWordEnd)),
                None,
            ),
            Self::NextBigWord => (
                KeyCode::Char('W'),
                Modifiers::NONE,
                Some(K::Motion(M::NextBigWordStart)),
                None,
            ),
            Self::PreviousBigWord => (
                KeyCode::Char('B'),
                Modifiers::NONE,
                Some(K::Motion(M::PreviousBigWordStart)),
                None,
            ),
            Self::NextBigWordEnd => (
                KeyCode::Char('E'),
                Modifiers::NONE,
                Some(K::Motion(M::NextBigWordEnd)),
                None,
            ),
            Self::PreviousParagraph => (
                KeyCode::Char('{'),
                Modifiers::NONE,
                Some(K::Motion(M::PreviousParagraph)),
                None,
            ),
            Self::NextParagraph => (
                KeyCode::Char('}'),
                Modifiers::NONE,
                Some(K::Motion(M::NextParagraph)),
                None,
            ),
            Self::PageUp => (
                KeyCode::PageUp,
                Modifiers::NONE,
                Some(K::Page { older: true }),
                None,
            ),
            Self::PageDown => (
                KeyCode::PageDown,
                Modifiers::NONE,
                Some(K::Page { older: false }),
                None,
            ),
            Self::Oldest => (KeyCode::Char('g'), Modifiers::NONE, Some(K::Oldest), None),
            Self::Newest => (KeyCode::Char('G'), Modifiers::NONE, Some(K::Newest), None),
            Self::SearchForward => (
                KeyCode::Char('/'),
                Modifiers::NONE,
                Some(K::Search(S::Forward)),
                None,
            ),
            Self::SearchBackward => (
                KeyCode::Char('?'),
                Modifiers::NONE,
                Some(K::Search(S::Backward)),
                None,
            ),
            Self::RepeatForward => (
                KeyCode::Char('n'),
                Modifiers::NONE,
                Some(K::RepeatSearch { reverse: false }),
                None,
            ),
            Self::RepeatReverse => (
                KeyCode::Char('N'),
                Modifiers::NONE,
                Some(K::RepeatSearch { reverse: true }),
                None,
            ),
            Self::ToggleSelection => (
                KeyCode::Char('v'),
                Modifiers::NONE,
                Some(K::ToggleSelection),
                None,
            ),
            Self::ToggleSelectionSpace => (
                KeyCode::Space,
                Modifiers::NONE,
                Some(K::ToggleSelection),
                None,
            ),
            Self::Copy => (KeyCode::Char('y'), Modifiers::NONE, Some(K::Copy), None),
            Self::CopyEnter => (
                KeyCode::Enter,
                Modifiers::NONE,
                Some(K::Copy),
                Some(C::Submit),
            ),
            Self::Exit => (KeyCode::Char('q'), Modifiers::NONE, Some(K::Exit), None),
            Self::ExitEscape => (KeyCode::Esc, Modifiers::NONE, Some(K::Exit), None),
            Self::Help => (KeyCode::F(1), Modifiers::NONE, Some(K::Help), None),
            Self::SearchBack => (KeyCode::Char('g'), Modifiers::CTRL, None, Some(C::Back)),
            Self::SearchReverse => (KeyCode::Char('r'), Modifiers::CTRL, None, Some(C::Reverse)),
        };
        BindingSpec {
            binding: Binding::new(code, modifiers),
            normal,
            search,
        }
    }
}

struct Group {
    bindings: &'static [LocalBinding],
    desc: PrefixDesc,
}

const GROUPS: &[Group] = &[
    Group {
        bindings: &[
            LocalBinding::Left,
            LocalBinding::LeftVim,
            LocalBinding::Down,
            LocalBinding::DownVim,
            LocalBinding::Up,
            LocalBinding::UpVim,
            LocalBinding::Right,
            LocalBinding::RightVim,
        ],
        desc: PrefixDesc::CopyMove,
    },
    Group {
        bindings: &[
            LocalBinding::NextWord,
            LocalBinding::PreviousWord,
            LocalBinding::NextWordEnd,
            LocalBinding::NextBigWord,
            LocalBinding::PreviousBigWord,
            LocalBinding::NextBigWordEnd,
        ],
        desc: PrefixDesc::CopyWord,
    },
    Group {
        bindings: &[LocalBinding::PreviousParagraph, LocalBinding::NextParagraph],
        desc: PrefixDesc::CopyParagraph,
    },
    Group {
        bindings: &[LocalBinding::PageUp, LocalBinding::PageDown],
        desc: PrefixDesc::CopyPage,
    },
    Group {
        bindings: &[LocalBinding::Oldest, LocalBinding::Newest],
        desc: PrefixDesc::CopyEnds,
    },
    Group {
        bindings: &[LocalBinding::SearchForward, LocalBinding::SearchBackward],
        desc: PrefixDesc::CopySearch,
    },
    Group {
        bindings: &[LocalBinding::RepeatForward, LocalBinding::RepeatReverse],
        desc: PrefixDesc::CopyRepeatSearch,
    },
    Group {
        bindings: &[
            LocalBinding::ToggleSelection,
            LocalBinding::ToggleSelectionSpace,
        ],
        desc: PrefixDesc::CopySelection,
    },
    Group {
        bindings: &[LocalBinding::Copy, LocalBinding::CopyEnter],
        desc: PrefixDesc::CopySubmit,
    },
    Group {
        bindings: &[LocalBinding::Exit, LocalBinding::ExitEscape],
        desc: PrefixDesc::CopyExit,
    },
    Group {
        bindings: &[LocalBinding::Help],
        desc: PrefixDesc::ShowHelp,
    },
];

const SEARCH_DISPATCH: &[LocalBinding] = &[
    LocalBinding::SearchBack,
    LocalBinding::CopyEnter,
    LocalBinding::SearchReverse,
];

/// One owner-ordered footer item backed by the same keys used for dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CopyFooterBinding {
    keys: &'static [LocalBinding],
    label: CopyHintLabel,
}

impl CopyFooterBinding {
    const fn new(keys: &'static [LocalBinding], label: CopyHintLabel) -> Self {
        Self { keys, label }
    }

    pub(crate) const fn label(self) -> CopyHintLabel {
        self.label
    }

    pub(crate) fn bindings(self) -> impl ExactSizeIterator<Item = Binding> {
        self.keys.iter().map(|key| key.spec().binding)
    }
}

const EXIT_KEY: &[LocalBinding] = &[LocalBinding::ExitEscape];
const COPY_KEY: &[LocalBinding] = &[LocalBinding::Copy];
const SELECT_KEY: &[LocalBinding] = &[LocalBinding::ToggleSelection];
const SEARCH_KEY: &[LocalBinding] = &[LocalBinding::SearchForward];
const REPEAT_KEYS: &[LocalBinding] = &[LocalBinding::RepeatForward, LocalBinding::RepeatReverse];
const HELP_KEY: &[LocalBinding] = &[LocalBinding::Help];
const MOVE_KEYS: &[LocalBinding] = &[
    LocalBinding::Up,
    LocalBinding::Down,
    LocalBinding::Left,
    LocalBinding::Right,
];
const FIND_KEY: &[LocalBinding] = &[LocalBinding::CopyEnter];
const BACK_KEY: &[LocalBinding] = &[LocalBinding::SearchBack];
const REVERSE_KEY: &[LocalBinding] = &[LocalBinding::SearchReverse];

const NORMAL_HINTS: &[CopyFooterBinding] = &[
    CopyFooterBinding::new(EXIT_KEY, CopyHintLabel::Exit),
    CopyFooterBinding::new(COPY_KEY, CopyHintLabel::Copy),
    CopyFooterBinding::new(SELECT_KEY, CopyHintLabel::Select),
    CopyFooterBinding::new(SEARCH_KEY, CopyHintLabel::Search),
    CopyFooterBinding::new(REPEAT_KEYS, CopyHintLabel::Repeat),
    CopyFooterBinding::new(HELP_KEY, CopyHintLabel::Help),
    CopyFooterBinding::new(MOVE_KEYS, CopyHintLabel::Move),
];
const SEARCH_HINTS: &[CopyFooterBinding] = &[
    CopyFooterBinding::new(EXIT_KEY, CopyHintLabel::Exit),
    CopyFooterBinding::new(FIND_KEY, CopyHintLabel::Find),
    CopyFooterBinding::new(BACK_KEY, CopyHintLabel::Back),
    CopyFooterBinding::new(REVERSE_KEY, CopyHintLabel::Reverse),
];

pub(crate) fn footer_bindings(searching: bool) -> &'static [CopyFooterBinding] {
    if searching {
        SEARCH_HINTS
    } else {
        NORMAL_HINTS
    }
}

pub(crate) fn decide(event: KeyEvent) -> Option<CopyModeKey> {
    if event.kind == KeyKind::Release {
        return None;
    }
    GROUPS
        .iter()
        .flat_map(|group| group.bindings)
        .find_map(|key| {
            let spec = key.spec();
            if spec.binding.matches(event) {
                spec.normal
            } else {
                None
            }
        })
}

pub(crate) fn search_command(event: KeyEvent) -> Option<CopySearchCommand> {
    if event.kind == KeyKind::Release {
        return None;
    }
    SEARCH_DISPATCH.iter().find_map(|key| {
        let spec = key.spec();
        if spec.binding.matches(event) {
            spec.search
        } else {
            None
        }
    })
}

pub(super) fn help_rows() -> Vec<(String, PrefixDesc)> {
    let mut rows = Vec::with_capacity(GROUPS.len() + 1);
    for group in GROUPS {
        rows.push((
            group
                .bindings
                .iter()
                .map(|key| super::format_binding(&key.spec().binding, LabelStyle::for_host()))
                .collect::<Vec<_>>()
                .join(" / "),
            group.desc,
        ));
        if group.desc == PrefixDesc::CopyRepeatSearch {
            rows.push((
                super::format_binding(
                    &LocalBinding::SearchReverse.spec().binding,
                    LabelStyle::for_host(),
                ),
                PrefixDesc::CopyReverseSearch,
            ));
        }
    }
    rows
}

#[cfg(test)]
mod tests;
