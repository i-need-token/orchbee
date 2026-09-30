//! Default prefix bindings. The owned resolved registry applies Client overrides
//! and supplies both dispatch and discovery. Localized descriptions stay in
//! `localization`; this table only names them.

use super::{
    ClientAction, KeyCode, Modifiers, ShellCommand,
    keys::{Binding, KeyEvent},
};
mod configured;
pub(crate) use configured::Bindings;

/// The chord the ordinary layer intercepts unconditionally: `Ctrl+B`.
pub(crate) const PREFIX_CHORD: Binding = Binding::new(KeyCode::Char('b'), Modifiers::CTRL);

/// What one member key resolves to when matched under an armed prefix.
/// `Client` actions map one-to-one to `ClientAction`; the toggle intents own
/// their local view effects inside `ClientView::decide_prefix`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PrefixAction {
    Client(ClientAction),
    SelectTabDigit,
    #[cfg(windows)]
    SelectAgentDigit,
    SidebarToggle,
    NavigatorToggle,
    BoardToggle,
    ShowHelp,
    HelpPagePrevious,
    HelpPageNext,
}

/// One matchable key set inside a group. `CharRange` exists so the ordered-Tab
/// selection is one member displayed as `1..9`, never nine rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeySet {
    One(Binding),
    CharRange(char, char),
}

impl KeySet {
    pub(crate) fn matches(&self, event: KeyEvent) -> bool {
        match self {
            Self::One(binding) => binding.matches(event),
            Self::CharRange(first, last) => {
                let (KeyCode::Char(c), mods) = event.normalized() else {
                    return false;
                };
                mods == Modifiers::NONE && c >= *first && c <= *last
            }
        }
    }

    /// The displayed label for this member (`1..9` for ranges).
    pub(crate) fn label(&self, style: super::LabelStyle) -> String {
        match self {
            Self::One(binding) => super::format_binding(binding, style),
            Self::CharRange(first, last) => format!("{first}..{last}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrefixMember {
    pub(crate) keys: KeySet,
    pub(crate) action: PrefixAction,
    pub(crate) shell: ShellCommand,
}

/// The localization key for a group's description row. Descriptions live in
/// `localization.rs` behind this enum; grouped members share one row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrefixDesc {
    #[cfg(windows)]
    Machines,
    #[cfg(windows)]
    AgentCycle,
    #[cfg(windows)]
    AgentIndex,
    ReloadConfiguration,
    WorkspaceCreate,
    WorkspaceDirectory,
    WorkspaceRename,
    ProjectRename,
    RenameTab,
    CloseTab,
    CreateTab,
    MoveTab,
    SelectTabWrap,
    SelectContext,
    SelectTabDigit,
    PanelFocus,
    PresentationSelect,
    PanelZoom,
    SplitPanel,
    ClosePanel,
    MovePanel,
    SidebarToggle,
    NavigatorToggle,
    BoardToggle,
    ShowHelp,
    HelpPage,
    PointerCapture,
    Exit,
    LiteralPrefix,
    DismissPrefix,
    OtherKeys,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PrefixGroup {
    pub(crate) members: &'static [PrefixMember],
    pub(crate) desc: PrefixDesc,
}

const fn member(keys: KeySet, action: ClientAction) -> PrefixMember {
    PrefixMember {
        keys,
        action: PrefixAction::Client(action),
        shell: ShellCommand::None,
    }
}

const fn key(code: KeyCode) -> KeySet {
    KeySet::One(Binding::new(code, Modifiers::NONE))
}

const fn shift_key(c: char) -> KeySet {
    KeySet::One(Binding::new(KeyCode::Char(c), Modifiers::NONE.with_shift()))
}

use super::PanelDirection;

/// Every prefix-layer binding group, in help order. The first member label of
/// each group joins with `" / "` into the help chord column.
pub(crate) static PREFIX_GROUPS: &[PrefixGroup] = &[
    #[cfg(windows)]
    PrefixGroup {
        members: &[member(key(KeyCode::Char('M')), ClientAction::OpenMachines)],
        desc: PrefixDesc::Machines,
    },
    #[cfg(windows)]
    PrefixGroup {
        members: &[
            member(key(KeyCode::Char('{')), ClientAction::PreviousAgent),
            member(key(KeyCode::Char('}')), ClientAction::NextAgent),
        ],
        desc: PrefixDesc::AgentCycle,
    },
    PrefixGroup {
        members: &[member(
            key(KeyCode::Char('a')),
            ClientAction::RequestWorkspaceCreate,
        )],
        desc: PrefixDesc::WorkspaceCreate,
    },
    PrefixGroup {
        members: &[member(
            key(KeyCode::Char('e')),
            ClientAction::RequestWorkspaceDirectory,
        )],
        desc: PrefixDesc::WorkspaceDirectory,
    },
    PrefixGroup {
        members: &[member(
            key(KeyCode::Char('r')),
            ClientAction::RequestWorkspaceRename,
        )],
        desc: PrefixDesc::WorkspaceRename,
    },
    PrefixGroup {
        members: &[member(shift_key('r'), ClientAction::RequestProjectRename)],
        desc: PrefixDesc::ProjectRename,
    },
    PrefixGroup {
        members: &[member(
            key(KeyCode::Char(',')),
            ClientAction::RequestRenameTab,
        )],
        desc: PrefixDesc::RenameTab,
    },
    PrefixGroup {
        members: &[member(
            key(KeyCode::Char('x')),
            ClientAction::RequestCloseTab,
        )],
        desc: PrefixDesc::CloseTab,
    },
    PrefixGroup {
        members: &[member(key(KeyCode::Char('c')), ClientAction::CreateTab)],
        desc: PrefixDesc::CreateTab,
    },
    PrefixGroup {
        members: &[
            member(key(KeyCode::Char('<')), ClientAction::MovePreviousTab),
            member(key(KeyCode::Char('>')), ClientAction::MoveNextTab),
        ],
        desc: PrefixDesc::MoveTab,
    },
    PrefixGroup {
        members: &[
            member(key(KeyCode::Char('n')), ClientAction::SelectNextTab),
            member(key(KeyCode::Char('p')), ClientAction::SelectPreviousTab),
        ],
        desc: PrefixDesc::SelectTabWrap,
    },
    PrefixGroup {
        members: &[
            member(key(KeyCode::Char('[')), ClientAction::SelectPreviousContext),
            member(key(KeyCode::Char(']')), ClientAction::SelectNextContext),
        ],
        desc: PrefixDesc::SelectContext,
    },
    PrefixGroup {
        members: &[PrefixMember {
            keys: KeySet::CharRange('1', '9'),
            action: PrefixAction::SelectTabDigit,
            shell: ShellCommand::None,
        }],
        desc: PrefixDesc::SelectTabDigit,
    },
    PrefixGroup {
        members: &[
            member(
                key(KeyCode::Char('h')),
                ClientAction::MovePanelFocus(PanelDirection::Left),
            ),
            member(
                key(KeyCode::Char('j')),
                ClientAction::MovePanelFocus(PanelDirection::Down),
            ),
            member(
                key(KeyCode::Char('k')),
                ClientAction::MovePanelFocus(PanelDirection::Up),
            ),
            member(
                key(KeyCode::Char('l')),
                ClientAction::MovePanelFocus(PanelDirection::Right),
            ),
        ],
        desc: PrefixDesc::PanelFocus,
    },
    PrefixGroup {
        members: &[
            member(key(KeyCode::Char('(')), ClientAction::PreviousPresentation),
            member(key(KeyCode::Char(')')), ClientAction::NextPresentation),
        ],
        desc: PrefixDesc::PresentationSelect,
    },
    PrefixGroup {
        members: &[member(key(KeyCode::Char('z')), ClientAction::ToggleZoom)],
        desc: PrefixDesc::PanelZoom,
    },
    PrefixGroup {
        members: &[
            member(
                key(KeyCode::Char('-')),
                ClientAction::SplitPanel(PanelDirection::Down),
            ),
            member(
                key(KeyCode::Char('_')),
                ClientAction::SplitPanel(PanelDirection::Up),
            ),
            member(
                key(KeyCode::Char('\\')),
                ClientAction::SplitPanel(PanelDirection::Left),
            ),
            member(
                key(KeyCode::Char('|')),
                ClientAction::SplitPanel(PanelDirection::Right),
            ),
        ],
        desc: PrefixDesc::SplitPanel,
    },
    PrefixGroup {
        members: &[member(shift_key('x'), ClientAction::ClosePanel)],
        desc: PrefixDesc::ClosePanel,
    },
    PrefixGroup {
        members: &[
            member(
                shift_key('h'),
                ClientAction::MovePanel(PanelDirection::Left),
            ),
            member(
                shift_key('j'),
                ClientAction::MovePanel(PanelDirection::Down),
            ),
            member(shift_key('k'), ClientAction::MovePanel(PanelDirection::Up)),
            member(
                shift_key('l'),
                ClientAction::MovePanel(PanelDirection::Right),
            ),
        ],
        desc: PrefixDesc::MovePanel,
    },
    PrefixGroup {
        members: &[PrefixMember {
            keys: key(KeyCode::Char('b')),
            action: PrefixAction::SidebarToggle,
            shell: ShellCommand::None,
        }],
        desc: PrefixDesc::SidebarToggle,
    },
    PrefixGroup {
        members: &[PrefixMember {
            keys: key(KeyCode::Char('w')),
            action: PrefixAction::NavigatorToggle,
            shell: ShellCommand::None,
        }],
        desc: PrefixDesc::NavigatorToggle,
    },
    PrefixGroup {
        members: &[PrefixMember {
            keys: key(KeyCode::Char('g')),
            action: PrefixAction::BoardToggle,
            shell: ShellCommand::None,
        }],
        desc: PrefixDesc::BoardToggle,
    },
    PrefixGroup {
        members: &[PrefixMember {
            keys: key(KeyCode::Char('?')),
            action: PrefixAction::ShowHelp,
            shell: ShellCommand::None,
        }],
        desc: PrefixDesc::ShowHelp,
    },
    PrefixGroup {
        members: &[member(
            key(KeyCode::Char('m')),
            ClientAction::TogglePointerCapture,
        )],
        desc: PrefixDesc::PointerCapture,
    },
    // Pages an open help overlay; with help closed the chord is inert
    // (same unknown-prefix outcome as any unbound member key).
    PrefixGroup {
        members: &[
            PrefixMember {
                keys: key(KeyCode::PageUp),
                action: PrefixAction::HelpPagePrevious,
                shell: ShellCommand::None,
            },
            PrefixMember {
                keys: key(KeyCode::PageDown),
                action: PrefixAction::HelpPageNext,
                shell: ShellCommand::None,
            },
        ],
        desc: PrefixDesc::HelpPage,
    },
    PrefixGroup {
        members: &[PrefixMember {
            keys: KeySet::One(PREFIX_CHORD),
            action: PrefixAction::Client(ClientAction::ForwardLiteralPrefix),
            shell: ShellCommand::ForwardTerminal,
        }],
        desc: PrefixDesc::LiteralPrefix,
    },
    PrefixGroup {
        members: &[member(key(KeyCode::Esc), ClientAction::DismissPrefix)],
        desc: PrefixDesc::DismissPrefix,
    },
    #[cfg(windows)]
    PrefixGroup {
        members: &[member(
            key(KeyCode::F(5)),
            ClientAction::ReloadConfiguration,
        )],
        desc: PrefixDesc::ReloadConfiguration,
    },
    // The exit binding stays last, per the keyboard-input standard.
    PrefixGroup {
        members: &[
            PrefixMember {
                keys: key(KeyCode::Char('q')),
                action: PrefixAction::Client(ClientAction::Exit),
                shell: ShellCommand::Exit,
            },
            PrefixMember {
                keys: key(KeyCode::Char('d')),
                action: PrefixAction::Client(ClientAction::Exit),
                shell: ShellCommand::Exit,
            },
        ],
        desc: PrefixDesc::Exit,
    },
];

/// Resolves a decoded event against the armed-prefix vocabulary.
#[cfg(test)]
pub(crate) fn match_prefix(event: KeyEvent) -> Option<&'static PrefixMember> {
    PREFIX_GROUPS
        .iter()
        .flat_map(|group| group.members)
        .find(|member| member.keys.matches(event))
}

#[cfg(test)]
mod tests;
