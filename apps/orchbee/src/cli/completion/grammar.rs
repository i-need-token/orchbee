//! Shared static completion grammar; shell adapters only interpret this projection.

use super::{
    BUSINESS_FLAGS, COMMAND_FLAGS, DIRECTION_VALUES, FAMILY_VERBS, FORMAT_VALUES, HELP_CHILDREN,
    HELP_LEVEL3, HELP_TOPICS, KEY_VALUES, OPERAND_PATHS, PROVIDER_VALUES, REPEATABLE_FLAGS,
    RIGHT_CLICK_VALUES, SHELLS, UNTIL_VALUES, command_flag_conflicts, command_flags, flag_value,
};
use serde_json::json;

pub(super) fn build() -> serde_json::Value {
    let mut commands = Vec::new();
    for (family, verbs) in FAMILY_VERBS {
        for verb in *verbs {
            let path = format!("{family}:{verb}");
            if HELP_LEVEL3.iter().any(|(parent, _)| *parent == path) {
                continue;
            }
            commands.push(json!({
                "words": [family, verb],
                "flags": command_flags(&path),
                "conflicts": command_flags(&path).iter().map(|flag| json!({"flag":flag,"with":command_flag_conflicts(&path,flag)})).collect::<Vec<_>>(),
                "operand": if path == "agent:start" { 65536 } else if path == "agent:send-keys" { 65 } else if matches!(path.as_str(), "agent:rename" | "agent:prompt" | "terminal:run") { 2 } else { usize::from(OPERAND_PATHS.contains(&path.as_str())) },
                "directions": DIRECTION_VALUES.iter().find(|(key, _)| *key == path).map(|(_, values)| *values).unwrap_or(&[]),
            }));
        }
    }
    for (parent, verbs) in HELP_LEVEL3 {
        for verb in *verbs {
            let path = format!("{parent}:{verb}");
            let words = path.split(':').collect::<Vec<_>>();
            commands.push(json!({"words": words, "flags": command_flags(&path), "conflicts": [], "operand": usize::from(OPERAND_PATHS.contains(&path.as_str())), "directions": []}));
        }
    }
    commands.push(
        json!({"words": ["status"], "flags": [], "conflicts": [], "operand": 0, "directions": []}),
    );
    commands
        .push(json!({"words": ["tui"], "flags": ["--mouse"], "conflicts": [], "operand": 0, "directions": []}));
    for command in &mut commands {
        let words = &command["words"];
        command["cwdValues"] = json!(if *words == json!(["tab", "create"])
            || *words == json!(["panel", "split"])
            || *words == json!(["workspace", "create"])
        {
            super::CWD_VALUES
        } else {
            &[]
        });
    }
    let grammar = json!({
        "commands": commands,
        "valueFlags": BUSINESS_FLAGS.iter().copied().filter(|flag| flag_value(flag).is_some()).chain(["--format", "--mouse"]).collect::<Vec<_>>(),
        "flags": BUSINESS_FLAGS.iter().copied().chain(["--format", "--mouse"]).collect::<Vec<_>>(),
        "repeatable": REPEATABLE_FLAGS,
        "shells": SHELLS,
        "formats": FORMAT_VALUES,
        "keys": KEY_VALUES,
        "until": UNTIL_VALUES,
        "providers": PROVIDER_VALUES,
        "rightClick": RIGHT_CLICK_VALUES,
        "outputSources": super::OUTPUT_SOURCES,
        "agentKinds": crate::agent_provider::CANONICAL_IDS,
        "helpTopics": HELP_TOPICS,
        "helpChildren": HELP_CHILDREN,
        "helpLevel3": HELP_LEVEL3,
    });
    // Validate ownership of every generated flag table in Rust as well as
    // exercising candidates in a real PowerShell process.
    debug_assert!(COMMAND_FLAGS.iter().all(|(_, flags)| {
        flags
            .iter()
            .all(|flag| *flag == "--mouse" || BUSINESS_FLAGS.contains(flag))
    }));
    grammar
}
