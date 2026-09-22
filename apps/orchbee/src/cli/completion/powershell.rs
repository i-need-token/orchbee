//! PowerShell's static grammar projection. The native completer parses only
//! literal AST words and never evaluates user expressions or queries a Site.

use super::{
    BUSINESS_FLAGS, COMMAND_FLAGS, DIRECTION_VALUES, FAMILY_VERBS, FORMAT_VALUES, HELP_CHILDREN,
    HELP_LEVEL3, HELP_TOPICS, INTEGRATION_VERBS, KEY_VALUES, OPERAND_PATHS, PROVIDER_VALUES,
    REPEATABLE_FLAGS, SHELLS, UNTIL_VALUES, command_flags, flag_value,
};
use serde_json::json;

#[cfg(all(test, windows))]
mod tests;

pub(super) fn render() -> String {
    let mut commands = Vec::new();
    for (family, verbs) in FAMILY_VERBS {
        for verb in *verbs {
            let path = format!("{family}:{verb}");
            commands.push(json!({
                "words": [family, verb],
                "flags": command_flags(&path),
                "operand": OPERAND_PATHS.contains(&path.as_str()),
                "directions": DIRECTION_VALUES.iter().find(|(key, _)| *key == path).map(|(_, values)| *values).unwrap_or(&[]),
            }));
        }
    }
    for verb in INTEGRATION_VERBS {
        commands.push(json!({"words": ["agent", "integration", verb], "flags": command_flags(&format!("agent:integration:{verb}")), "operand": false, "directions": []}));
    }
    commands.push(json!({"words": ["status"], "flags": [], "operand": false, "directions": []}));
    commands
        .push(json!({"words": ["tui"], "flags": ["--mouse"], "operand": false, "directions": []}));
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
        "helpTopics": HELP_TOPICS,
        "helpChildren": HELP_CHILDREN,
        "helpLevel3": HELP_LEVEL3,
    });
    // Validate ownership of every generated flag table in Rust as well as
    // exercising candidates in a real PowerShell process.
    debug_assert!(
        COMMAND_FLAGS
            .iter()
            .all(|(_, flags)| flags.iter().all(|flag| BUSINESS_FLAGS.contains(flag)))
    );
    include_str!("powershell.ps1").replace("__ORCHBEE_GRAMMAR__", &grammar.to_string())
}
