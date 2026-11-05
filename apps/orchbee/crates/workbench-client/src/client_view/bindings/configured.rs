//! An owned Client registry shared by input and discovery, never a process-global cache.
use super::{
    Binding, KeyCode, KeyEvent, KeySet, PREFIX_CHORD, PREFIX_GROUPS, PrefixDesc, PrefixMember,
};
use super::{ClientAction, Modifiers, PanelDirection, PrefixAction, ShellCommand};
use crate::action_projection::{ActionMetadata, binding_metadata};
use orchbee_local_runtime::DefaultActionBindings;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveryEntry<'a> {
    pub(crate) action: PrefixAction,
    pub(crate) description: PrefixDesc,
    pub(crate) metadata: ActionMetadata<'a>,
    pub(crate) key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    enabled: bool,
    pub(crate) prefix: bool,
    pub(crate) member: PrefixMember,
    pub(crate) desc: PrefixDesc,
}

/// An installed configured command as discovery names it. The name is the Server's text for it,
/// never derived here from the command line, so every Client shows the same words.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ConfiguredName {
    id: String,
    /// The catalog's description, else its display text; `None` until the Server's catalog
    /// answers for this exact identity.
    name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bindings {
    commands: Vec<ConfiguredName>,
    pub prefix: Binding,
    pub(crate) entries: Vec<Entry>,
}

impl Default for Bindings {
    fn default() -> Self {
        let entries = PREFIX_GROUPS
            .iter()
            .flat_map(|group| {
                group.members.iter().map(|member| Entry {
                    enabled: true,
                    prefix: true,
                    member: member.clone(),
                    desc: group.desc,
                })
            })
            .collect::<Vec<_>>();
        let entries = {
            let mut entries = entries;
            for name in [
                "resize_pane_left",
                "resize_pane_right",
                "resize_pane_up",
                "resize_pane_down",
            ] {
                let (action, desc, shell) = action(name);
                entries.push(Entry {
                    enabled: false,
                    prefix: false,
                    member: PrefixMember {
                        keys: KeySet::One(PREFIX_CHORD),
                        action,
                        shell,
                    },
                    desc,
                });
            }
            entries
        };
        Self {
            commands: Vec::new(),
            prefix: PREFIX_CHORD,
            entries,
        }
    }
}

impl Bindings {
    /// Returns only effective, bound actions for keyboard discovery. The
    /// action metadata is the semantic identity shared by menus and help;
    /// chord text comes from the exact same resolved entries used by dispatch.
    pub(crate) fn discovery_entries(
        &self,
        glyphs: super::super::KeyLabelGlyphs,
    ) -> Vec<DiscoveryEntry<'_>> {
        self.entries
            .iter()
            .filter(|entry| entry.enabled)
            .filter_map(|entry| {
                let metadata = binding_metadata(&entry.member.action)?;
                Some(DiscoveryEntry {
                    action: entry.member.action.clone(),
                    description: entry.desc,
                    metadata,
                    key: self.entry_label(entry, glyphs),
                })
            })
            .collect()
    }

    pub fn default_windows_key_action_bindings() -> Vec<DefaultActionBindings> {
        let defaults = Self::default();
        orchbee_local_runtime::KeySettings::registered_actions()
            .iter()
            .map(|&name| {
                let (action, _, _) = action(name);
                let bindings = defaults
                    .entries
                    .iter()
                    .filter(|entry| entry.enabled && entry.member.action == action)
                    .map(configuration_trigger)
                    .collect();
                DefaultActionBindings {
                    action: name,
                    bindings,
                }
            })
            .collect()
    }

    /// The Server's name for a configured command's discovery row, shown as sent: the Server
    /// alone makes it safe to show. `None` leaves the row its localized generic title.
    pub(crate) fn configured_name(&self, desc: PrefixDesc) -> Option<&str> {
        let PrefixDesc::ConfiguredCommand(index) = desc else {
            return None;
        };
        self.commands.get(usize::from(index))?.name.as_deref()
    }

    /// Names installed commands from a Server catalog. An entry names only the command with its
    /// identity, which binds the whole installed profile, so a catalog read against another
    /// profile names nothing.
    pub(crate) fn name_configured_commands(
        &mut self,
        catalog: &[orchbee_workbench_protocol::ConfiguredCommandDescriptor],
    ) {
        for command in &mut self.commands {
            command.name = catalog
                .iter()
                .find(|entry| entry.id == command.id)
                .and_then(|entry| entry.description.clone().or_else(|| entry.display.clone()));
        }
    }
    pub(crate) fn find(&self, event: KeyEvent, prefix: bool) -> Option<&PrefixMember> {
        let exact = self
            .entries
            .iter()
            .find(|entry| {
                entry.enabled && entry.prefix == prefix && entry.member.keys.matches(event)
            })
            .map(|entry| &entry.member);
        exact.or_else(|| {
            self.entries
                .iter()
                .find(|entry| {
                    entry.enabled
                        && entry.prefix == prefix
                        && shifted_agent_digit(&entry.member, event)
                })
                .map(|entry| &entry.member)
        })
    }

    pub(crate) fn labels(&self, desc: PrefixDesc) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.enabled && entry.desc == desc)
            .map(|entry| self.entry_label(entry, super::super::KeyLabelGlyphs::Unicode))
            .collect()
    }

    pub(crate) fn action_labels(&self, action: PrefixAction) -> Vec<String> {
        self.action_labels_with_glyphs(action, super::super::KeyLabelGlyphs::Unicode)
    }

    pub(crate) fn action_labels_with_glyphs(
        &self,
        action: PrefixAction,
        glyphs: super::super::KeyLabelGlyphs,
    ) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.enabled && entry.member.action == action)
            .map(|entry| self.entry_label(entry, glyphs))
            .collect()
    }

    fn entry_label(&self, entry: &Entry, glyphs: super::super::KeyLabelGlyphs) -> String {
        let style = super::super::LabelStyle::for_host();
        let key = match entry.member.keys {
            KeySet::One(binding) => {
                super::super::format_binding_with_glyphs(&binding, style, glyphs)
            }
            KeySet::CharRange(first, last) => format!("{first}..{last}"),
        };
        if entry.prefix {
            format!(
                "{} {key}",
                super::super::format_binding_with_glyphs(&self.prefix, style, glyphs)
            )
        } else {
            key
        }
    }

    pub fn configured(settings: &orchbee_local_runtime::KeySettings) -> Self {
        let prefix = chord(&settings.prefix);
        let mut user = Vec::new();
        let mut replaced = Vec::new();
        for setting in &settings.overrides {
            let (action, desc, shell) = action(setting.action);
            replaced.push(action.clone());
            if setting.triggers.is_empty() {
                user.push(Entry {
                    enabled: false,
                    prefix: true,
                    member: PrefixMember {
                        keys: KeySet::One(PREFIX_CHORD),
                        action: action.clone(),
                        shell,
                    },
                    desc,
                });
            }
            user.extend(setting.triggers.iter().map(|trigger| Entry {
                enabled: true,
                prefix: trigger.prefix,
                member: PrefixMember {
                    keys: KeySet::One(chord(&trigger.chord)),
                    action: action.clone(),
                    shell,
                },
                desc,
            }));
        }
        let commands = settings.command_catalog();
        for (index, entry) in commands.iter().enumerate() {
            let action = PrefixAction::Client(ClientAction::ConfiguredCommand(entry.id.clone()));
            let desc = PrefixDesc::ConfiguredCommand(index as u8);
            if entry.definition.triggers.is_empty() {
                user.push(Entry {
                    enabled: false,
                    prefix: true,
                    member: PrefixMember {
                        keys: KeySet::One(PREFIX_CHORD),
                        action: action.clone(),
                        shell: ShellCommand::None,
                    },
                    desc,
                });
            }
            user.extend(entry.definition.triggers.iter().map(|trigger| Entry {
                enabled: true,
                prefix: trigger.prefix,
                member: PrefixMember {
                    keys: KeySet::One(chord(&trigger.chord)),
                    action: action.clone(),
                    shell: ShellCommand::None,
                },
                desc,
            }));
        }
        let mut defaults = Vec::new();
        for mut entry in Self::default().entries {
            if replaced.contains(&entry.member.action) {
                continue;
            }
            if entry.desc == PrefixDesc::LiteralPrefix {
                entry.member.keys = KeySet::One(prefix);
                defaults.push(entry);
                continue;
            }
            // Expand only for conflict filtering; an unchanged range stays a single help row.
            let keys: Vec<_> = match entry.member.keys {
                KeySet::One(binding) => vec![binding],
                KeySet::CharRange(first, last) => (first..=last)
                    .map(|key| Binding::new(KeyCode::Char(key), Modifiers::NONE))
                    .collect(),
            };
            let retained: Vec<_> = keys
                .iter()
                .copied()
                .filter(|binding| {
                    *binding != prefix
                        && !user.iter().any(|custom| {
                            custom.enabled
                                && custom.prefix
                                && custom
                                    .member
                                    .keys
                                    .matches(KeyEvent::press(binding.code, binding.mods))
                        })
                })
                .collect();
            if retained.is_empty() {
                entry.enabled = false;
                defaults.push(entry);
            } else if retained.len() == keys.len() {
                defaults.push(entry);
            } else {
                defaults.extend(retained.into_iter().map(|binding| Entry {
                    member: PrefixMember {
                        keys: KeySet::One(binding),
                        ..entry.member.clone()
                    },
                    ..entry.clone()
                }));
            }
        }
        user.extend(defaults);
        let order = Self::default().entries;
        user.sort_by_key(|entry| {
            if let PrefixDesc::ConfiguredCommand(index) = entry.desc {
                return order.len() + usize::from(index) + 1;
            }
            // Configured-only actions have no default chord to displace.
            if entry.member.action == PrefixAction::SelectAgentDigit {
                return order.len();
            }
            order
                .iter()
                .position(|original| original.member.action == entry.member.action)
                .expect("implemented action has a default registry position")
        });
        Self {
            commands: commands
                .into_iter()
                .map(|entry| ConfiguredName {
                    id: entry.id,
                    name: None,
                })
                .collect(),
            prefix,
            entries: user,
        }
    }
}

fn configuration_trigger(entry: &Entry) -> String {
    let chord = match entry.member.keys {
        KeySet::CharRange(first, last) => format!("{first}..{last}"),
        KeySet::One(binding) => configuration_chord(binding),
    };
    if entry.prefix {
        format!("prefix+{chord}")
    } else {
        chord
    }
}

fn configuration_chord(binding: Binding) -> String {
    let mut parts = Vec::new();
    if binding.mods.ctrl {
        parts.push("ctrl".to_owned());
    }
    if binding.mods.alt {
        parts.push("alt".to_owned());
    }
    if binding.mods.shift && !matches!(binding.code, KeyCode::Char(key) if key.is_ascii_lowercase())
    {
        parts.push("shift".to_owned());
    }
    if binding.mods.super_ {
        parts.push("super".to_owned());
    }
    let key = match binding.code {
        KeyCode::Char('+') => "plus".to_owned(),
        KeyCode::Char(' ') => "space".to_owned(),
        KeyCode::Char(key) if binding.mods.shift && key.is_ascii_lowercase() => {
            key.to_ascii_uppercase().to_string()
        }
        KeyCode::Char(key) => key.to_string(),
        KeyCode::Esc => "esc".to_owned(),
        KeyCode::Backspace => "backspace".to_owned(),
        KeyCode::Delete => "delete".to_owned(),
        KeyCode::Insert => "insert".to_owned(),
        KeyCode::Home => "home".to_owned(),
        KeyCode::End => "end".to_owned(),
        KeyCode::PageUp => "pageup".to_owned(),
        KeyCode::PageDown => "pagedown".to_owned(),
        KeyCode::Up => "up".to_owned(),
        KeyCode::Down => "down".to_owned(),
        KeyCode::Left => "left".to_owned(),
        KeyCode::Right => "right".to_owned(),
        KeyCode::F(number) => format!("f{number}"),
        KeyCode::Enter | KeyCode::Tab | KeyCode::Space => {
            unreachable!("bindings are stored in canonical form")
        }
    };
    parts.push(key);
    parts.join("+")
}

fn shifted_agent_digit(member: &PrefixMember, event: KeyEvent) -> bool {
    if member.action != PrefixAction::SelectAgentDigit {
        return false;
    }
    let KeySet::One(binding) = member.keys else {
        return false;
    };
    if !binding.mods.shift {
        return false;
    }
    let digit = match event.code {
        KeyCode::Char('!') => '1',
        KeyCode::Char('@') => '2',
        KeyCode::Char('#') => '3',
        KeyCode::Char('$') => '4',
        KeyCode::Char('%') => '5',
        KeyCode::Char('^') => '6',
        KeyCode::Char('&') => '7',
        KeyCode::Char('*') => '8',
        KeyCode::Char('(') => '9',
        _ => return false,
    };
    binding.matches(KeyEvent {
        code: KeyCode::Char(digit),
        mods: event.mods.with_shift(),
        ..event
    })
}

fn chord(value: &orchbee_local_runtime::KeyChord) -> Binding {
    let code = match value.key.as_str() {
        "esc" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        key if key.starts_with('f') && key.len() > 1 => {
            KeyCode::F(key[1..].parse().expect("validated function key"))
        }
        key => KeyCode::Char(key.chars().next().expect("validated key")),
    };
    Binding::new(
        code,
        Modifiers {
            ctrl: value.ctrl,
            alt: value.alt,
            shift: value.shift,
            super_: value.super_key,
        },
    )
}

fn action(name: &str) -> (PrefixAction, PrefixDesc, ShellCommand) {
    use ClientAction as A;
    use PrefixDesc as D;
    let (action, desc) = match name {
        "menu" => (A::OpenActionMenu, D::ActionMenu),
        "copy_mode" => (A::EnterCopyMode, D::CopyMode),
        "resize_mode" => (A::EnterResizeMode, D::ResizeMode),
        "resize_pane_left" => (
            A::ResizePane {
                direction: PanelDirection::Left,
                fine: false,
            },
            D::ResizeLeft,
        ),
        "resize_pane_right" => (
            A::ResizePane {
                direction: PanelDirection::Right,
                fine: false,
            },
            D::ResizeRight,
        ),
        "resize_pane_up" => (
            A::ResizePane {
                direction: PanelDirection::Up,
                fine: false,
            },
            D::ResizeUp,
        ),
        "resize_pane_down" => (
            A::ResizePane {
                direction: PanelDirection::Down,
                fine: false,
            },
            D::ResizeDown,
        ),
        "reload_config" => (A::ReloadConfiguration, D::ReloadConfiguration),
        "help" => return (PrefixAction::ShowHelp, D::ShowHelp, ShellCommand::None),
        "workspace_picker" => {
            return (
                PrefixAction::NavigatorToggle,
                D::NavigatorToggle,
                ShellCommand::None,
            );
        }
        "toggle_sidebar" => {
            return (
                PrefixAction::SidebarToggle,
                D::SidebarToggle,
                ShellCommand::None,
            );
        }
        "switch_tab" => {
            return (
                PrefixAction::SelectTabDigit,
                D::SelectTabDigit,
                ShellCommand::None,
            );
        }
        "detach" => return (PrefixAction::Client(A::Exit), D::Exit, ShellCommand::Exit),
        "new_workspace" => (A::RequestWorkspaceCreate, D::WorkspaceCreate),
        "new_worktree" => (A::RequestWorktreeCreate, D::WorktreeCreate),
        "open_worktree" => (A::RequestWorktreeOpen, D::WorktreeOpen),
        "rename_workspace" => (A::RequestWorkspaceRename, D::WorkspaceRename),
        "previous_workspace" => (A::SelectPreviousContext, D::SelectContext),
        "next_workspace" => (A::SelectNextContext, D::SelectContext),
        "previous_agent" => (A::PreviousAgent, D::AgentCycle),
        "next_agent" => (A::NextAgent, D::AgentCycle),
        "focus_agent" => {
            return (
                PrefixAction::SelectAgentDigit,
                D::AgentIndex,
                ShellCommand::None,
            );
        }
        "new_tab" => (A::CreateTab, D::CreateTab),
        "rename_tab" => (A::RequestRenameTab, D::RenameTab),
        "previous_tab" => (A::SelectPreviousTab, D::SelectTabWrap),
        "next_tab" => (A::SelectNextTab, D::SelectTabWrap),
        "move_tab_previous" => (A::MovePreviousTab, D::MoveTab),
        "move_tab_next" => (A::MoveNextTab, D::MoveTab),
        "close_tab" => (A::RequestCloseTab, D::CloseTab),
        "focus_pane_left" => (A::MovePanelFocus(PanelDirection::Left), D::PanelFocus),
        "focus_pane_down" => (A::MovePanelFocus(PanelDirection::Down), D::PanelFocus),
        "focus_pane_up" => (A::MovePanelFocus(PanelDirection::Up), D::PanelFocus),
        "focus_pane_right" => (A::MovePanelFocus(PanelDirection::Right), D::PanelFocus),
        "split_vertical" => (A::SplitPanel(PanelDirection::Right), D::SplitPanel),
        "split_horizontal" => (A::SplitPanel(PanelDirection::Down), D::SplitPanel),
        "close_pane" => (A::ClosePanel, D::ClosePanel),
        "zoom" => (A::ToggleZoom, D::PanelZoom),
        _ => unreachable!("shared settings admit only implemented actions"),
    };
    (PrefixAction::Client(action), desc, ShellCommand::None)
}

#[cfg(test)]
mod tests;
