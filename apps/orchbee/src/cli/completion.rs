//! Static completion scripts for the implemented grammar. The scripts are
//! emitted to stdout only; nothing is installed, no user files are read or
//! written, and no resource is enumerated.

use crate::invocation::CompletionShell;

/// Top-level words the grammar accepts before any option.
const ROOT_WORDS: &[&str] = &[
    "api",
    "completion",
    "config",
    "help",
    "panel",
    "project",
    "status",
    "tab",
    "terminal",
    "tui",
    "workspace",
];

const ROOT_FLAGS: &[&str] = &["--help", "--version"];

/// family → the verbs it implements.
const FAMILY_VERBS: &[(&str, &[&str])] = &[
    ("api", &["schema", "snapshot"]),
    ("config", &["check"]),
    ("panel", &["get", "list"]),
    ("project", &["get", "list"]),
    ("tab", &["get", "list"]),
    ("terminal", &["get", "list"]),
    ("workspace", &["get", "list"]),
];

/// Help paths beyond the bare families.
const HELP_COMMANDS: &[&str] = &[
    "api schema",
    "api snapshot",
    "config check",
    "panel get",
    "panel list",
    "project get",
    "project list",
    "status",
    "tab get",
    "tab list",
    "terminal get",
    "terminal list",
    "tui",
    "workspace get",
    "workspace list",
];

const SHELLS: &[&str] = &["bash", "fish", "zsh"];

pub(crate) fn render(shell: CompletionShell) -> String {
    match shell {
        CompletionShell::Bash => bash(),
        CompletionShell::Zsh => zsh(),
        CompletionShell::Fish => fish(),
    }
}

fn bash() -> String {
    let families: Vec<&str> = FAMILY_VERBS.iter().map(|(family, _)| *family).collect();
    format!(
        concat!(
            "# bash completion for orchbee — static grammar, no dynamic state.\n",
            "_orchbee() {{\n",
            "    local cur prev\n",
            "    cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n",
            "    prev=\"${{COMP_WORDS[COMP_CWORD-1]:-}}\"\n",
            "    case \"$prev\" in\n",
            "        --format) COMPREPLY=( $(compgen -W \"text json\" -- \"$cur\") ); return ;;\n",
            "        --project|--workspace|--tab) COMPREPLY=(); return ;;\n",
            "        completion) COMPREPLY=( $(compgen -W \"{shells}\" -- \"$cur\") ); return ;;\n",
            "        help) COMPREPLY=( $(compgen -W \"{help}\" -- \"$cur\") ); return ;;\n",
            "    esac\n",
            "    if [[ $COMP_CWORD -eq 1 ]]; then\n",
            "        COMPREPLY=( $(compgen -W \"{roots} {flags}\" -- \"$cur\") ); return\n",
            "    fi\n",
            "    case \"${{COMP_WORDS[1]}}\" in\n",
            "{cases}",
            "        *) COMPREPLY=() ;;\n",
            "    esac\n",
            "}}\n",
            "complete -F _orchbee orchbee\n"
        ),
        shells = SHELLS.join(" "),
        help = HELP_COMMANDS
            .iter()
            .map(|path| path.split_whitespace().next().expect("nonempty"))
            .chain(families.iter().copied())
            .collect::<Vec<_>>()
            .join(" "),
        roots = ROOT_WORDS.join(" "),
        flags = ROOT_FLAGS.join(" "),
        cases = FAMILY_VERBS
            .iter()
            .map(|(family, verbs)| {
                format!(
                    "        {family}) COMPREPLY=( $(compgen -W \"{} --format\" -- \"$cur\") ) ;;\n",
                    verbs.join(" ")
                )
            })
            .collect::<String>(),
    )
}

fn zsh() -> String {
    let mut script = String::from(concat!(
        "#compdef orchbee\n",
        "# zsh completion for orchbee — static grammar, no dynamic state.\n",
        "_orchbee() {\n",
        "    local -a words\n",
        "    case $CURRENT in\n",
        "        2)\n",
        "            words=(",
    ));
    for word in ROOT_WORDS.iter().chain(ROOT_FLAGS.iter()) {
        script.push_str(&format!("'{word}' "));
    }
    script.push_str(")\n            _describe 'command' words\n            ;;\n        *)\n");
    for (family, verbs) in FAMILY_VERBS {
        script.push_str(&format!(
            "            if [[ ${{words[1]}} == {family} ]]; then\n                compadd -- {} --format\n                return\n            fi\n",
            verbs.join(" ")
        ));
    }
    script.push_str("            if [[ ${words[1]} == help ]]; then\n                compadd --");
    for (family, _) in FAMILY_VERBS {
        script.push_str(&format!(" {family}"));
    }
    for command in HELP_COMMANDS {
        script.push_str(&format!(
            " '{}'",
            command.split_whitespace().next().expect("nonempty")
        ));
    }
    script.push_str(
        "\n                return\n            fi\n            if [[ ${words[1]} == completion ]]; then\n                compadd --",
    );
    for shell in SHELLS {
        script.push_str(&format!(" {shell}"));
    }
    script.push_str(
        "\n                return\n            fi\n            if [[ $prev == --format ]]; then\n                compadd -- text json\n                return\n            fi\n            ;;\n    esac\n}\n_orchbee \"$@\"\n",
    );
    script
}

fn fish() -> String {
    let mut script =
        String::from("# fish completion for orchbee — static grammar, no dynamic state.\n");
    for word in ROOT_WORDS {
        script.push_str(&format!(
            "complete -c orchbee -n '__fish_use_subcommand' -f -a '{word}'\n"
        ));
    }
    for flag in ROOT_FLAGS {
        script.push_str(&format!("complete -c orchbee -f -a '{flag}'\n"));
    }
    for (family, verbs) in FAMILY_VERBS {
        for verb in *verbs {
            script.push_str(&format!(
                "complete -c orchbee -n '__fish_seen_subcommand_from {family}' -f -a '{verb}'\n"
            ));
        }
    }
    script.push_str(
        "complete -c orchbee -n '__fish_seen_subcommand_from status project workspace tab panel terminal api config' -f -a '--format' -r\n",
    );
    script.push_str(
        "complete -c orchbee -n '__fish_seen_subcommand_from --format' -f -a 'text json'\n",
    );
    for (family, _) in FAMILY_VERBS {
        script.push_str(&format!(
            "complete -c orchbee -n '__fish_seen_subcommand_from help' -f -a '{family}'\n"
        ));
    }
    for command in HELP_COMMANDS {
        script.push_str(&format!(
            "complete -c orchbee -n '__fish_seen_subcommand_from help' -f -a '{}'\n",
            command.split_whitespace().next().expect("nonempty")
        ));
    }
    for shell in SHELLS {
        script.push_str(&format!(
            "complete -c orchbee -n '__fish_seen_subcommand_from completion' -f -a '{shell}'\n"
        ));
    }
    script
}

#[cfg(test)]
mod tests {
    use super::{FAMILY_VERBS, HELP_COMMANDS, ROOT_WORDS, render};
    use crate::invocation::CompletionShell;

    #[test]
    fn scripts_cover_the_implemented_surface() {
        for shell in [
            CompletionShell::Bash,
            CompletionShell::Zsh,
            CompletionShell::Fish,
        ] {
            let script = render(shell);
            for word in ROOT_WORDS {
                assert!(script.contains(word), "{shell:?} lacks {word}");
            }
            for (family, verbs) in FAMILY_VERBS {
                for verb in *verbs {
                    assert!(script.contains(verb), "{shell:?} lacks {family} {verb}");
                }
            }
            assert!(script.contains("--format"));
            assert!(script.contains("text json"));
            assert!(script.contains("bash"));
            assert!(script.contains("zsh"));
            assert!(script.contains("fish"));
        }
        // Every help path begins with an implemented family or command word.
        for command in HELP_COMMANDS {
            let first = command.split_whitespace().next().expect("nonempty");
            assert!(ROOT_WORDS.contains(&first), "{command}");
        }
    }
}
