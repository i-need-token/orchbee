//! Language-neutral completion projection of the invocation authority.
use crate::invocation::grammar::{self as authority, TargetRule};
use serde_json::json;

pub(super) fn build() -> serde_json::Value {
    let commands = authority::COMMANDS.iter().map(|command| json!({
        "words": command.words,
        "flags": command.flags(),
        "conflicts": command.flags().iter().map(|flag| json!({"flag":flag,"with":command.conflicts(flag)})).collect::<Vec<_>>(),
        "operand": command.maximum_operands,
        "directions": if command.flags().contains(&"--direction") { authority::CARDINAL_DIRECTIONS } else { &[] },
        "agentTarget": command.target == TargetRule::Agent,
        "machine": command.machine,
        "globalFlags": *super::GLOBAL_FLAGS,
        "format": command.format,
        "cwdValues": if command.cwd_values { authority::values::CWD_VALUES } else { &[] },
    })).collect::<Vec<_>>();
    json!({
        "commands": commands,
        "valueFlags": authority::OPTIONS.iter().filter(|option| option.value.is_some()).map(|option| option.spelling).chain(["--format"]).collect::<Vec<_>>(),
        "flags": authority::OPTIONS.iter().map(|option| option.spelling).chain(["--format"]).collect::<Vec<_>>(),
        "globalFlags": *super::GLOBAL_FLAGS,
        "prefixFlags": authority::PREFIX_FLAGS,
        "rootModeFlags": authority::ROOT_MODES.iter().map(|flag| flag.spelling).collect::<Vec<_>>(),
        "rootModeFlagsRequireNoSeenFlags": authority::ROOT_MODES.iter().filter(|flag| flag.requires_no_seen_flags).map(|flag| flag.spelling).collect::<Vec<_>>(),
        "repeatable": *super::REPEATABLE_FLAGS,
        "shells": authority::values::SHELLS,
        "formats": authority::values::FORMAT_VALUES,
        "keys": authority::values::KEY_VALUES,
        "until": authority::values::UNTIL_VALUES,
        "providers": authority::values::PROVIDER_VALUES,
        "orders": authority::values::ORDER_VALUES,
        "searchDirections": authority::values::SEARCH_DIRECTION_VALUES,
        "copyMotions": authority::values::COPY_MOTION_VALUES,
        "rightClick": authority::values::RIGHT_CLICK_VALUES,
        "notificationPositions": ["top-left", "top-right", "bottom-left", "bottom-right"],
        "notificationSounds": ["none", "done", "request"],
        "outputSources": authority::values::OUTPUT_SOURCES,
        "agentKinds": crate::agent_provider::CANONICAL_IDS,
        "agentTargets": super::agent_target_values(),
        "helpTopics": *super::HELP_TOPICS,
        "helpChildren": *super::HELP_CHILDREN,
        "helpLevel3": *super::HELP_LEVEL3,
    })
}

pub(super) fn accepts_format(words: &[&str]) -> bool {
    authority::COMMANDS
        .iter()
        .find(|command| command.words == words)
        .is_some_and(|command| command.format)
}
