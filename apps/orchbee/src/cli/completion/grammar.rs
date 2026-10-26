//! Language-neutral completion projection of the invocation authority.
use crate::invocation::grammar::{self as authority, TargetRule};
use serde_json::json;

pub(super) fn build() -> serde_json::Value {
    let commands = authority::COMMANDS.iter().map(|command| json!({
        "words": command.words,
        "flags": command.flags(),
        "conflicts": command.flags().iter().map(|flag| json!({"flag":flag,"with":command.conflicts(flag)})).collect::<Vec<_>>(),
        "operand": command.maximum_operands,
        "operandString": string_rule(command.operand_string_rule()),
        "operandEquals": if command.operand_string_rule().is_some() { "--session" } else { "" },
        "reductions": command.operands().reductions,
        "selectors": command.operands().selectors,
        "operandValues": command.operands().values,
        "operandValueOffset": command.operands().value_offset,
        "repeatable": command.flags().iter().filter(|flag| command.repeats(flag)).collect::<Vec<_>>(),
        "requirements": command.requirements().map(|rule| json!({"flag": rule.flag, "any": rule.any})).collect::<Vec<_>>(),
        "values": command.flags().iter().copied().chain(super::GLOBAL_FLAGS.iter().copied()).chain(command.format.then_some("--format")).map(|flag| {
            let (values, closed) = command.values(flag);
            json!({"flag": flag, "choices": values, "closed": closed, "string": string_rule(command.string_rule(flag))})
        }).collect::<Vec<_>>(),
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
        "joinedFlags": authority::OPTIONS.iter().filter(|option| option.value.is_some() && option.joined).map(|option| option.spelling).chain(["--format"]).collect::<Vec<_>>(),
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

fn string_rule(rule: Option<&authority::StringRule>) -> serde_json::Value {
    rule.map_or(
        serde_json::Value::Null,
        |rule| json!({ "pattern": rule.pattern, "excluded": rule.excluded, "trim": rule.trim }),
    )
}
