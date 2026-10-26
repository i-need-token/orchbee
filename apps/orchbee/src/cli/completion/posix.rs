//! Bash and Zsh share one literal metadata walker; only their completion hooks differ.
use crate::invocation::CompletionShell;

pub(super) fn render(shell: CompletionShell) -> String {
    let mut script = String::from(
        "# Static Orchbee completion: no runtime or filesystem probing.\n__orchbee_meta() {\n  reply=()\n  case \"$1\" in\n",
    );
    for (key, values) in super::metadata::rows() {
        if values.is_empty() {
            continue;
        }
        script.push_str(&format!(
            "    {}) reply=({}) ;;\n",
            super::metadata::quote(&key),
            values
                .iter()
                .map(|value| super::metadata::quote(value))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    script.push_str("  esac\n}\n");
    script.push_str(&include_str!("posix.sh").replace(
        "__ORCHBEE_TRIM_CHARACTERS__",
        &super::metadata::quote(crate::invocation::grammar::trim_characters()),
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
  previous=("${words[@]:1:CURRENT-2}")
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
