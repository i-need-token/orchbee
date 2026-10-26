//! Flat, literal-only projection for shells without a JSON reader.
use crate::invocation::grammar::{self as authority, TargetRule};
use std::collections::BTreeMap;

pub(super) fn rows() -> BTreeMap<String, Vec<String>> {
    let mut rows = BTreeMap::new();
    let mut insert = |key: String, values: Vec<&str>| {
        rows.insert(key, values.into_iter().map(str::to_owned).collect());
    };
    insert(
        "commands".into(),
        authority::COMMANDS
            .iter()
            .map(|command| command.key)
            .collect(),
    );
    insert("root".into(), super::HELP_TOPICS.to_vec());
    insert("global".into(), super::GLOBAL_FLAGS.to_vec());
    insert(
        "modes".into(),
        authority::ROOT_MODES
            .iter()
            .map(|mode| mode.spelling)
            .collect(),
    );
    insert(
        "empty-modes".into(),
        authority::ROOT_MODES
            .iter()
            .filter(|mode| mode.requires_no_seen_flags)
            .map(|mode| mode.spelling)
            .collect(),
    );
    insert("prefix".into(), authority::PREFIX_FLAGS.to_vec());
    insert("shells".into(), authority::values::SHELLS.to_vec());
    insert(
        "flags".into(),
        authority::OPTIONS
            .iter()
            .map(|option| option.spelling)
            .chain(["--format"])
            .collect(),
    );
    insert(
        "value-flags".into(),
        authority::OPTIONS
            .iter()
            .filter(|option| option.value.is_some())
            .map(|option| option.spelling)
            .chain(["--format"])
            .collect(),
    );
    insert(
        "joined-flags".into(),
        authority::OPTIONS
            .iter()
            .filter(|option| option.value.is_some() && option.joined)
            .map(|option| option.spelling)
            .chain(["--format"])
            .collect(),
    );
    for family in authority::FAMILIES {
        insert(
            format!("help|{}", family.key),
            authority::children(family.key).to_vec(),
        );
    }
    for command in authority::COMMANDS {
        let key = command.key;
        insert(format!("words|{key}"), command.words.to_vec());
        if let Some(rule) = command.operand_string_rule() {
            insert(format!("string|{key}|operand"), vec![rule.pattern]);
            insert(format!("excluded|{key}|operand"), rule.excluded.to_vec());
            insert(format!("operand-equals|{key}"), vec!["--session"]);
        }
        let flags = command
            .flags()
            .iter()
            .copied()
            .chain(super::GLOBAL_FLAGS.iter().copied())
            .chain(command.format.then_some("--format"))
            .collect::<Vec<_>>();
        insert(format!("flags|{key}"), flags.clone());
        insert(
            format!("repeat|{key}"),
            flags
                .iter()
                .copied()
                .filter(|flag| command.repeats(flag))
                .collect(),
        );
        insert(
            format!("machine|{key}"),
            if command.machine { vec!["1"] } else { vec![] },
        );
        insert(
            format!("agent|{key}"),
            if command.target == TargetRule::Agent {
                vec!["1"]
            } else {
                vec![]
            },
        );
        insert(
            format!("selectors|{key}"),
            command.operands().selectors.to_vec(),
        );
        insert(
            format!("operand-values|{key}"),
            command.operands().values.to_vec(),
        );
        for flag in flags {
            insert(
                format!("conflicts|{key}|{flag}"),
                command.conflicts(flag).to_vec(),
            );
            if let Some(rule) = command.string_rule(flag) {
                insert(format!("string|{key}|{flag}"), vec![rule.pattern]);
                insert(format!("excluded|{key}|{flag}"), rule.excluded.to_vec());
                if rule.trim {
                    insert(format!("trim|{key}|{flag}"), vec!["1"]);
                }
            }
            let (values, closed) = command.values(flag);
            insert(format!("values|{key}|{flag}"), values.to_vec());
            if closed {
                insert(format!("closed|{key}|{flag}"), vec!["1"]);
            }
            if let Some(rule) = command.requirements().find(|rule| rule.flag == flag) {
                insert(format!("requires|{key}|{flag}"), rule.any.to_vec());
            }
        }
    }
    // Numeric rows own no vocabulary; values come directly from the descriptors.
    for command in authority::COMMANDS {
        rows.insert(
            format!("limit|{}", command.key),
            vec![command.maximum_operands.to_string()],
        );
        rows.insert(
            format!("offset|{}", command.key),
            vec![command.operands().value_offset.to_string()],
        );
        rows.insert(
            format!("reductions|{}", command.key),
            command
                .operands()
                .reductions
                .iter()
                .map(|group| group.join(" "))
                .collect(),
        );
    }
    rows.insert("agent-targets".into(), super::agent_target_values());
    rows
}

pub(super) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "'\\''"))
}
