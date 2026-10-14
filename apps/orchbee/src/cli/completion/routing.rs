//! Static runtime selectors, kept separate from command-scoped Agent targets.

const BASH: [&str; 5] = [
    r#"    local seen_local_session=0 local_session="" session_target="" seen_machine=0
    __orchbee_machine_path() {
        local prefix="$1" command allowed
        shift
        while IFS='|' read -r command allowed; do
            if [[ -z $prefix || $command == "$prefix" || $command == "$prefix":* ]]; then
                admit "$allowed" "$@" && return 0
            fi
        done <<'ORCHBEE_REMOTE_COMMANDS'
__ORCHBEE_REMOTE_COMMANDS__
ORCHBEE_REMOTE_COMMANDS
        return 1
    }
"#,
    r#"            if [[ $value == local_session ]]; then
                local_session=$word
                [[ $word =~ ^[A-Za-z0-9._-]{1,64}$ && $word != . && $word != .. ]] || done=1
            fi
            if [[ $value == machine && -z $word ]]; then done=1; fi
"#,
    r#"            --session|--session=*)
                if ((seen_local_session || seen_machine)); then done=1; else
                    seen_local_session=1
                    if [[ $word == --session ]]; then value=local_session; else
                        local_session=${word#*=}
                        [[ $local_session =~ ^[A-Za-z0-9._-]{1,64}$ && $local_session != . && $local_session != .. ]] || done=1
                    fi
                fi ;;
            --machine|--machine=*)
                if ((seen_machine || seen_local_session || seen_format || seen_mouse || ${#seen_flags[@]})) || [[ $state != verb ]]; then done=1; else
                    seen_machine=1
                    if [[ $word == --machine ]]; then value=machine
                    elif [[ -z ${word#*=} ]]; then done=1; fi
                fi ;;
"#,
    r#"    if ((seen_local_session)) && [[ $family:$sub == config:import-herdr ]]; then done=1; fi
    if ((seen_local_session)) && [[ -n $session_target && $local_session != "$session_target" ]]; then done=1; fi
    local routing_path=""
    if [[ -n $leaf ]]; then routing_path=$leaf
    elif [[ -n $sub ]]; then routing_path="$family:$sub"
    elif [[ -n $family ]]; then routing_path=$family; fi
    if ((seen_machine)) && [[ $state != help ]] && ! __orchbee_machine_path "$routing_path" "${seen_flags[@]}"; then done=1; fi
"#,
    r#"    if ((!done && !ended && !dead)) && [[ -z $value && $cur != *=* && $state != help ]]; then
        candidates="$candidates --help"
        if ((!seen_machine && !seen_local_session)) && [[ $family:$sub != config:import-herdr ]]; then candidates="$candidates --session"; fi
        if ((!seen_machine && !seen_local_session && !seen_format && !seen_mouse && !${#seen_flags[@]})) && [[ $state == verb ]]; then candidates="$candidates --machine"; fi
    fi
    local routing_candidate routing_filtered="" routing_candidate_path
    for routing_candidate in __ORCHBEE_CANDIDATE_WORDS__; do
        if ((seen_local_session)) && [[ $family == config && $state == sub && $routing_candidate == import-herdr ]]; then continue; fi
        if ((seen_machine)) && [[ $routing_candidate != -* && $state != help && -z $value ]]; then
            routing_candidate_path="$routing_path:$routing_candidate"
            [[ -z $routing_path ]] && routing_candidate_path=$routing_candidate
            case "$state" in
                verb|sub|deep|leaf) __orchbee_machine_path "$routing_candidate_path" "${seen_flags[@]}" || continue ;;
            esac
        fi
        if [[ " $routing_filtered " != *" $routing_candidate "* ]]; then routing_filtered="$routing_filtered $routing_candidate"; fi
    done
    candidates=$routing_filtered
"#,
];

const FISH: [&str; 5] = [
    r#"    set -l seen_local_session 0; set -l local_session ''; set -l session_target ''; set -l seen_machine 0
    function __orchbee_machine_path
        for entry in __ORCHBEE_REMOTE_COMMANDS__
            set -l parts (string split '|' -- $entry)
            if test -z "$argv[1]"; or test "$parts[1]" = "$argv[1]"; or string match -q -- "$argv[1]:*" "$parts[1]"
                __orchbee_admit "$parts[2]" $argv[2..]; and return 0
            end
        end
        return 1
    end
"#,
    r#"            if test "$value" = local_session
                set local_session $word
                if not string match -rq '^[A-Za-z0-9._-]{1,64}$' -- $word; or contains -- $word . ..; set done 1; end
            end
            if test "$value" = machine; and test -z "$word"; set done 1; end
"#,
    r#"            case '--session' '--session=*'
                if test $seen_local_session -eq 1 -o $seen_machine -eq 1; set done 1; else
                    set seen_local_session 1
                    if test "$word" = --session; set value local_session; else
                        set local_session (string sub -s 11 -- $word)
                        if not string match -rq '^[A-Za-z0-9._-]{1,64}$' -- $local_session; or contains -- $local_session . ..; set done 1; end
                    end
                end
            case '--machine' '--machine=*'
                if test $seen_machine -eq 1 -o $seen_local_session -eq 1 -o $seen_format -eq 1 -o $seen_mouse -eq 1; or test (count $seen_flags) -gt 0; or test "$state" != verb; set done 1; else
                    set seen_machine 1
                    if test "$word" = --machine; set value machine
                    else if test -z (string sub -s 11 -- $word); set done 1; end
                end
"#,
    r#"    if test $seen_local_session -eq 1; and test "$family:$sub" = config:import-herdr; set done 1; end
    if test $seen_local_session -eq 1; and test -n "$session_target"; and test "$local_session" != "$session_target"; set done 1; end
    set -l routing_path ''
    if test -n "$leaf"; set routing_path $leaf
    else if test -n "$sub"; set routing_path "$family:$sub"
    else if test -n "$family"; set routing_path $family; end
    if test $seen_machine -eq 1; and test "$state" != help; and not __orchbee_machine_path "$routing_path" $seen_flags; set done 1; end
"#,
    r#"    if test $done -eq 0 -a $ended -eq 0 -a $dead -eq 0; and test -z "$value"; and not string match -q -- '*=*' "$cur"; and test "$state" != help
        set -a candidates --help
        if test $seen_local_session -eq 0 -a $seen_machine -eq 0; and test "$family:$sub" != config:import-herdr; set -a candidates --session; end
        if test $seen_local_session -eq 0 -a $seen_machine -eq 0 -a $seen_format -eq 0 -a $seen_mouse -eq 0; and test (count $seen_flags) -eq 0; and test "$state" = verb; set -a candidates --machine; end
    end
    set -l routing_filtered
    for candidate in $candidates
        if test $seen_local_session -eq 1; and test "$family" = config; and test "$state" = sub; and test "$candidate" = import-herdr; continue; end
        if test $seen_machine -eq 1; and not string match -q -- '-*' "$candidate"; and test "$state" != help; and test -z "$value"
            set -l candidate_path "$routing_path:$candidate"
            if test -z "$routing_path"; set candidate_path $candidate; end
            if contains -- "$state" verb sub deep leaf
                __orchbee_machine_path "$candidate_path" $seen_flags; or continue
            end
        end
        contains -- $candidate $routing_filtered; or set -a routing_filtered $candidate
    end
    set candidates $routing_filtered
"#,
];

pub(super) fn expand(mut script: String, shell: crate::invocation::CompletionShell) -> String {
    let fish = matches!(shell, crate::invocation::CompletionShell::Fish);
    let mut parts = if fish { FISH } else { BASH };
    if !cfg!(windows) {
        parts = [
            "",
            "",
            "",
            "",
            if fish {
                "    if test $done -eq 0 -a $ended -eq 0 -a $dead -eq 0; and test -z \"$value\"; and not string match -q -- '*=*' \"$cur\"; and test \"$state\" != help; and not contains -- --help $candidates; set -a candidates --help; end\n"
            } else {
                "    if ((!done && !ended && !dead)) && [[ -z $value && $cur != *=* && $state != help && \" $candidates \" != *\" --help \"* ]]; then candidates=\"$candidates --help\"; fi\n"
            },
        ];
    }
    for (key, value) in ["DECLARATIONS", "VALUE", "PARSE", "ADMISSION", "CANDIDATES"]
        .iter()
        .zip(parts)
    {
        script = script.replace(&format!("__ORCHBEE_ROUTING_{key}__"), value);
    }
    let grammar = super::grammar::build();
    let rows = grammar["commands"]
        .as_array()
        .expect("static commands")
        .iter()
        .filter(|command| command["machine"] == true)
        .map(|command| {
            let words = command["words"]
                .as_array()
                .expect("command words")
                .iter()
                .map(|value| value.as_str().expect("word"))
                .collect::<Vec<_>>()
                .join(":");
            let flags = command["flags"]
                .as_array()
                .expect("command flags")
                .iter()
                .map(|value| value.as_str().expect("flag"))
                .collect::<Vec<_>>()
                .join(" ");
            if fish {
                format!("'{words}|{flags}'")
            } else {
                format!("{words}|{flags}")
            }
        })
        .collect::<Vec<_>>()
        .join(if fish { " " } else { "\n" });
    script
        .replace("__ORCHBEE_REMOTE_COMMANDS__", &rows)
        .replace(
            "__ORCHBEE_CANDIDATE_WORDS__",
            if matches!(shell, crate::invocation::CompletionShell::Zsh) {
                "${=candidates}"
            } else {
                "$candidates"
            },
        )
}
