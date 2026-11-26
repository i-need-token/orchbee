//! Bash and Zsh share one literal metadata walker; only their completion hooks differ.
use std::collections::BTreeMap;

use crate::invocation::CompletionShell;

pub(super) fn render(shell: CompletionShell) -> String {
    let mut script =
        String::from("# Static Orchbee completion: no runtime or filesystem probing.\n");
    // Bash copies a function's whole body on every call, and one Tab press makes thousands of
    // lookups. With every row in one function a lookup cost about a millisecond and `orchbee -`
    // took seconds; one function per kind, the text before the first `|`, keeps each copy small.
    let mut kinds = BTreeMap::<String, String>::new();
    for (key, values) in super::metadata::rows() {
        if values.is_empty() {
            continue;
        }
        let kind = key.split('|').next().unwrap_or_default().to_owned();
        kinds.entry(kind).or_default().push_str(&format!(
            "    {}) reply=({}) ;;\n",
            super::metadata::quote(&key),
            values
                .iter()
                .map(|value| super::metadata::quote(value))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    let mut dispatch = String::from("__orchbee_meta() {\n  reply=()\n  case \"${1%%[|]*}\" in\n");
    for (kind, rows) in &kinds {
        let function = format!("__orchbee_meta_{}", kind.replace('-', "_"));
        script.push_str(&format!(
            "{function}() {{\n  case \"$1\" in\n{rows}  esac\n}}\n"
        ));
        dispatch.push_str(&format!(
            "    {}) {function} \"$1\" ;;\n",
            super::metadata::quote(kind)
        ));
    }
    script.push_str(&dispatch);
    script.push_str("  esac\n}\n");
    script.push_str(&include_str!("posix.sh").replace(
        "__ORCHBEE_TRIM_CHARACTERS__",
        &quoted_characters(crate::invocation::grammar::trim_characters()),
    ));
    script.push_str(match shell {
        CompletionShell::Bash => {
            r#"
_orchbee() {
  COMPREPLY=()
  local candidate
  while IFS= read -r candidate; do COMPREPLY+=("$candidate"); done < <(
    __orchbee_candidates "${COMP_WORDS[COMP_CWORD]}" "${COMP_WORDS[@]:1:COMP_CWORD-1}"
  )
}
complete -F _orchbee orchbee
"#
        }
        CompletionShell::Zsh => {
            r#"
_orchbee() {
  local current=${words[CURRENT]}
  local -a previous candidates
  previous=("${(@)words[2,CURRENT-1]}")
  candidates=("${(@f)$(__orchbee_candidates "$current" "${previous[@]}")}")
  (( ${#candidates[@]} )) && compadd -- "${candidates[@]}"
  return 0
}
compdef _orchbee orchbee
"#
        }
        _ => unreachable!("POSIX renderer receives Bash or Zsh"),
    });
    script
}

// MSYS Bash may discard literal CR while reading a script, even inside quotes.
// Fixed three-digit UTF-8 byte escapes preserve the exact character set on
// Bash 3.2 as well; Unicode escape syntax requires a newer Bash interpreter.
// Separate quoted words let byte locales compare whole encoded characters.
fn quoted_characters(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            let escaped = character
                .to_string()
                .bytes()
                .map(|byte| format!("\\{byte:03o}"))
                .collect::<String>();
            format!("$'{escaped}'")
        })
        .collect::<Vec<_>>()
        .join(" ")
}
