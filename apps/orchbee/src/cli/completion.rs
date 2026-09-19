//! Static completion scripts for the implemented grammar. All three shells
//! run the same state machine — a faithful projection of `invocation::scan`/
//! `resolve` — and emit only the candidates the parser could still accept.
//! The scripts are emitted to stdout only; nothing is installed, no user
//! files are read or written, and no resource or Site state is ever
//! enumerated.
//!
//! Walker state (per completed word):
//! - `state` — positional position: verb | sub | after | operand | help |
//!   completion
//! - `done` — the argv can no longer produce a legal request (invalid or
//!   terminal): no candidates
//! - `ended` — `--` was seen: option termination, and `--` is never consumed
//!   as a flag value (checked before value ownership)
//! - `value` — a declared flag owns the next word: `format` | `direction` |
//!   `filter` (every other value-owning flag)
//! - `seen_*` — one consumed bit per `--format` and per business flag;
//!   duplicates are usage
//!
//! Walk precedence per completed word (mirrors `format_envelope` + `scan`):
//! 1. `--` → ended, always (the parser never takes it as a value)
//! 2. done | ended → ignore (nothing more is legal)
//! 3. value set → the word is the flag's value; a `--format` value outside
//!    {text,json} makes the parse fail
//! 4. `--format*` → legal only where options may still appear
//!    (verb/sub/after/operand, leaf `status`); a second selector or an
//!    invalid value is usage
//! 5. business flags → legal iff a command reachable here admits the flag:
//!    verb records it (checked when the command lands), sub requires the
//!    flag's owning family, after/operand require membership in the resolved
//!    command's flag scope (`--workspace`/`--standalone` stay exclusive);
//!    operand-taking paths consume one positional first; leaf/help/
//!    completion reject all options
//! 6. `--help`/`--version` → terminal flags: nothing may follow → done
//! 7. unknown `-x` → usage → done
//! 8. other word → positional dispatch:
//!    - verb: family→sub | `status`→leaf (fails if a business flag was seen) |
//!      `tui`→leaf (fails if any option was seen) | `help`→help /
//!      `completion`→completion (both fail if any option was seen)
//!    - sub: family:verb → after; the verb's declared flag scope must cover
//!      everything already seen
//!    - after: operand-taking paths → operand; anything else → done
//!    - operand/leaf/completion → done
//!    - help: HELP_TOPICS / HELP_CHILDREN walk
//!
//! Candidate precedence at the cursor (same order as the parser's legality):
//! - dead consumed prefix → nothing: when the already-typed words leave no
//!   legal root word (verb) or no legal verb (sub), every candidate is
//!   suppressed before the pending-value, `--format=` override, and offer
//!   arms below — `--workspace x --standalone` and `--project x terminal`
//!   complete nothing
//! - done | ended → nothing
//! - value == format → text json
//! - value == direction → the resolved command's direction words
//! - value == filter → nothing (no resource enumeration)
//! - cur == `--format=*` → `--format=text`/`--format=json` only where a
//!   format option is legal and none was consumed — never after
//!   done/ended/invalid/help/completion/tui or inside a pending flag value
//! - verb → root words still viable under the seen business flags + format
//!   flags + `--help`/`--version` only when nothing was consumed
//! - sub → verbs whose flag scope covers the seen flags + format flags
//! - leaf → `status`: format flags + `--help` when nothing was seen; `tui`:
//!   `--help` (options already fail)
//! - after → format flags + the command's unused business flags + `--help`
//!   on every command path with no option seen; the operand-taking `get`/
//!   `api schema`/mutation paths take a legal suffix help before their
//!   operand
//! - operand → format flags + the command's unused business flags
//! - help → help topics / children
//! - completion → shells + `--help`

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

/// Root words that still parse when a `--format` selection was consumed:
/// every data command family accepts it; `help`/`completion`/`tui`/`--version`
/// reject it.
const FORMAT_ROOT_WORDS: &[&str] = &[
    "api",
    "config",
    "panel",
    "project",
    "status",
    "tab",
    "terminal",
    "workspace",
];

/// family → the verbs it implements.
const FAMILY_VERBS: &[(&str, &[&str])] = &[
    ("api", &["schema", "snapshot"]),
    ("config", &["check"]),
    ("panel", &["close", "get", "list", "move", "split"]),
    ("project", &["create", "get", "list"]),
    ("tab", &["close", "create", "get", "list", "move", "rename"]),
    ("terminal", &["get", "list", "read"]),
    ("workspace", &["create", "get", "list", "rename"]),
];

/// Every business flag the grammar admits (`--format` is orthogonal), in the
/// order the state machine records them.
const BUSINESS_FLAGS: &[&str] = &[
    "--project",
    "--workspace",
    "--tab",
    "--standalone",
    "--label",
    "--direction",
    "--cols",
    "--rows",
    "--cell-width",
    "--cell-height",
    "--confirm",
    "--target-panel",
    "--history",
];

/// `<family>:<verb>` → the business flags that command admits. Commands not
/// listed here admit none.
const COMMAND_FLAGS: &[(&str, &[&str])] = &[
    ("project:create", &["--label"]),
    ("workspace:list", &["--project"]),
    ("workspace:create", &["--project", "--label"]),
    ("workspace:rename", &["--label"]),
    ("tab:list", &["--workspace", "--standalone"]),
    (
        "tab:create",
        &[
            "--workspace",
            "--standalone",
            "--label",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
        ],
    ),
    ("tab:rename", &["--label"]),
    ("tab:move", &["--direction"]),
    ("tab:close", &["--confirm"]),
    ("panel:list", &["--tab"]),
    (
        "panel:split",
        &[
            "--direction",
            "--label",
            "--cols",
            "--rows",
            "--cell-width",
            "--cell-height",
        ],
    ),
    ("panel:close", &["--confirm"]),
    ("panel:move", &["--target-panel", "--direction"]),
    ("terminal:read", &["--history", "--rows"]),
];

/// `<family>:<verb>` → the words its `--direction` flag accepts. Only
/// direction-admitting commands appear.
const DIRECTION_VALUES: &[(&str, &[&str])] = &[
    ("tab:move", &["first", "last", "next", "previous"]),
    ("panel:split", &["above", "below", "left", "right"]),
    ("panel:move", &["above", "below", "left", "right"]),
];

/// `<family>:<verb>` paths that consume one positional operand.
const OPERAND_PATHS: &[&str] = &[
    "api:schema",
    "panel:close",
    "panel:get",
    "panel:move",
    "panel:split",
    "project:get",
    "tab:close",
    "tab:get",
    "tab:move",
    "tab:rename",
    "terminal:get",
    "terminal:read",
    "workspace:get",
    "workspace:rename",
];

/// Help topics that lead anywhere (`help <word>`); leaf topics accept no
/// deeper word.
const HELP_TOPICS: &[&str] = &[
    "api",
    "completion",
    "config",
    "panel",
    "project",
    "status",
    "tab",
    "terminal",
    "tui",
    "workspace",
];

/// `help <topic>` → the deeper words that complete a help path.
const HELP_CHILDREN: &[(&str, &[&str])] = &[
    ("api", &["schema", "snapshot"]),
    ("config", &["check"]),
    ("panel", &["close", "get", "list", "move", "split"]),
    ("project", &["create", "get", "list"]),
    ("tab", &["close", "create", "get", "list", "move", "rename"]),
    ("terminal", &["get", "list", "read"]),
    ("workspace", &["create", "get", "list", "rename"]),
];

const SHELLS: &[&str] = &["bash", "fish", "zsh"];
const FORMAT_VALUES: &[&str] = &["text", "json"];

pub(crate) fn render(shell: CompletionShell) -> String {
    match shell {
        CompletionShell::Bash => bash(),
        CompletionShell::Zsh => zsh(),
        CompletionShell::Fish => fish(),
    }
}

/// `flag` → the walker's one-shot seen bit for it.
fn flag_bit(flag: &str) -> &'static str {
    match flag {
        "--project" => "seen_project",
        "--workspace" => "seen_workspace",
        "--tab" => "seen_tab",
        "--standalone" => "seen_standalone",
        "--label" => "seen_label",
        "--direction" => "seen_direction",
        "--cols" => "seen_cols",
        "--rows" => "seen_rows",
        "--cell-width" => "seen_cell_width",
        "--cell-height" => "seen_cell_height",
        "--confirm" => "seen_confirm",
        "--target-panel" => "seen_target_panel",
        "--history" => "seen_history",
        other => unreachable!("flag {other} lacks a seen bit"),
    }
}

/// `flag` → the value kind it leaves pending: `direction` candidates are
/// per-command; every other owned value stays opaque (`filter`).
fn flag_value(flag: &str) -> Option<&'static str> {
    match flag {
        "--direction" => Some("direction"),
        "--standalone" | "--confirm" | "--history" => None,
        _ => Some("filter"),
    }
}

/// `<family>:<verb>` → the business flags it admits (`--format` excluded).
fn command_flags(command: &str) -> &'static [&'static str] {
    COMMAND_FLAGS
        .iter()
        .find(|(path, _)| *path == command)
        .map(|(_, flags)| *flags)
        .unwrap_or(&[])
}

/// `flag` → the families owning it, in `FAMILY_VERBS` order.
fn families_of(flag: &str) -> Vec<&'static str> {
    FAMILY_VERBS
        .iter()
        .filter(|(family, verbs)| {
            verbs
                .iter()
                .any(|verb| command_flags(&format!("{family}:{verb}")).contains(&flag))
        })
        .map(|(family, _)| *family)
        .collect()
}

/// The flags a resolved command does NOT admit, as walker seen-bits.
fn forbidden_bits(command: &str) -> Vec<&'static str> {
    let scope = command_flags(command);
    BUSINESS_FLAGS
        .iter()
        .filter(|flag| !scope.contains(flag))
        .map(|flag| flag_bit(flag))
        .collect()
}

/// `family:verb` pairs, one per line of a shell `case` pattern list.
fn path_patterns(families: &[(&str, &[&str])]) -> String {
    families
        .iter()
        .flat_map(|(family, verbs)| verbs.iter().map(move |verb| format!("{family}:{verb}")))
        .collect::<Vec<_>>()
        .join("|")
}

/// bash/zsh `seen` bookkeeping arms, one per business flag.
fn seen_cases_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let bit = flag_bit(flag);
            let assign = match flag_value(flag) {
                Some(kind) => format!("{bit}=1; value={kind}"),
                None => format!("{bit}=1"),
            };
            format!("                        {flag})\n                            if (({bit})); then done=1; else {assign}; fi ;;")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh sub-state admission arms: a business flag is tentatively legal
/// iff the current family owns it on at least one verb.
fn sub_flag_cases_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = families_of(flag)
                .iter()
                .map(|family| format!("{family}:{flag}"))
                .collect::<Vec<_>>()
                .join("|");
            match *flag {
                "--workspace" => {
                    format!(
                        "                            {patterns}) ((seen_standalone)) && done=1 ;;"
                    )
                }
                "--standalone" => {
                    format!(
                        "                            {patterns}) ((seen_workspace)) && done=1 ;;"
                    )
                }
                _ => format!("                            {patterns}) ;;"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh after/operand admission arms: a business flag is legal iff the
/// resolved command's scope contains it.
fn after_flag_cases_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = COMMAND_FLAGS
                .iter()
                .filter(|(_, flags)| flags.contains(flag))
                .map(|(command, _)| format!("{command}:{flag}"))
                .collect::<Vec<_>>()
                .join("|");
            match *flag {
                "--workspace" => {
                    format!(
                        "                            {patterns}) ((seen_standalone)) && done=1 ;;"
                    )
                }
                "--standalone" => {
                    format!(
                        "                            {patterns}) ((seen_workspace)) && done=1 ;;"
                    )
                }
                _ => format!("                            {patterns}) ;;"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh verb-landing arms: the chosen command's scope must cover every
/// business flag already recorded.
fn scope_cases_bash() -> String {
    FAMILY_VERBS
        .iter()
        .flat_map(|(family, verbs)| {
            verbs.iter().map(move |verb| {
                let command = format!("{family}:{verb}");
                let bad = forbidden_bits(&command).join(" || ");
                // `--workspace`/`--standalone` stay exclusive wherever both
                // are admitted — only tab commands admit them.
                let exclusive = if *family == "tab" {
                    "((seen_workspace && seen_standalone)) && done=1\n                                        "
                } else {
                    ""
                };
                format!("                                    {command})\n                                        {exclusive}(({bad})) && done=1 ;;")
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh sub-state offers: the verbs still viable under the flags
/// already consumed.
fn verb_cases_bash() -> String {
    FAMILY_VERBS
        .iter()
        .map(|(family, verbs)| {
            let lines = verbs
                .iter()
                .enumerate()
                .map(|(index, verb)| {
                    let bad = forbidden_bits(&format!("{family}:{verb}")).join(" || ");
                    let exclusive = if *family == "tab" {
                        " || (seen_workspace && seen_standalone)"
                    } else {
                        ""
                    };
                    let assign = if index == 0 {
                        format!("verbs=\" {verb}\"")
                    } else {
                        format!("verbs=\"$verbs {verb}\"")
                    };
                    format!("                (({bad}{exclusive})) || {assign}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("            {family})\n{lines} ;;")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh verb-state narrowing: each consumed business flag intersects the
/// still-viable root words with the families that declare it.
fn narrow_lines_bash() -> String {
    BUSINESS_FLAGS
        .iter()
        .filter_map(|flag| {
            let keep = families_of(flag).join(" ");
            match *flag {
                "--workspace" => Some(format!(
                    "        if ((seen_workspace || seen_standalone)); then\n            if ((seen_workspace && seen_standalone)); then possible=\"\"\n            else narrow \"{keep}\"; fi\n        fi"
                )),
                // Folded into the --workspace arm: the exclusive pair shares
                // one family set.
                "--standalone" => None,
                _ => Some(format!("        (({})) && narrow \"{keep}\"", flag_bit(flag))),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh after/operand offers: the resolved command's unused business
/// flags. An exclusive flag is offered only while its pair is also unseen.
fn extras_cases_bash() -> String {
    COMMAND_FLAGS
        .iter()
        .map(|(command, flags)| {
            let lines = flags
                .iter()
                .map(|flag| {
                    let cond = match *flag {
                        "--workspace" | "--standalone" => {
                            "seen_workspace || seen_standalone".to_owned()
                        }
                        _ => flag_bit(flag).to_owned(),
                    };
                    format!("                (({cond})) || extras=\"$extras {flag}\"")
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("            {command})\n{lines} ;;")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// bash/zsh direction-value offers while a `--direction` value is pending.
fn direction_cases_bash() -> String {
    DIRECTION_VALUES
        .iter()
        .map(|(command, values)| {
            format!(
                "            {command}) candidates=\"{}\" ;;",
                values.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The bash/zsh "no option was consumed" condition that keeps `--help`
/// offers legal.
fn no_options_bash() -> String {
    let mut bits = vec!["seen_format"];
    bits.extend(BUSINESS_FLAGS.iter().map(|flag| flag_bit(flag)));
    bits.iter()
        .map(|bit| format!("!{bit}"))
        .collect::<Vec<_>>()
        .join(" && ")
}

fn bash() -> String {
    format!(
        concat!(
            "# bash completion for orchbee — a static projection of the implemented grammar.\n",
            "# No dynamic state: no Site, resource, or runtime enumeration.\n",
            "_orchbee() {{\n",
            "    local cur candidates=\"\" extras=\"\" verbs=\"\" possible=\"\"\n",
            "    local state=verb help_a=\"\" help_b=\"\" family=\"\" sub=\"\" leaf=\"\"\n",
            "    local seen_format=0 {seen_decls}\n",
            "    local ended=0 done=0 dead=0 value=\"\"\n",
            "    local i word\n",
            "    # Each consumed business flag narrows the viable root words to the\n",
            "    # families that declare it.\n",
            "    narrow() {{\n",
            "        local keep=\" $1 \" word out=\"\"\n",
            "        for word in $possible; do\n",
            "            [[ $keep == *\" $word \"* ]] && out=\"$out $word\"\n",
            "        done\n",
            "        possible=\"${{out# }}\"\n",
            "    }}\n",
            "    cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n",
            "    for ((i = 1; i < COMP_CWORD; i++)); do\n",
            "        word=\"${{COMP_WORDS[i]}}\"\n",
            "        # `--` terminates options and is never consumed as a flag value.\n",
            "        if [[ $word == -- ]]; then ended=1; continue; fi\n",
            "        if ((ended || done)); then continue; fi\n",
            "        # A declared flag owns the next word whole — even one spelled like a flag.\n",
            "        if [[ -n $value ]]; then\n",
            "            if [[ $value == format && $word != text && $word != json ]]; then done=1; fi\n",
            "            value=\"\"\n",
            "            continue\n",
            "        fi\n",
            "        case \"$word\" in\n",
            "            --format|--format=*)\n",
            "                case \"$state\" in\n",
            "                    verb|sub|after|operand) ;;\n",
            "                    leaf) [[ $leaf == status ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "                        --format)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; value=format; fi ;;\n",
            "                        --format=text|--format=json)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; fi ;;\n",
            "                        *) done=1 ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            {flag_patterns})\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "{sub_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after|operand)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "{after_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "{seen_cases}\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --help|--version) done=1 ;;\n",
            "            -*) done=1 ;;\n",
            "            *)\n",
            "                case \"$state\" in\n",
            "                    verb)\n",
            "                        case \"$word\" in\n",
            "                            status)\n",
            "                                if (({business_bits})); then done=1; else leaf=status; state=leaf; fi ;;\n",
            "                            tui|help|completion)\n",
            "                                if ((seen_format || {business_bits})); then done=1; else\n",
            "                                    [[ $word == tui ]] && leaf=tui && state=leaf || state=$word\n",
            "                                fi ;;\n",
            "                            {families}) family=$word; state=sub ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            {paths})\n",
            "                                sub=$word; state=after\n",
            "                                # The chosen verb's flag scope must cover\n",
            "                                # every flag already recorded.\n",
            "                                case \"$family:$sub\" in\n",
            "{scope_cases}\n",
            "                                esac ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub\" in\n",
            "                            {operand_paths}) state=operand ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    leaf|operand|completion) done=1 ;;\n",
            "                    help)\n",
            "                        if [[ -z $help_a ]]; then\n",
            "                            case \"$word\" in\n",
            "                                {topics}) help_a=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        elif [[ -z $help_b ]]; then\n",
            "                            case \"$help_a:$word\" in\n",
            "                                {help_paths}) help_b=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        else done=1; fi ;;\n",
            "                esac ;;\n",
            "        esac\n",
            "    done\n",
            "    # A consumed prefix that left no legal root word (verb) or no legal\n",
            "    # verb (sub) is dead: every candidate is suppressed before pending\n",
            "    # values, the --format= override, or any offer can resurrect one.\n",
            "    if [[ $state == verb ]]; then\n",
            "        possible=\"{roots}\"\n",
            "{narrow_lines}\n",
            "        if ((seen_format)) && [[ $possible == *help* ]]; then\n",
            "            possible=\"{format_roots}\"\n",
            "        fi\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "{verb_cases}\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == direction ]]; then\n",
            "        case \"$family:$sub\" in\n",
            "{direction_cases}\n",
            "        esac\n",
            "    elif [[ -n $value ]]; then candidates=\"\"\n",
            "    elif [[ $cur == --format=* ]]; then\n",
            "        if ((!seen_format)); then\n",
            "            case \"$state\" in\n",
            "                verb|sub|after|operand) candidates=\"--format=text --format=json\" ;;\n",
            "                leaf) [[ $leaf == status ]] && candidates=\"--format=text --format=json\" ;;\n",
            "            esac\n",
            "        fi\n",
            "    elif [[ $state == verb ]]; then\n",
            "        candidates=\"$possible\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "        if (({no_options})); then\n",
            "            candidates=\"$candidates --help --version\"\n",
            "        fi\n",
            "    elif [[ $state == sub ]]; then\n",
            "        candidates=\"$verbs\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "    elif [[ $state == leaf ]]; then\n",
            "        if [[ $leaf == tui ]]; then candidates=\"--help\"\n",
            "        elif ((!seen_format)); then candidates=\"--format --format= --help\"; fi\n",
            "    elif [[ $state == after ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
            "        case \"$family:$sub\" in\n",
            "{extras_cases}\n",
            "        esac\n",
            "        # Suffix `--help` is legal on every command path — including the\n",
            "        # operand-owed `get`/`api schema`/mutation paths — while no\n",
            "        # option was seen.\n",
            "        if (({no_options})); then\n",
            "            candidates=\"$candidates --help\"\n",
            "        fi\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == operand ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
            "        case \"$family:$sub\" in\n",
            "{extras_cases}\n",
            "        esac\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == completion ]]; then candidates=\"{shells} --help\"\n",
            "    elif [[ $state == help ]]; then\n",
            "        if [[ -z $help_a ]]; then candidates=\"{topic_words}\"\n",
            "        elif [[ -z $help_b ]]; then\n",
            "            case \"$help_a\" in\n",
            "{help_cases}\n",
            "            esac\n",
            "        fi\n",
            "    fi\n",
            "    COMPREPLY=()\n",
            "    # shellcheck disable=SC2207\n",
            "    if [[ -n $candidates ]]; then COMPREPLY=($(compgen -W \"$candidates\" -- \"$cur\")); fi\n",
            "}}\n",
            "complete -F _orchbee orchbee\n"
        ),
        families = FAMILY_VERBS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>()
            .join("|"),
        paths = path_patterns(FAMILY_VERBS),
        operand_paths = OPERAND_PATHS.join("|"),
        topics = HELP_TOPICS.join("|"),
        topic_words = HELP_TOPICS.join(" "),
        help_paths = path_patterns(HELP_CHILDREN),
        format_values = FORMAT_VALUES.join(" "),
        roots = ROOT_WORDS.join(" "),
        format_roots = FORMAT_ROOT_WORDS.join(" "),
        shells = SHELLS.join(" "),
        flag_patterns = BUSINESS_FLAGS.join("|"),
        seen_decls = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("{}=0", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join(" "),
        business_bits = BUSINESS_FLAGS
            .iter()
            .map(|flag| flag_bit(flag))
            .collect::<Vec<_>>()
            .join(" || "),
        sub_flag_cases = sub_flag_cases_bash(),
        after_flag_cases = after_flag_cases_bash(),
        seen_cases = seen_cases_bash(),
        scope_cases = scope_cases_bash(),
        verb_cases = verb_cases_bash(),
        narrow_lines = narrow_lines_bash(),
        extras_cases = extras_cases_bash(),
        direction_cases = direction_cases_bash(),
        no_options = no_options_bash(),
        help_cases = HELP_CHILDREN
            .iter()
            .map(|(topic, children)| format!(
                "            {topic}) candidates=\"{}\" ;;",
                children.join(" ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn zsh() -> String {
    format!(
        concat!(
            "#compdef orchbee\n",
            "# zsh completion for orchbee — a static projection of the implemented\n",
            "# grammar. No dynamic state: no Site, resource, or runtime enumeration.\n",
            "_orchbee() {{\n",
            "    local candidates=\"\" extras=\"\" verbs=\"\" possible=\"\"\n",
            "    local state=verb help_a=\"\" help_b=\"\" family=\"\" sub=\"\" leaf=\"\"\n",
            "    local seen_format=0 {seen_decls}\n",
            "    local ended=0 done=0 dead=0 value=\"\"\n",
            "    local i word cur\n",
            "    narrow() {{\n",
            "        local keep=\" $1 \" word out=\"\"\n",
            "        for word in ${{=possible}}; do\n",
            "            [[ $keep == *\" $word \"* ]] && out=\"$out $word\"\n",
            "        done\n",
            "        possible=\"${{out# }}\"\n",
            "    }}\n",
            "    cur=\"${{words[CURRENT]}}\"\n",
            "    for ((i = 2; i < CURRENT; i++)); do\n",
            "        word=\"${{words[i]}}\"\n",
            "        if [[ $word == -- ]]; then ended=1; continue; fi\n",
            "        if ((ended || done)); then continue; fi\n",
            "        if [[ -n $value ]]; then\n",
            "            if [[ $value == format && $word != text && $word != json ]]; then done=1; fi\n",
            "            value=\"\"\n",
            "            continue\n",
            "        fi\n",
            "        case \"$word\" in\n",
            "            --format|--format=*)\n",
            "                case \"$state\" in\n",
            "                    verb|sub|after|operand) ;;\n",
            "                    leaf) [[ $leaf == status ]] || done=1 ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "                        --format)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; value=format; fi ;;\n",
            "                        --format=text|--format=json)\n",
            "                            if ((seen_format)); then done=1; else seen_format=1; fi ;;\n",
            "                        *) done=1 ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            {flag_patterns})\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "{sub_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after|operand)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "{after_flag_cases}\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "{seen_cases}\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --help|--version) done=1 ;;\n",
            "            -*) done=1 ;;\n",
            "            *)\n",
            "                case \"$state\" in\n",
            "                    verb)\n",
            "                        case \"$word\" in\n",
            "                            status)\n",
            "                                if (({business_bits})); then done=1; else leaf=status; state=leaf; fi ;;\n",
            "                            tui|help|completion)\n",
            "                                if ((seen_format || {business_bits})); then done=1; else\n",
            "                                    [[ $word == tui ]] && leaf=tui && state=leaf || state=$word\n",
            "                                fi ;;\n",
            "                            {families}) family=$word; state=sub ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            {paths})\n",
            "                                sub=$word; state=after\n",
            "                                case \"$family:$sub\" in\n",
            "{scope_cases}\n",
            "                                esac ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub\" in\n",
            "                            {operand_paths}) state=operand ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    leaf|operand|completion) done=1 ;;\n",
            "                    help)\n",
            "                        if [[ -z $help_a ]]; then\n",
            "                            case \"$word\" in\n",
            "                                {topics}) help_a=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        elif [[ -z $help_b ]]; then\n",
            "                            case \"$help_a:$word\" in\n",
            "                                {help_paths}) help_b=$word ;;\n",
            "                                *) done=1 ;;\n",
            "                            esac\n",
            "                        else done=1; fi ;;\n",
            "                esac ;;\n",
            "        esac\n",
            "    done\n",
            "    if [[ $state == verb ]]; then\n",
            "        possible=\"{roots}\"\n",
            "{narrow_lines}\n",
            "        if ((seen_format)) && [[ $possible == *help* ]]; then\n",
            "            possible=\"{format_roots}\"\n",
            "        fi\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "{verb_cases}\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == direction ]]; then\n",
            "        case \"$family:$sub\" in\n",
            "{direction_cases}\n",
            "        esac\n",
            "    elif [[ -n $value ]]; then candidates=\"\"\n",
            "    elif [[ $cur == --format=* ]]; then\n",
            "        if ((!seen_format)); then\n",
            "            case \"$state\" in\n",
            "                verb|sub|after|operand) candidates=\"--format=text --format=json\" ;;\n",
            "                leaf) [[ $leaf == status ]] && candidates=\"--format=text --format=json\" ;;\n",
            "            esac\n",
            "        fi\n",
            "    elif [[ $state == verb ]]; then\n",
            "        candidates=\"$possible\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "        if (({no_options})); then\n",
            "            candidates=\"$candidates --help --version\"\n",
            "        fi\n",
            "    elif [[ $state == sub ]]; then\n",
            "        candidates=\"$verbs\"\n",
            "        ((seen_format)) || candidates=\"$candidates --format --format=\"\n",
            "    elif [[ $state == leaf ]]; then\n",
            "        if [[ $leaf == tui ]]; then candidates=\"--help\"\n",
            "        elif ((!seen_format)); then candidates=\"--format --format= --help\"; fi\n",
            "    elif [[ $state == after ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
            "        case \"$family:$sub\" in\n",
            "{extras_cases}\n",
            "        esac\n",
            "        if (({no_options})); then\n",
            "            candidates=\"$candidates --help\"\n",
            "        fi\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == operand ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
            "        case \"$family:$sub\" in\n",
            "{extras_cases}\n",
            "        esac\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == completion ]]; then candidates=\"{shells} --help\"\n",
            "    elif [[ $state == help ]]; then\n",
            "        if [[ -z $help_a ]]; then candidates=\"{topic_words}\"\n",
            "        elif [[ -z $help_b ]]; then\n",
            "            case \"$help_a\" in\n",
            "{help_cases}\n",
            "            esac\n",
            "        fi\n",
            "    fi\n",
            "    [[ -n $candidates ]] && compadd -- ${{=candidates}}\n",
            "    return 0\n",
            "}}\n",
            "if (( ${{+functions[compdef]}} )); then compdef _orchbee orchbee; fi\n"
        ),
        families = FAMILY_VERBS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>()
            .join("|"),
        paths = path_patterns(FAMILY_VERBS),
        operand_paths = OPERAND_PATHS.join("|"),
        topics = HELP_TOPICS.join("|"),
        topic_words = HELP_TOPICS.join(" "),
        help_paths = path_patterns(HELP_CHILDREN),
        format_values = FORMAT_VALUES.join(" "),
        roots = ROOT_WORDS.join(" "),
        format_roots = FORMAT_ROOT_WORDS.join(" "),
        shells = SHELLS.join(" "),
        flag_patterns = BUSINESS_FLAGS.join("|"),
        seen_decls = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("{}=0", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join(" "),
        business_bits = BUSINESS_FLAGS
            .iter()
            .map(|flag| flag_bit(flag))
            .collect::<Vec<_>>()
            .join(" || "),
        sub_flag_cases = sub_flag_cases_bash(),
        after_flag_cases = after_flag_cases_bash(),
        seen_cases = seen_cases_bash(),
        scope_cases = scope_cases_bash(),
        verb_cases = verb_cases_bash(),
        narrow_lines = narrow_lines_bash(),
        extras_cases = extras_cases_bash(),
        direction_cases = direction_cases_bash(),
        no_options = no_options_bash(),
        help_cases = HELP_CHILDREN
            .iter()
            .map(|(topic, children)| format!(
                "            {topic}) candidates=\"{}\" ;;",
                children.join(" ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// fish `seen` bookkeeping arms, one per business flag.
fn seen_cases_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let bit = flag_bit(flag);
            let assign = match flag_value(flag) {
                Some(kind) => format!("set {bit} 1; set value {kind}"),
                None => format!("set {bit} 1"),
            };
            format!("                        case '{flag}'\n                            if test ${bit} -eq 1; set done 1; else; {assign}; end")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish sub-state admission arms.
fn sub_flag_cases_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = families_of(flag)
                .iter()
                .map(|family| format!("'{family}:{flag}'"))
                .collect::<Vec<_>>()
                .join(" ");
            match *flag {
                "--workspace" => {
                    format!("                            case {patterns}; if test $seen_standalone -eq 1; set done 1; end")
                }
                "--standalone" => {
                    format!("                            case {patterns}; if test $seen_workspace -eq 1; set done 1; end")
                }
                _ => format!("                            case {patterns}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish after/operand admission arms.
fn after_flag_cases_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .map(|flag| {
            let patterns = COMMAND_FLAGS
                .iter()
                .filter(|(_, flags)| flags.contains(flag))
                .map(|(command, _)| format!("'{command}:{flag}'"))
                .collect::<Vec<_>>()
                .join(" ");
            match *flag {
                "--workspace" => {
                    format!("                            case {patterns}; if test $seen_standalone -eq 1; set done 1; end")
                }
                "--standalone" => {
                    format!("                            case {patterns}; if test $seen_workspace -eq 1; set done 1; end")
                }
                _ => format!("                            case {patterns}"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish verb-landing scope arms.
fn scope_cases_fish() -> String {
    FAMILY_VERBS
        .iter()
        .flat_map(|(family, verbs)| {
            verbs.iter().map(move |verb| {
                let command = format!("{family}:{verb}");
                let bad = forbidden_bits(&command)
                    .iter()
                    .map(|bit| format!("${bit} -eq 1"))
                    .collect::<Vec<_>>()
                    .join(" -o ");
                let exclusive = if *family == "tab" {
                    "\n                                        if test $seen_workspace -eq 1 -a $seen_standalone -eq 1; set done 1; end"
                } else {
                    ""
                };
                format!("                                    case '{command}'\n                                        if test {bad}; set done 1; end{exclusive}")
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish sub-state verb offers.
fn verb_cases_fish() -> String {
    FAMILY_VERBS
        .iter()
        .map(|(family, verbs)| {
            let lines = verbs
                .iter()
                .map(|verb| {
                    let bad = forbidden_bits(&format!("{family}:{verb}"))
                        .iter()
                        .map(|bit| format!("${bit} -eq 0"))
                        .collect::<Vec<_>>()
                        .join(" -a ");
                    // The exclusive pair must not both be seen for a tab verb
                    // to remain viable.
                    if *family == "tab" {
                        format!("                if test {bad}\n                    if test $seen_workspace -eq 0 -o $seen_standalone -eq 0; set -a verbs {verb}; end\n                end")
                    } else {
                        format!("                if test {bad}; set -a verbs {verb}; end")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("            case {family}\n{lines}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish verb-state narrowing through the `__orchbee_narrow` helper.
fn narrow_lines_fish() -> String {
    BUSINESS_FLAGS
        .iter()
        .filter_map(|flag| {
            let keep = families_of(flag).join(" ");
            match *flag {
                "--workspace" => Some(format!(
                    "        if test $seen_workspace -eq 1 -a $seen_standalone -eq 1\n            set possible\n        else if test $seen_workspace -eq 1 -o $seen_standalone -eq 1\n            set possible (__orchbee_narrow \"{keep}\" $possible)\n        end"
                )),
                "--standalone" => None,
                _ => Some(format!(
                    "        if test ${} -eq 1; set possible (__orchbee_narrow \"{keep}\" $possible); end",
                    flag_bit(flag)
                )),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish after/operand offers: unused business flags appended to candidates.
fn extras_cases_fish() -> String {
    COMMAND_FLAGS
        .iter()
        .map(|(command, flags)| {
            let lines = flags
                .iter()
                .map(|flag| {
                    let cond = match *flag {
                        "--workspace" | "--standalone" => {
                            "$seen_workspace -eq 0 -a $seen_standalone -eq 0".to_owned()
                        }
                        _ => format!("${} -eq 0", flag_bit(flag)),
                    };
                    format!("                if test {cond}; set -a candidates {flag}; end")
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("            case '{command}'\n{lines}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// fish direction-value offers.
fn direction_cases_fish() -> String {
    DIRECTION_VALUES
        .iter()
        .map(|(command, values)| {
            format!(
                "            case '{command}'; set candidates {}",
                values.join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The fish "no option was consumed" condition.
fn no_options_fish() -> String {
    let mut bits = vec!["seen_format"];
    bits.extend(BUSINESS_FLAGS.iter().map(|flag| flag_bit(flag)));
    bits.iter()
        .map(|bit| format!("${bit} -eq 0"))
        .collect::<Vec<_>>()
        .join(" -a ")
}

fn fish() -> String {
    format!(
        concat!(
            "# fish completion for orchbee — a static projection of the implemented\n",
            "# grammar. No dynamic state: no Site, resource, or runtime enumeration.\n",
            "function __orchbee_narrow\n",
            "    for word in $argv[2..]\n",
            "        if contains -- $word (string split ' ' -- $argv[1]); echo $word; end\n",
            "    end\n",
            "end\n",
            "function __orchbee_candidates\n",
            "    set -l words (commandline -opc)\n",
            "    set -l cur (commandline -ct)\n",
            "    set -e words[1]\n",
            "    set -l state verb; set -l help_a \"\"; set -l help_b \"\"\n",
            "    set -l family \"\"; set -l sub \"\"; set -l leaf \"\"\n",
            "    set -l seen_format 0; {seen_decls}\n",
            "    set -l ended 0; set -l done 0; set -l value \"\"\n",
            "    for word in $words\n",
            "        if string match -q -- \"--\" \"$word\"; set ended 1; continue; end\n",
            "        if test $ended -eq 1 -o $done -eq 1; continue; end\n",
            "        if test -n \"$value\"\n",
            "            if test \"$value\" = format -a \"$word\" != text -a \"$word\" != json; set done 1; end\n",
            "            set value \"\"\n",
            "            continue\n",
            "        end\n",
            "        switch \"$word\"\n",
            "            case '--format' '--format=*'\n",
            "                switch \"$state\"\n",
            "                    case verb sub after operand\n",
            "                    case leaf; if test \"$leaf\" != status; set done 1; end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$word\"\n",
            "                        case '--format'\n",
            "                            if test $seen_format -eq 1; set done 1; else; set seen_format 1; set value format; end\n",
            "                        case '--format=text' '--format=json'\n",
            "                            if test $seen_format -eq 1; set done 1; else; set seen_format 1; end\n",
            "                        case '*'; set done 1\n",
            "                    end\n",
            "                end\n",
            "            case {flag_patterns}\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "{sub_flag_cases}\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case after operand\n",
            "                        switch \"$family:$sub:$word\"\n",
            "{after_flag_cases}\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$word\"\n",
            "{seen_cases}\n",
            "                    end\n",
            "                end\n",
            "            case '--help' '--version'; set done 1\n",
            "            case '-*'; set done 1\n",
            "            case '*'\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                        switch \"$word\"\n",
            "                            case status\n",
            "                                if test {business_bits}\n",
            "                                    set done 1\n",
            "                                else; set leaf status; set state leaf; end\n",
            "                            case tui help completion\n",
            "                                if test $seen_format -eq 1 -o {business_bits}\n",
            "                                    set done 1\n",
            "                                else if test \"$word\" = tui; set leaf tui; set state leaf\n",
            "                                else; set state $word; end\n",
            "                            case {families}; set family $word; set state sub\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "                            case {paths}\n",
            "                                set sub $word; set state after\n",
            "                                switch \"$family:$sub\"\n",
            "{scope_cases}\n",
            "                                end\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case after\n",
            "                        switch \"$family:$sub\"\n",
            "                            case {operand_paths}; set state operand\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case leaf operand completion; set done 1\n",
            "                    case help\n",
            "                        if test -z \"$help_a\"\n",
            "                            switch \"$word\"\n",
            "                                case {topics}; set help_a $word\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        else if test -z \"$help_b\"\n",
            "                            switch \"$help_a:$word\"\n",
            "                                case {help_paths}; set help_b $word\n",
            "                                case '*'; set done 1\n",
            "                            end\n",
            "                        else; set done 1; end\n",
            "                end\n",
            "        end\n",
            "    end\n",
            "    set -l possible; set -l verbs; set -l dead 0\n",
            "    if test \"$state\" = verb\n",
            "        set possible {roots}\n",
            "{narrow_lines}\n",
            "        if test $seen_format -eq 1\n",
            "            if contains help $possible; or contains completion $possible; or contains tui $possible\n",
            "                set possible {format_roots}\n",
            "            end\n",
            "        end\n",
            "        if test (count $possible) -eq 0; set dead 1; end\n",
            "    else if test \"$state\" = sub\n",
            "        switch \"$family\"\n",
            "{verb_cases}\n",
            "        end\n",
            "        if test (count $verbs) -eq 0; set dead 1; end\n",
            "    end\n",
            "    set -l candidates\n",
            "    if test $done -eq 1 -o $ended -eq 1 -o $dead -eq 1; set candidates\n",
            "    else if test \"$value\" = format; set candidates {format_values}\n",
            "    else if test \"$value\" = direction\n",
            "        switch \"$family:$sub\"\n",
            "{direction_cases}\n",
            "        end\n",
            "    else if test -n \"$value\"; set candidates\n",
            "    else if string match -q -- '--format=*' $cur\n",
            "        if test $seen_format -eq 0\n",
            "            switch \"$state\"\n",
            "                case verb sub after operand; set candidates --format=text --format=json\n",
            "                case leaf; if test \"$leaf\" = status; set candidates --format=text --format=json; end\n",
            "            end\n",
            "        end\n",
            "    else if test \"$state\" = verb\n",
            "        set candidates $possible\n",
            "        if test $seen_format -eq 0; set -a candidates --format --format=; end\n",
            "        if test {no_options}\n",
            "            set -a candidates --help --version\n",
            "        end\n",
            "    else if test \"$state\" = sub\n",
            "        set candidates $verbs\n",
            "        if test $seen_format -eq 0; set -a candidates --format --format=; end\n",
            "    else if test \"$state\" = leaf\n",
            "        if test \"$leaf\" = tui; set candidates --help\n",
            "        else if test $seen_format -eq 0; set candidates --format --format= --help; end\n",
            "    else if test \"$state\" = after\n",
            "        if test $seen_format -eq 0; set candidates --format --format=; end\n",
            "        switch \"$family:$sub\"\n",
            "{extras_cases}\n",
            "        end\n",
            "        if test {no_options}\n",
            "            set -a candidates --help\n",
            "        end\n",
            "    else if test $state = operand\n",
            "        if test $seen_format -eq 0; set candidates --format --format=; end\n",
            "        switch \"$family:$sub\"\n",
            "{extras_cases}\n",
            "        end\n",
            "    else if test $state = completion; set candidates {shells} --help\n",
            "    else if test $state = help\n",
            "        if test -z \"$help_a\"; set candidates {topics}\n",
            "        else if test -z \"$help_b\"\n",
            "            switch \"$help_a\"\n",
            "{help_cases}\n",
            "            end\n",
            "        end\n",
            "    end\n",
            "    printf '%s\\n' $candidates\n",
            "end\n",
            "complete -c orchbee -f -a '(__orchbee_candidates)'\n"
        ),
        families = FAMILY_VERBS
            .iter()
            .map(|(family, _)| *family)
            .collect::<Vec<_>>()
            .join(" "),
        paths = path_patterns(FAMILY_VERBS).replace('|', " "),
        operand_paths = OPERAND_PATHS.join(" "),
        topics = HELP_TOPICS.join(" "),
        help_paths = path_patterns(HELP_CHILDREN).replace('|', " "),
        format_values = FORMAT_VALUES.join(" "),
        roots = ROOT_WORDS.join(" "),
        format_roots = FORMAT_ROOT_WORDS.join(" "),
        shells = SHELLS.join(" "),
        flag_patterns = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("'{flag}'"))
            .collect::<Vec<_>>()
            .join(" "),
        seen_decls = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("set -l {} 0", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join("; "),
        business_bits = BUSINESS_FLAGS
            .iter()
            .map(|flag| format!("${} -eq 1", flag_bit(flag)))
            .collect::<Vec<_>>()
            .join(" -o "),
        sub_flag_cases = sub_flag_cases_fish(),
        after_flag_cases = after_flag_cases_fish(),
        seen_cases = seen_cases_fish(),
        scope_cases = scope_cases_fish(),
        verb_cases = verb_cases_fish(),
        narrow_lines = narrow_lines_fish(),
        extras_cases = extras_cases_fish(),
        direction_cases = direction_cases_fish(),
        no_options = no_options_fish(),
        help_cases = HELP_CHILDREN
            .iter()
            .map(|(topic, children)| format!(
                "            case {topic}; set candidates {}",
                children.join(" ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        BUSINESS_FLAGS, COMMAND_FLAGS, DIRECTION_VALUES, FAMILY_VERBS, FORMAT_VALUES,
        HELP_CHILDREN, HELP_TOPICS, OPERAND_PATHS, ROOT_WORDS, command_flags, families_of, render,
    };
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
            for flag in BUSINESS_FLAGS {
                assert!(script.contains(flag), "{shell:?} lacks {flag}");
            }
            assert!(script.contains("--format"));
            for value in FORMAT_VALUES {
                assert!(script.contains(value), "{shell:?} lacks {value}");
            }
            assert!(script.contains("bash"));
            assert!(script.contains("zsh"));
            assert!(script.contains("fish"));
            assert!(script.contains("--"), "{shell:?} lacks terminator handling");
        }
        // Every help child path begins with an implemented topic.
        for (topic, children) in HELP_CHILDREN {
            assert!(HELP_TOPICS.contains(topic), "{topic}");
            for child in *children {
                assert!(
                    FAMILY_VERBS
                        .iter()
                        .any(|(family, verbs)| family == topic && verbs.contains(child)),
                    "{topic} {child}"
                );
            }
        }
    }

    #[test]
    fn path_tables_agree_with_the_grammar() {
        // Every operand path is an implemented verb.
        for path in OPERAND_PATHS {
            let (family, verb) = path.split_once(':').expect("family:verb");
            assert!(
                FAMILY_VERBS
                    .iter()
                    .any(|(f, verbs)| *f == family && verbs.contains(&verb)),
                "{path} is not an implemented command"
            );
        }
        assert!(OPERAND_PATHS.contains(&"api:schema"));
        // Every scoped command is an implemented verb, and every scoped flag
        // is a declared business flag.
        for (command, flags) in COMMAND_FLAGS {
            let (family, verb) = command.split_once(':').expect("family:verb");
            assert!(
                FAMILY_VERBS
                    .iter()
                    .any(|(f, verbs)| *f == family && verbs.contains(&verb)),
                "{command} is not an implemented command"
            );
            for flag in *flags {
                assert!(BUSINESS_FLAGS.contains(flag), "{command} lists {flag}");
            }
        }
        // Direction values hang off commands that actually admit --direction.
        for (command, values) in DIRECTION_VALUES {
            assert!(
                command_flags(command).contains(&"--direction"),
                "{command} lacks --direction"
            );
            assert!(!values.is_empty(), "{command} lacks direction words");
        }
        // Every flag has at least one owning family.
        for flag in BUSINESS_FLAGS {
            assert!(!families_of(flag).is_empty(), "{flag} has no owner");
        }
    }
}
