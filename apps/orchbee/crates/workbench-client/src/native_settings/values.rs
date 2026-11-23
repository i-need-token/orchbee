//! Pure scalar and catalog edits on the page's values; executable paths and arguments remain
//! literal data.
use super::{AdjustmentStep, Choice, ChoicePicker, Edit, PageState as Draft, Row, SettingControl};
use crate::localization::SettingsTextRequirement;
use orchbee_local_runtime::InstalledShell;
use orchbee_workbench_protocol::{UserLaunchCommand, UserLaunchProfile};

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

fn integer_bounds(row: Row) -> (u64, u64) {
    let Some(SettingControl::BoundedInteger {
        minimum, maximum, ..
    }) = row.setting().map(|metadata| metadata.control)
    else {
        unreachable!("integer preference metadata")
    };
    (minimum, maximum)
}

fn integer_step(row: Row, step: AdjustmentStep) -> (u64, u64, u64) {
    let Some(SettingControl::BoundedInteger {
        minimum,
        maximum,
        small_step,
        large_step,
        ..
    }) = row.setting().map(|metadata| metadata.control)
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

fn step_integer(current: u64, row: Row, forward: bool, step: AdjustmentStep) -> u64 {
    let (minimum, maximum, amount) = integer_step(row, step);
    if forward {
        current.saturating_add(amount).min(maximum)
    } else {
        current.saturating_sub(amount).max(minimum)
    }
}

pub(super) fn adjust(draft: &mut Draft, forward: bool, step: AdjustmentStep) {
    if step == AdjustmentStep::Small
        && let Some(mut picker) = Choice::from_values(draft.row, &draft.values)
            .and_then(|original| ChoicePicker::open(draft.row, original))
    {
        picker.navigate(forward);
        picker.candidate().apply(&mut draft.values);
        return;
    }
    let values = &mut draft.values;
    if step == AdjustmentStep::Small
        && let Some(flag) = draft.row.flag()
    {
        (flag.write)(values, !(flag.read)(values));
        return;
    }
    match draft.row {
        Row::AgentYolo(provider) => {
            let launch = values.agent_launch.entry(provider).or_default();
            launch.yolo = !launch.yolo;
        }
        Row::CompletionThreshold => {
            values.completion_reminder_minimum_seconds = u16::try_from(step_integer(
                u64::from(values.completion_reminder_minimum_seconds),
                Row::CompletionThreshold,
                forward,
                step,
            ))
            .expect("completion threshold metadata fits u16");
        }
        Row::Scrollback => {
            values.scrollback_bytes = u32::try_from(step_integer(
                u64::from(values.scrollback_bytes),
                Row::Scrollback,
                forward,
                step,
            ))
            .expect("scrollback metadata fits u32");
        }
        _ if step == AdjustmentStep::Large => {}
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
        Row::SetDefault => {
            draft.values.launch_profiles.default_profile_id = draft.values.launch_profiles.profiles
                [draft.profile]
                .id
                .clone()
        }
        Row::DeleteProfile if draft.values.launch_profiles.profiles.len() > 1 => {
            draft.confirm_delete = true
        }
        Row::AddProfile => add_profile(draft, None),
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
/// Adds a profile at the end of the catalog and focuses its name: an installed shell under its
/// name, with its executable and arguments, or the system shell under a fresh name.
pub(super) fn add_profile(draft: &mut Draft, shell: Option<&InstalledShell>) {
    let profiles = &draft.values.launch_profiles.profiles;
    if profiles.len() >= 64 {
        return;
    }
    let Some(id) = (1..=65)
        .map(|index| format!("profile-{index}"))
        .find(|id| profiles.iter().all(|profile| &profile.id != id))
    else {
        return;
    };
    let (name, command) = match shell {
        Some(shell) => (
            draft.locale.installed_shell_name(shell).to_owned(),
            UserLaunchCommand::CustomExecutable {
                executable: shell.executable.clone(),
                arguments: shell.arguments.clone(),
            },
        ),
        None => (id.clone(), UserLaunchCommand::BUILT_IN),
    };
    let profiles = &mut draft.values.launch_profiles.profiles;
    profiles.push(UserLaunchProfile {
        id,
        name: Some(name),
        command,
    });
    draft.profile = profiles.len() - 1;
    draft.argument = 0;
    draft.row = Row::ProfileName;
}
pub(super) fn text(draft: &Draft) -> Option<String> {
    let values = &draft.values;
    match draft.row {
        Row::QuotaResumePrompt => Some(values.quota_resume_prompt.clone()),
        Row::AgentArguments(provider) => Some(
            values
                .agent_launch
                .get(&provider)
                .map_or_else(String::new, |launch| launch.arguments.clone()),
        ),
        Row::PrefixKey => Some(draft.text_value(draft.row).unwrap_or_default().to_owned()),
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
        _ => {}
    }
}
/// What a common row's editor accepts, for the reason a refused text names.
pub(super) fn requirement(row: Row) -> Option<SettingsTextRequirement> {
    Some(match row {
        Row::Foreground | Row::Background => SettingsTextRequirement::Color,
        Row::QuotaResumePrompt => SettingsTextRequirement::QuotaResumePrompt,
        Row::CompletionThreshold => {
            let (minimum, maximum) = integer_bounds(Row::CompletionThreshold);
            SettingsTextRequirement::WholeNumber { minimum, maximum }
        }
        Row::Scrollback => {
            let (minimum, maximum) = integer_bounds(Row::Scrollback);
            let megabytes = |bytes: u64| scrollback_megabytes(u32::try_from(bytes).unwrap_or(0));
            SettingsTextRequirement::Number {
                minimum: megabytes(minimum),
                maximum: megabytes(maximum),
            }
        }
        _ => return None,
    })
}
/// Writes an accepted text into the page's values; false leaves them as they were.
pub(super) fn accept_text(draft: &mut Draft, value: String) -> bool {
    match draft.row {
        Row::QuotaResumePrompt => {
            if !orchbee_workbench_protocol::valid_quota_resume_prompt(&value) {
                return false;
            }
            draft.values.quota_resume_prompt = value;
        }
        Row::AgentArguments(provider) => {
            let Ok(arguments) = orchbee_workbench_protocol::parse_agent_arguments(&value) else {
                return false;
            };
            let launch = draft.values.agent_launch.entry(provider).or_default();
            launch.arguments = if arguments.is_empty() {
                String::new()
            } else {
                value
            };
        }
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
            let (minimum, maximum) = integer_bounds(Row::CompletionThreshold);
            if !(minimum..=maximum).contains(&u64::from(number)) {
                return false;
            }
            draft.values.completion_reminder_minimum_seconds = number;
        }
        Row::Scrollback => {
            let Some(number) = parse_scrollback_megabytes(&value) else {
                return false;
            };
            let (minimum, maximum) = integer_bounds(Row::Scrollback);
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
