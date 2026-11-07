//! Pure scalar and catalog draft edits; executable paths and arguments remain literal data.
use super::{
    AdjustmentStep, ChoicePicker, Confirmation, Draft, Edit, Page, Row, SettingControl, setting,
};
use orchbee_workbench_protocol::{PreferenceKey, UserLaunchCommand, UserLaunchProfile};

/// Decimal MB preserves every persisted byte without floating-point rounding.
pub(crate) fn scrollback_megabytes(bytes: u32) -> String {
    let whole = bytes / 1_000_000;
    let fraction = bytes % 1_000_000;
    if fraction == 0 {
        return whole.to_string();
    }
    format!("{whole}.{fraction:06}")
        .trim_end_matches('0')
        .to_owned()
}

fn parse_scrollback_megabytes(value: &str) -> Option<u32> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole = whole.parse::<u32>().ok()?.checked_mul(1_000_000)?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u32>()
            .ok()?
            .checked_mul(10u32.pow(6 - fraction.len() as u32))?
    };
    whole.checked_add(fraction)
}

fn integer_bounds(key: PreferenceKey) -> (u64, u64) {
    let SettingControl::BoundedInteger {
        minimum, maximum, ..
    } = setting(key).control
    else {
        unreachable!("integer preference metadata")
    };
    (minimum, maximum)
}

fn integer_step(key: PreferenceKey, step: AdjustmentStep) -> (u64, u64, u64) {
    let SettingControl::BoundedInteger {
        minimum,
        maximum,
        small_step,
        large_step,
        ..
    } = setting(key).control
    else {
        unreachable!("integer preference metadata")
    };
    (
        minimum,
        maximum,
        match step {
            AdjustmentStep::Small => small_step,
            AdjustmentStep::Large => large_step,
        },
    )
}

fn step_integer(current: u64, key: PreferenceKey, forward: bool, step: AdjustmentStep) -> u64 {
    let (minimum, maximum, amount) = integer_step(key, step);
    if forward {
        current.saturating_add(amount).min(maximum)
    } else {
        current.saturating_sub(amount).max(minimum)
    }
}

pub(super) fn adjust(draft: &mut Draft, forward: bool, step: AdjustmentStep) {
    if step == AdjustmentStep::Small
        && let Some(mut picker) = ChoicePicker::open(draft.row, &draft.values)
    {
        picker.navigate(forward);
        picker.candidate().apply(&mut draft.values);
        return;
    }
    let values = &mut draft.values;
    match draft.row {
        Row::CompletionThreshold => {
            values.completion_reminder_minimum_seconds = u16::try_from(step_integer(
                u64::from(values.completion_reminder_minimum_seconds),
                PreferenceKey::CompletionReminderMinimumSeconds,
                forward,
                step,
            ))
            .expect("completion threshold metadata fits u16");
        }
        Row::Scrollback => {
            values.scrollback_bytes = u32::try_from(step_integer(
                u64::from(values.scrollback_bytes),
                PreferenceKey::ScrollbackBytes,
                forward,
                step,
            ))
            .expect("scrollback metadata fits u32");
        }
        _ if step == AdjustmentStep::Large => {}
        Row::CopyOnSelect => values.copy_on_select = !values.copy_on_select,
        Row::TrimAgents => {
            values.trim_trailing_blank_rows_for_agents = !values.trim_trailing_blank_rows_for_agents
        }
        Row::ResumeAgents => values.resume_agents_on_restore = !values.resume_agents_on_restore,
        Row::RestoreOutput => values.restore_terminal_output = !values.restore_terminal_output,
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
                if !draft.login_shell_supported {
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
        Row::LaunchProfiles => draft.enter_profiles(),
        Row::AgentIntegrations => draft.enter_page(
            Page::Integrations,
            Row::Agent(orchbee_agent_integration::IntegrationId::ALL[0]),
        ),
        Row::Back => draft.return_to_parent(),
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
        _ => adjust(draft, true, AdjustmentStep::Small),
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
        Row::Scrollback => Some(scrollback_megabytes(values.scrollback_bytes)),
        Row::ProfileName => Some(
            values.launch_profiles.profiles[draft.profile]
                .name
                .clone()
                .unwrap_or_default(),
        ),
        Row::Executable => match &values.launch_profiles.profiles[draft.profile].command {
            UserLaunchCommand::CustomExecutable { executable, .. } => Some(executable.clone()),
            UserLaunchCommand::SystemShell { .. } => None,
        },
        Row::ArgumentValue => match &values.launch_profiles.profiles[draft.profile].command {
            UserLaunchCommand::CustomExecutable { arguments, .. } => {
                arguments.get(draft.argument).cloned()
            }
            UserLaunchCommand::SystemShell { .. } => None,
        },
        _ => None,
    }
}
pub(super) fn edit_text(draft: &mut Draft, edit: Edit) {
    let Some(editor) = &mut draft.text else {
        return;
    };
    draft.invalid = false;
    draft.invalid_field = None;
    match edit {
        Edit::Insert(character) if !character.is_control() => {
            draft.invalid = editor.insert(&character.to_string()).is_err()
        }
        Edit::Backspace => {
            editor.backspace();
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
        Edit::Line(key) => draft.invalid = editor.apply(key).is_err(),
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
            let (minimum, maximum) =
                integer_bounds(PreferenceKey::CompletionReminderMinimumSeconds);
            if !(minimum..=maximum).contains(&u64::from(number)) {
                return false;
            }
            draft.values.completion_reminder_minimum_seconds = number;
        }
        Row::Scrollback => {
            let Some(number) = parse_scrollback_megabytes(&value) else {
                return false;
            };
            let (minimum, maximum) = integer_bounds(PreferenceKey::ScrollbackBytes);
            if !(minimum..=maximum).contains(&u64::from(number)) {
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
