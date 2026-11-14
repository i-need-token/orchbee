//! Language-neutral completion projection of the invocation authority.
use crate::invocation::grammar::{self as authority, TargetRule};
use serde_json::json;

pub(super) fn build() -> serde_json::Value {
    let commands = super::offered_commands().map(|command| json!({
        "words": command.words,
        "flags": command.listed_flags(),
        "conflicts": command.flags().iter().copied().chain(command.format.then_some("--format")).map(|flag| json!({"flag":flag,"with":command.conflicts(flag)})).collect::<Vec<_>>(),
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
        "agentTarget": command.target == TargetRule::Agent,
        "machine": command.machine,
        "globalFlags": *super::GLOBAL_FLAGS,
        "format": command.format,
    })).collect::<Vec<_>>();
    json!({
        "commands": commands,
        "trimCharacters": authority::trim_characters(),
        "valueFlags": authority::OPTIONS.iter().filter(|option| option.value.is_some()).map(|option| option.spelling).chain(["--format"]).collect::<Vec<_>>(),
        "joinedFlags": authority::OPTIONS.iter().filter(|option| option.value.is_some() && option.joined).map(|option| option.spelling).chain(["--format"]).collect::<Vec<_>>(),
        "flags": authority::OPTIONS.iter().map(|option| option.spelling).chain(["--format"]).collect::<Vec<_>>(),
        "globalFlags": *super::GLOBAL_FLAGS,
        "prefixFlags": authority::PREFIX_FLAGS,
        "rootModeFlags": authority::ROOT_MODES.iter().map(|flag| flag.spelling).collect::<Vec<_>>(),
        "rootModeFlagsRequireNoSeenFlags": authority::ROOT_MODES.iter().filter(|flag| flag.requires_no_seen_flags).map(|flag| flag.spelling).collect::<Vec<_>>(),
        "repeatable": *super::REPEATABLE_FLAGS,
        "shells": authority::values::SHELLS,
        "agentTargets": super::agent_target_values(),
        "helpTopics": *super::HELP_TOPICS,
        "helpChildren": *super::HELP_CHILDREN,
        "helpLevel3": *super::HELP_LEVEL3,
    })
}

fn string_rule(rule: Option<&authority::StringRule>) -> serde_json::Value {
    rule.map_or(
        serde_json::Value::Null,
        |rule| json!({ "allowed": rule.allowed, "minimum": rule.minimum, "maximum": rule.maximum, "excluded": rule.excluded, "trim": rule.trim }),
    )
}
