//! Copy-mode bindings are the single source for dispatch and localized help.
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

struct Group {
    bindings: &'static [(Binding, CopyModeKey)],
    desc: PrefixDesc,
}
const fn key(code: KeyCode) -> Binding {
    Binding::new(code, Modifiers::NONE)
}
use CopyModeKey as K;
use CopyMotion as M;
use CopySearchDirection as S;
const GROUPS: &[Group] = &[
    Group {
        bindings: &[
            (key(KeyCode::Left), K::Left),
            (key(KeyCode::Char('h')), K::Left),
            (key(KeyCode::Down), K::Down),
            (key(KeyCode::Char('j')), K::Down),
            (key(KeyCode::Up), K::Up),
            (key(KeyCode::Char('k')), K::Up),
            (key(KeyCode::Right), K::Right),
            (key(KeyCode::Char('l')), K::Right),
        ],
        desc: PrefixDesc::CopyMove,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('w')), K::Motion(M::NextWordStart)),
            (key(KeyCode::Char('b')), K::Motion(M::PreviousWordStart)),
            (key(KeyCode::Char('e')), K::Motion(M::NextWordEnd)),
            (key(KeyCode::Char('W')), K::Motion(M::NextBigWordStart)),
            (key(KeyCode::Char('B')), K::Motion(M::PreviousBigWordStart)),
            (key(KeyCode::Char('E')), K::Motion(M::NextBigWordEnd)),
        ],
        desc: PrefixDesc::CopyWord,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('{')), K::Motion(M::PreviousParagraph)),
            (key(KeyCode::Char('}')), K::Motion(M::NextParagraph)),
        ],
        desc: PrefixDesc::CopyParagraph,
    },
    Group {
        bindings: &[
            (key(KeyCode::PageUp), K::Page { older: true }),
            (key(KeyCode::PageDown), K::Page { older: false }),
        ],
        desc: PrefixDesc::CopyPage,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('g')), K::Oldest),
            (key(KeyCode::Char('G')), K::Newest),
        ],
        desc: PrefixDesc::CopyEnds,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('/')), K::Search(S::Forward)),
            (key(KeyCode::Char('?')), K::Search(S::Backward)),
        ],
        desc: PrefixDesc::CopySearch,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('n')), K::RepeatSearch { reverse: false }),
            (key(KeyCode::Char('N')), K::RepeatSearch { reverse: true }),
        ],
        desc: PrefixDesc::CopyRepeatSearch,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('v')), K::ToggleSelection),
            (key(KeyCode::Space), K::ToggleSelection),
        ],
        desc: PrefixDesc::CopySelection,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('y')), K::Copy),
            (key(KeyCode::Enter), K::Copy),
        ],
        desc: PrefixDesc::CopySubmit,
    },
    Group {
        bindings: &[
            (key(KeyCode::Char('q')), K::Exit),
            (key(KeyCode::Esc), K::Exit),
        ],
        desc: PrefixDesc::CopyExit,
    },
    Group {
        bindings: &[(key(KeyCode::F(1)), K::Help)],
        desc: PrefixDesc::ShowHelp,
    },
];

pub(crate) fn decide(event: KeyEvent) -> Option<CopyModeKey> {
    if event.kind == KeyKind::Release {
        return None;
    }
    GROUPS
        .iter()
        .flat_map(|group| group.bindings)
        .find_map(|(binding, action)| binding.matches(event).then_some(*action))
}

pub(super) fn help_rows() -> Vec<(String, PrefixDesc)> {
    GROUPS
        .iter()
        .map(|group| {
            (
                group
                    .bindings
                    .iter()
                    .map(|(binding, _)| super::format_binding(binding, LabelStyle::for_host()))
                    .collect::<Vec<_>>()
                    .join(" / "),
                group.desc,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
