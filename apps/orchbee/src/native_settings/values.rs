//! Pure scalar and catalog draft edits; executable paths and arguments remain literal data.
use super::{Confirmation, Draft, Edit, Page, Row};
use orchbee_workbench_protocol::{
    AgentReminderMode, ApplicationAppearance, CompletionReminderMode, PreferencesLocale,
    TerminalDisplayAppearance, UserLaunchCommand, UserLaunchProfile,
};

fn cycle<T: Copy + PartialEq>(current: &mut T, values: &[T], forward: bool) {
    let index = values
        .iter()
        .position(|value| value == current)
        .unwrap_or(0);
    *current = values[(index + if forward { 1 } else { values.len() - 1 }) % values.len()];
}
pub(super) fn adjust(draft: &mut Draft, forward: bool) {
    let values = &mut draft.values;
    match draft.row {
        Row::Language => cycle(
            &mut values.locale,
            &[
                PreferencesLocale::System,
                PreferencesLocale::EnUs,
                PreferencesLocale::ZhCn,
            ],
            forward,
        ),
        Row::Theme => cycle(
            &mut values.application_theme,
            &[
                ApplicationAppearance::System,
                ApplicationAppearance::Light,
                ApplicationAppearance::Dark,
            ],
            forward,
        ),
        Row::TerminalTheme => cycle(
            &mut values.terminal_display_theme,
            &[
                TerminalDisplayAppearance::Host,
                TerminalDisplayAppearance::Light,
                TerminalDisplayAppearance::Dark,
                TerminalDisplayAppearance::Custom,
            ],
            forward,
        ),
        Row::CopyOnSelect => values.copy_on_select = !values.copy_on_select,
        Row::TrimAgents => {
            values.trim_trailing_blank_rows_for_agents = !values.trim_trailing_blank_rows_for_agents
        }
        Row::Reminders => cycle(
            &mut values.agent_notification_mode,
            &[AgentReminderMode::Never, AgentReminderMode::Hidden],
            forward,
        ),
        Row::CompletionReminders => cycle(
            &mut values.completion_reminder_mode,
            &[
                CompletionReminderMode::Never,
                CompletionReminderMode::Unfocused,
                CompletionReminderMode::Always,
            ],
            forward,
        ),
        Row::Sound => values.sound_enabled = !values.sound_enabled,
        Row::Profile => {
            let count = values.launch_profiles.profiles.len();
            draft.profile = (draft.profile + if forward { 1 } else { count - 1 }) % count;
            draft.argument = 0;
        }
        Row::CommandType => {
            let profile = &mut values.launch_profiles.profiles[draft.profile];
            profile.command = match profile.command {
                UserLaunchCommand::SystemShell { .. } => UserLaunchCommand::CustomExecutable {
                    executable: String::new(),
                    arguments: Vec::new(),
                },
                UserLaunchCommand::CustomExecutable { .. } => {
                    UserLaunchCommand::SystemShell { login: false }
                }
            };
            draft.argument = 0;
        }
        Row::Login => {
            if let UserLaunchCommand::SystemShell { login } =
                &mut values.launch_profiles.profiles[draft.profile].command
            {
                if cfg!(windows) {
                    draft.invalid = true;
                } else {
                    *login = !*login;
                }
            }
        }
        Row::Argument => {
            if let UserLaunchCommand::CustomExecutable { arguments, .. } =
                &values.launch_profiles.profiles[draft.profile].command
                && !arguments.is_empty()
            {
                draft.argument = (draft.argument + if forward { 1 } else { arguments.len() - 1 })
                    % arguments.len();
            }
        }
        _ => {}
    }
}
pub(super) fn activate(draft: &mut Draft) {
    match draft.row {
        Row::LaunchProfiles => {
            draft.page = Page::Profiles;
            draft.row = Row::Profile;
        }
        Row::Back => {
            draft.page = Page::General;
            draft.row = Row::LaunchProfiles;
        }
        Row::ResetAll => draft.confirmation = Some(Confirmation::ResetAll),
        Row::ResetProfiles => draft.confirmation = Some(Confirmation::ResetProfiles),
        Row::Reload => draft.confirmation = Some(Confirmation::Reload),
        Row::SetDefault => {
            draft.values.launch_profiles.default_profile_id = draft.values.launch_profiles.profiles
                [draft.profile]
                .id
                .clone()
        }
        Row::DeleteProfile if draft.values.launch_profiles.profiles.len() > 1 => {
            draft.confirmation = Some(Confirmation::DeleteProfile)
        }
        Row::AddProfile if draft.values.launch_profiles.profiles.len() < 64 => {
            let profiles = &mut draft.values.launch_profiles.profiles;
            let Some(id) = (1..=65)
                .map(|index| format!("profile-{index}"))
                .find(|id| profiles.iter().all(|profile| &profile.id != id))
            else {
                return;
            };
            profiles.push(UserLaunchProfile {
                id: id.clone(),
                name: Some(id),
                command: UserLaunchCommand::SystemShell { login: false },
            });
            draft.profile = profiles.len() - 1;
            draft.argument = 0;
            draft.row = Row::ProfileName;
        }
        Row::AddArgument | Row::DeleteArgument => {
            if let UserLaunchCommand::CustomExecutable { arguments, .. } =
                &mut draft.values.launch_profiles.profiles[draft.profile].command
            {
                if draft.row == Row::AddArgument && arguments.len() < 128 {
                    arguments.push(String::new());
                    draft.argument = arguments.len() - 1;
                    draft.row = Row::ArgumentValue;
                } else if draft.row == Row::DeleteArgument && draft.argument < arguments.len() {
                    arguments.remove(draft.argument);
                    draft.argument = draft.argument.min(arguments.len().saturating_sub(1));
                }
            }
        }
        _ => adjust(draft, true),
    }
}
pub(super) fn delete_profile(draft: &mut Draft) {
    let catalog = &mut draft.values.launch_profiles;
    if catalog.profiles.len() <= 1 {
        return;
    }
    let removed = catalog.profiles.remove(draft.profile);
    draft.profile = draft.profile.min(catalog.profiles.len() - 1);
    draft.argument = 0;
    if catalog.default_profile_id == removed.id {
        catalog.default_profile_id = catalog.profiles[draft.profile].id.clone();
    }
    draft.row = Row::Profile;
}
pub(super) fn text(draft: &Draft) -> Option<String> {
    let values = &draft.values;
    match draft.row {
        Row::Foreground => Some(values.custom_foreground.clone()),
        Row::Background => Some(values.custom_background.clone()),
        Row::CompletionThreshold => Some(values.completion_reminder_minimum_seconds.to_string()),
        Row::Scrollback => Some(values.scrollback_bytes.to_string()),
        Row::ProfileName => Some(
            values.launch_profiles.profiles[draft.profile]
                .name
                .clone()
                .unwrap_or_default(),
        ),
        Row::Executable => match &values.launch_profiles.profiles[draft.profile].command {
            UserLaunchCommand::CustomExecutable { executable, .. } => Some(executable.clone()),
            _ => None,
        },
        Row::ArgumentValue => match &values.launch_profiles.profiles[draft.profile].command {
            UserLaunchCommand::CustomExecutable { arguments, .. } => {
                arguments.get(draft.argument).cloned()
            }
            _ => None,
        },
        _ => None,
    }
}
pub(super) fn edit_text(draft: &mut Draft, edit: Edit) {
    let Some(editor) = &mut draft.text else {
        return;
    };
    draft.invalid = false;
    match edit {
        Edit::Insert(character) if !character.is_control() => {
            draft.invalid = editor.insert(&character.to_string()).is_err()
        }
        Edit::Backspace => {
            editor.backspace();
        }
        Edit::WordBack => {
            editor.word_back();
        }
        Edit::Delete => {
            editor.delete();
        }
        Edit::Adjust(false) => {
            editor.left();
        }
        Edit::Adjust(true) => {
            editor.right();
        }
        Edit::Home => {
            editor.home();
        }
        Edit::End => {
            editor.end();
        }
        Edit::Clear => {
            *editor = super::LineEditor::new(
                String::new(),
                super::LengthLimit::Bytes(draft.row.text_limit().unwrap_or(0)),
            );
        }
        Edit::Activate => {
            let value = editor.text().to_owned();
            if accept_text(draft, value) {
                draft.text = None;
            } else {
                draft.invalid = true;
            }
        }
        _ => {}
    }
}
fn accept_text(draft: &mut Draft, value: String) -> bool {
    match draft.row {
        Row::Foreground | Row::Background => {
            if value.len() != 7
                || !value.starts_with('#')
                || !value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return false;
            }
            if draft.row == Row::Foreground {
                draft.values.custom_foreground = value.to_ascii_lowercase();
            } else {
                draft.values.custom_background = value.to_ascii_lowercase();
            }
        }
        Row::CompletionThreshold => {
            let Ok(number) = value.parse::<u16>() else {
                return false;
            };
            if !(1..=3600).contains(&number) {
                return false;
            }
            draft.values.completion_reminder_minimum_seconds = number;
        }
        Row::Scrollback => {
            let Ok(number) = value.parse::<u32>() else {
                return false;
            };
            if number > 1_000_000_000 {
                return false;
            }
            draft.values.scrollback_bytes = number;
        }
        Row::ProfileName => {
            let profile = &mut draft.values.launch_profiles.profiles[draft.profile];
            if value.is_empty() && profile.id == "system-default" {
                profile.name = None;
            } else if value.trim().is_empty() || value.trim() != value {
                return false;
            } else {
                profile.name = Some(value);
            }
        }
        Row::Executable => {
            if !std::path::Path::new(&value).is_absolute() {
                return false;
            }
            if let UserLaunchCommand::CustomExecutable { executable, .. } =
                &mut draft.values.launch_profiles.profiles[draft.profile].command
            {
                *executable = value;
            }
        }
        Row::ArgumentValue => {
            if let UserLaunchCommand::CustomExecutable { arguments, .. } =
                &mut draft.values.launch_profiles.profiles[draft.profile].command
                && let Some(argument) = arguments.get_mut(draft.argument)
            {
                *argument = value;
            }
        }
        _ => return false,
    }
    true
}
