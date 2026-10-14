//! Orthogonal option scopes derived from the shared command projection.

use crate::invocation::CompletionShell;

const BASH_SCOPE: &str = r#"    local completion_scope="$family:$sub" accepts_format=1
    [[ -n $leaf ]] && completion_scope=$leaf
    case "$completion_scope" in
        __ORCHBEE_NO_FORMAT_PATHS__) accepts_format=0 ;;
    esac
    if ((seen_format && !accepts_format)); then done=1; fi
    if ((seen_provider2)) && [[ $state != verb && $state != sub && $completion_scope != agent:doctor ]]; then done=1; fi
"#;
const FISH_SCOPE: &str = r#"    set -l completion_scope "$family:$sub"; set -l accepts_format 1
    if test -n "$leaf"; set completion_scope $leaf; end
    if contains -- "$completion_scope" __ORCHBEE_NO_FORMAT_PATHS__; set accepts_format 0; end
    if test $seen_format -eq 1 -a $accepts_format -eq 0; set done 1; end
    if test $seen_provider2 -eq 1; and not contains -- "$state" verb sub; and test "$completion_scope" != agent:doctor; set done 1; end
"#;

pub(super) fn expand(mut script: String, shell: CompletionShell) -> String {
    let fish = matches!(shell, CompletionShell::Fish);
    let grammar = super::grammar::build();
    let commands = grammar["commands"].as_array().expect("static commands");
    let words = |command: &serde_json::Value| {
        command["words"]
            .as_array()
            .expect("command words")
            .iter()
            .map(|word| word.as_str().expect("word").to_owned())
            .collect::<Vec<_>>()
    };
    let mouse = commands
        .iter()
        .filter(|command| {
            command["flags"]
                .as_array()
                .expect("command flags")
                .iter()
                .any(|flag| flag == "--mouse")
        })
        .map(words)
        .collect::<Vec<_>>();
    let roots = mouse
        .iter()
        .map(|words| words[0].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let families = mouse
        .iter()
        .filter(|words| words.len() > 1)
        .map(|words| words[0].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let paths = mouse
        .iter()
        .map(|words| words.join(":"))
        .collect::<Vec<_>>();
    let no_format = commands
        .iter()
        .filter(|command| command["format"] == false)
        .map(|command| words(command).join(":"))
        .collect::<Vec<_>>();
    let guard = |variable: &str, values: Vec<&str>| {
        values
            .iter()
            .map(|value| format!("{variable} == {value}"))
            .collect::<Vec<_>>()
            .join(" || ")
    };
    let possible = roots
        .iter()
        .map(|root| {
            if fish {
                format!("contains -- {root} $possible")
            } else {
                format!("\" $possible \" == *\" {root} \"*")
            }
        })
        .collect::<Vec<_>>()
        .join(if fish { "; or " } else { " || " });
    script = script.replace(
        "__ORCHBEE_OPTION_SCOPE__",
        if fish { FISH_SCOPE } else { BASH_SCOPE },
    );
    for (key, value) in [
        (
            "NO_FORMAT_PATHS",
            no_format.join(if fish { " " } else { "|" }),
        ),
        (
            "MOUSE_ROOTS",
            roots.iter().copied().collect::<Vec<_>>().join(" "),
        ),
        (
            "MOUSE_FAMILIES",
            families.iter().copied().collect::<Vec<_>>().join(" "),
        ),
        ("MOUSE_PATHS", paths.join(" ")),
        (
            "MOUSE_FAMILY_GUARD",
            guard("$family", families.iter().copied().collect()),
        ),
        (
            "MOUSE_WORD_GUARD",
            guard("$word", families.iter().copied().collect()),
        ),
        (
            "MOUSE_PATH_GUARD",
            guard("$family:$sub", paths.iter().map(String::as_str).collect()),
        ),
        ("MOUSE_POSSIBLE_GUARD", possible),
    ] {
        script = script.replace(&format!("__ORCHBEE_{key}__"), &value);
    }
    script
}
