//! An owned Client registry shared by input and discovery, never a process-global cache.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    enabled: bool,
    pub(crate) prefix: bool,
    pub(crate) member: PrefixMember,
    pub(crate) desc: PrefixDesc,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Bindings {
    pub(crate) prefix: Binding,
    pub(crate) entries: Vec<Entry>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            prefix: PREFIX_CHORD,
            entries: PREFIX_GROUPS
                .iter()
                .flat_map(|group| {
                    group.members.iter().map(|member| Entry {
                        enabled: true,
                        prefix: true,
                        member: member.clone(),
                        desc: group.desc,
                    })
                })
                .collect(),
        }
    }
}

impl Bindings {
    pub(crate) fn descriptions(&self) -> Vec<PrefixDesc> {
        let mut result: Vec<_> = PREFIX_GROUPS.iter().map(|g| g.desc).collect();
        for entry in &self.entries {
            if !result.contains(&entry.desc) {
                result.push(entry.desc);
            }
        }
        result
    }
    pub(crate) fn group_label(&self, desc: PrefixDesc, unbound: &str) -> Option<String> {
        let entries: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.desc == desc)
            .collect();
        if entries.is_empty() {
            return None;
        }
        if entries.iter().all(|entry| !entry.enabled) {
            return Some(unbound.into());
        }
        let prefix = entries[0].prefix;
        if !entries.iter().all(|entry| entry.prefix == prefix) {
            return Some(
                entries
                    .iter()
                    .map(|entry| {
                        if !entry.enabled {
                            return unbound.to_owned();
                        }
                        let key = entry
                            .member
                            .keys
                            .label(super::super::LabelStyle::for_host());
                        if entry.prefix {
                            format!(
                                "{} {key}",
                                super::super::format_binding(
                                    &self.prefix,
                                    super::super::LabelStyle::for_host()
                                )
                            )
                        } else {
                            key
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" / "),
            );
        }
        let mut labels: Vec<_> = entries
            .iter()
            .map(|entry| {
                if entry.enabled {
                    entry
                        .member
                        .keys
                        .label(super::super::LabelStyle::for_host())
                } else {
                    unbound.into()
                }
            })
            .collect();
        let indexed = desc == PrefixDesc::SelectTabDigit;
        #[cfg(windows)]
        let indexed = indexed || desc == PrefixDesc::AgentIndex;
        if indexed && entries.len() == 9 {
            let keys: Vec<_> = entries
                .iter()
                .filter_map(|entry| match entry.member.keys {
                    KeySet::One(binding) => Some(binding),
                    _ => None,
                })
                .collect();
            if keys.len() == 9
                && keys.iter().enumerate().all(|(index, key)| {
                    key.code == KeyCode::Char(char::from(b'1' + index as u8))
                        && key.mods == keys[0].mods
                })
            {
                labels = vec![format!(
                    "{}1..9",
                    labels[0].strip_suffix('1').expect("digit binding label")
                )];
            }
        }
        let separator = if labels.len() == 2 {
            " / "
        } else if labels
            .iter()
            .any(|label| matches!(label.as_str(), "\\" | "|" | "/"))
        {
            " "
        } else {
            "/"
        };
        if !prefix {
            return Some(labels.join(separator));
        }
        Some(format!(
            "{} {}",
            super::super::format_binding(&self.prefix, super::super::LabelStyle::for_host()),
            labels.join(separator)
        ))
    }

    pub(crate) fn find(&self, event: KeyEvent, prefix: bool) -> Option<&PrefixMember> {
        let exact = self
            .entries
            .iter()
            .find(|entry| {
                entry.enabled && entry.prefix == prefix && entry.member.keys.matches(event)
            })
            .map(|entry| &entry.member);
        #[cfg(windows)]
        let exact = exact.or_else(|| {
            self.entries
                .iter()
                .find(|entry| {
                    entry.enabled
                        && entry.prefix == prefix
                        && shifted_agent_digit(&entry.member, event)
                })
                .map(|entry| &entry.member)
        });
        exact
    }

    pub(crate) fn labels(&self, desc: PrefixDesc) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.enabled && entry.desc == desc)
            .map(|entry| {
                let key = entry
                    .member
                    .keys
                    .label(super::super::LabelStyle::for_host());
                if entry.prefix {
                    format!(
                        "{} {key}",
                        super::super::format_binding(
                            &self.prefix,
                            super::super::LabelStyle::for_host()
                        )
                    )
                } else {
                    key
                }
            })
            .collect()
    }

    #[cfg(windows)]
    pub(crate) fn configured(settings: &orchbee_local_site_runtime::WindowsKeySettings) -> Self {
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
            prefix,
            entries: user,
        }
    }
}

#[cfg(windows)]
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

#[cfg(windows)]
fn chord(value: &orchbee_local_site_runtime::WindowsKeyChord) -> Binding {
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

#[cfg(windows)]
fn action(name: &str) -> (PrefixAction, PrefixDesc, ShellCommand) {
    use ClientAction as A;
    use PrefixDesc as D;
    let (action, desc) = match name {
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
