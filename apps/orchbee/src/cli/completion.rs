//! Static completion scripts for the implemented grammar. All three shells
//! run the same state machine — a faithful projection of `invocation::scan`/
//! `resolve` — and emit only the candidates the parser could still accept.
//! The scripts are emitted to stdout only; nothing is installed, no user
//! files are read or written, and no resource or Site state is ever
//! enumerated.
//!
//! Walker state (per completed word):
//! - `state` — positional position: verb | sub | leaf | after | operand |
//!   help | completion
//! - `done` — the argv can no longer produce a legal request (invalid or
//!   terminal): no candidates
//! - `ended` — `--` was seen: option termination, and `--` is never consumed
//!   as a flag value (checked before value ownership)
//! - `value` — a declared flag owns the next word: `format` | `filter`
//! - `seen_*` — consumed `--format`/filter selectors; duplicates are usage
//!
//! Walk precedence per completed word (mirrors `format_envelope` + `scan`):
//! 1. `--` → ended, always (the parser never takes it as a value)
//! 2. done | ended → ignore (nothing more is legal)
//! 3. value set → the word is the flag's value; a `--format` value outside
//!    {text,json} makes the parse fail
//! 4. `--format*` → legal only where options may still appear
//!    (verb/sub/after/operand, leaf `status`); a second selector or an
//!    invalid value is usage
//! 5. filter flags → legal iff a command reachable here declares it: verb
//!    records it (checked when the command lands), sub requires the flag's
//!    owning family, after requires membership in the resolved list's set
//!    (`--workspace`/`--standalone` stay exclusive); operand/leaf/help/
//!    completion reject all options
//! 6. `--help`/`--version` → terminal flags: nothing may follow → done
//! 7. unknown `-x` → usage → done
//! 8. other word → positional dispatch:
//!    - verb: family→sub | `status`→leaf (fails if a filter was seen) |
//!      `tui`→leaf (fails if any option was seen) | `help`→help /
//!      `completion`→completion (both fail if any option was seen)
//!    - sub: family:verb → after; the verb's declared filters must cover
//!      everything already seen
//!    - after: operand-taking paths (api schema, * get) → operand; anything
//!      else → done
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
//! - value == filter → nothing (no resource enumeration)
//! - cur == `--format=*` → `--format=text`/`--format=json` only where a
//!   format option is legal and none was consumed — never after
//!   done/ended/invalid/help/completion/tui or inside a pending flag value
//! - verb → root words still viable under the seen filters + format flags +
//!   `--help`/`--version` only when nothing was consumed
//! - sub → verbs whose filter scope covers the seen flags + format flags
//! - leaf → `status`: format flags + `--help` when nothing was seen; `tui`:
//!   `--help` (options already fail)
//! - after → format flags + the list's unused filters + `--help` on every
//!   command path with no option seen; the operand-taking `get`/`api schema`
//!   paths take a legal suffix help before their operand
//! - operand → format flags
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
    ("panel", &["get", "list"]),
    ("project", &["get", "list"]),
    ("tab", &["get", "list"]),
    ("terminal", &["get", "list"]),
    ("workspace", &["get", "list"]),
];

/// `<family> list` → the filters that list declares.
const LIST_FILTERS: &[(&str, &[&str])] = &[
    ("workspace", &["--project"]),
    ("tab", &["--workspace", "--standalone"]),
    ("panel", &["--tab"]),
];

/// `<family>:<verb>` paths that consume one positional operand.
const OPERAND_PATHS: &[&str] = &[
    "api:schema",
    "panel:get",
    "project:get",
    "tab:get",
    "terminal:get",
    "workspace:get",
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
    ("panel", &["get", "list"]),
    ("project", &["get", "list"]),
    ("tab", &["get", "list"]),
    ("terminal", &["get", "list"]),
    ("workspace", &["get", "list"]),
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

/// `family:verb` pairs, one per line of a shell `case` pattern list.
fn path_patterns(families: &[(&str, &[&str])]) -> String {
    families
        .iter()
        .flat_map(|(family, verbs)| verbs.iter().map(move |verb| format!("{family}:{verb}")))
        .collect::<Vec<_>>()
        .join("|")
}

fn bash() -> String {
    format!(
        concat!(
            "# bash completion for orchbee — a static projection of the implemented grammar.\n",
            "# No dynamic state: no Site, resource, or runtime enumeration.\n",
            "_orchbee() {{\n",
            "    local cur candidates=\"\" extras=\"\" verbs=\"\" possible=\"\"\n",
            "    local state=verb help_a=\"\" help_b=\"\" family=\"\" sub=\"\" leaf=\"\"\n",
            "    local seen_project=0 seen_workspace=0 seen_tab=0 seen_standalone=0 seen_format=0\n",
            "    local ended=0 done=0 dead=0 value=\"\"\n",
            "    local i word\n",
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
            "            --project|--workspace|--tab|--standalone)\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            workspace:--project) ;;\n",
            "                            tab:--workspace) ((seen_standalone)) && done=1 ;;\n",
            "                            tab:--standalone) ((seen_workspace)) && done=1 ;;\n",
            "                            panel:--tab) ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "                            workspace:list:--project) ;;\n",
            "                            tab:list:--workspace) ((seen_standalone)) && done=1 ;;\n",
            "                            tab:list:--standalone) ((seen_workspace)) && done=1 ;;\n",
            "                            panel:list:--tab) ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "                        --project)\n",
            "                            if ((seen_project)); then done=1; else seen_project=1; value=filter; fi ;;\n",
            "                        --workspace)\n",
            "                            if ((seen_workspace)); then done=1; else seen_workspace=1; value=filter; fi ;;\n",
            "                        --tab)\n",
            "                            if ((seen_tab)); then done=1; else seen_tab=1; value=filter; fi ;;\n",
            "                        --standalone)\n",
            "                            if ((seen_standalone)); then done=1; else seen_standalone=1; fi ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --help|--version) done=1 ;;\n",
            "            -*) done=1 ;;\n",
            "            *)\n",
            "                case \"$state\" in\n",
            "                    verb)\n",
            "                        case \"$word\" in\n",
            "                            status)\n",
            "                                if ((seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else leaf=status; state=leaf; fi ;;\n",
            "                            tui)\n",
            "                                if ((seen_format || seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else leaf=tui; state=leaf; fi ;;\n",
            "                            help)\n",
            "                                if ((seen_format || seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else state=help; fi ;;\n",
            "                            completion)\n",
            "                                if ((seen_format || seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else state=completion; fi ;;\n",
            "                            {families}) family=$word; state=sub ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            {paths})\n",
            "                                sub=$word; state=after\n",
            "                                # The chosen verb's filter scope must cover\n",
            "                                # every filter already recorded.\n",
            "                                case \"$family:$sub\" in\n",
            "                                    workspace:list)\n",
            "                                        ((seen_workspace || seen_tab || seen_standalone)) && done=1 ;;\n",
            "                                    tab:list)\n",
            "                                        ((seen_project || seen_tab || (seen_workspace && seen_standalone))) && done=1 ;;\n",
            "                                    panel:list)\n",
            "                                        ((seen_project || seen_workspace || seen_standalone)) && done=1 ;;\n",
            "                                    *)\n",
            "                                        ((seen_project || seen_workspace || seen_tab || seen_standalone)) && done=1 ;;\n",
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
            "        ((seen_project)) && possible=\"workspace\"\n",
            "        if ((seen_workspace || seen_standalone)); then\n",
            "            if ((seen_workspace && seen_standalone)); then possible=\"\"\n",
            "            elif [[ $possible == *tab* ]]; then possible=\"tab\"\n",
            "            else possible=\"\"; fi\n",
            "        fi\n",
            "        if ((seen_tab)); then\n",
            "            if [[ $possible == *panel* ]]; then possible=\"panel\"; else possible=\"\"; fi\n",
            "        fi\n",
            "        if ((seen_format)) && [[ $possible == *help* ]]; then\n",
            "            possible=\"{format_roots}\"\n",
            "        fi\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "            workspace)\n",
            "                ((seen_workspace || seen_tab || seen_standalone)) || verbs=\" list\"\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\"$verbs get\" ;;\n",
            "            tab)\n",
            "                ((seen_project || seen_tab)) || ((seen_workspace && seen_standalone)) || verbs=\" list\"\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\"$verbs get\" ;;\n",
            "            panel)\n",
            "                ((seen_project || seen_workspace || seen_standalone)) || verbs=\" list\"\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\"$verbs get\" ;;\n",
            "            project|terminal)\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\" get list\" ;;\n",
            "            api)\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\" schema snapshot\" ;;\n",
            "            config)\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\" check\" ;;\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == filter ]]; then candidates=\"\"\n",
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
            "        if ((!seen_format && !seen_project && !seen_workspace && !seen_tab && !seen_standalone)); then\n",
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
            "{filter_cases}\n",
            "        esac\n",
            "        # Suffix `--help` is legal on every command path — including the\n",
            "        # operand-owed `get`/`api schema` paths — while no option was seen.\n",
            "        if ((!seen_format && !seen_project && !seen_workspace && !seen_tab && !seen_standalone)); then\n",
            "            candidates=\"$candidates --help\"\n",
            "        fi\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == operand ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
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
        filter_cases = LIST_FILTERS
            .iter()
            .map(|(family, filters)| {
                let seen = filters
                    .iter()
                    .map(|filter| match *filter {
                        "--project" => "seen_project",
                        "--workspace" => "seen_workspace",
                        "--tab" => "seen_tab",
                        "--standalone" => "seen_standalone",
                        other => unreachable!("filter {other} lacks a seen bit"),
                    })
                    .collect::<Vec<_>>()
                    .join(" || ");
                format!(
                    "            {family}:list) (({seen})) || extras=\"{}\" ;;",
                    filters.join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
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
            "    local seen_project=0 seen_workspace=0 seen_tab=0 seen_standalone=0 seen_format=0\n",
            "    local ended=0 done=0 dead=0 value=\"\"\n",
            "    local i word cur\n",
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
            "            --project|--workspace|--tab|--standalone)\n",
            "                case \"$state\" in\n",
            "                    verb) ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            workspace:--project) ;;\n",
            "                            tab:--workspace) ((seen_standalone)) && done=1 ;;\n",
            "                            tab:--standalone) ((seen_workspace)) && done=1 ;;\n",
            "                            panel:--tab) ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    after)\n",
            "                        case \"$family:$sub:$word\" in\n",
            "                            workspace:list:--project) ;;\n",
            "                            tab:list:--workspace) ((seen_standalone)) && done=1 ;;\n",
            "                            tab:list:--standalone) ((seen_workspace)) && done=1 ;;\n",
            "                            panel:list:--tab) ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    *) done=1 ;;\n",
            "                esac\n",
            "                if ((!done)); then\n",
            "                    case \"$word\" in\n",
            "                        --project)\n",
            "                            if ((seen_project)); then done=1; else seen_project=1; value=filter; fi ;;\n",
            "                        --workspace)\n",
            "                            if ((seen_workspace)); then done=1; else seen_workspace=1; value=filter; fi ;;\n",
            "                        --tab)\n",
            "                            if ((seen_tab)); then done=1; else seen_tab=1; value=filter; fi ;;\n",
            "                        --standalone)\n",
            "                            if ((seen_standalone)); then done=1; else seen_standalone=1; fi ;;\n",
            "                    esac\n",
            "                fi ;;\n",
            "            --help|--version) done=1 ;;\n",
            "            -*) done=1 ;;\n",
            "            *)\n",
            "                case \"$state\" in\n",
            "                    verb)\n",
            "                        case \"$word\" in\n",
            "                            status)\n",
            "                                if ((seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else leaf=status; state=leaf; fi ;;\n",
            "                            tui)\n",
            "                                if ((seen_format || seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else leaf=tui; state=leaf; fi ;;\n",
            "                            help)\n",
            "                                if ((seen_format || seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else state=help; fi ;;\n",
            "                            completion)\n",
            "                                if ((seen_format || seen_project || seen_workspace || seen_tab || seen_standalone));\n",
            "                                then done=1; else state=completion; fi ;;\n",
            "                            {families}) family=$word; state=sub ;;\n",
            "                            *) done=1 ;;\n",
            "                        esac ;;\n",
            "                    sub)\n",
            "                        case \"$family:$word\" in\n",
            "                            {paths})\n",
            "                                sub=$word; state=after\n",
            "                                case \"$family:$sub\" in\n",
            "                                    workspace:list)\n",
            "                                        ((seen_workspace || seen_tab || seen_standalone)) && done=1 ;;\n",
            "                                    tab:list)\n",
            "                                        ((seen_project || seen_tab || (seen_workspace && seen_standalone))) && done=1 ;;\n",
            "                                    panel:list)\n",
            "                                        ((seen_project || seen_workspace || seen_standalone)) && done=1 ;;\n",
            "                                    *)\n",
            "                                        ((seen_project || seen_workspace || seen_tab || seen_standalone)) && done=1 ;;\n",
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
            "        ((seen_project)) && possible=\"workspace\"\n",
            "        if ((seen_workspace || seen_standalone)); then\n",
            "            if ((seen_workspace && seen_standalone)); then possible=\"\"\n",
            "            elif [[ $possible == *tab* ]]; then possible=\"tab\"\n",
            "            else possible=\"\"; fi\n",
            "        fi\n",
            "        if ((seen_tab)); then\n",
            "            if [[ $possible == *panel* ]]; then possible=\"panel\"; else possible=\"\"; fi\n",
            "        fi\n",
            "        if ((seen_format)) && [[ $possible == *help* ]]; then\n",
            "            possible=\"{format_roots}\"\n",
            "        fi\n",
            "        [[ -z $possible ]] && dead=1\n",
            "    elif [[ $state == sub ]]; then\n",
            "        case \"$family\" in\n",
            "            workspace)\n",
            "                ((seen_workspace || seen_tab || seen_standalone)) || verbs=\" list\"\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\"$verbs get\" ;;\n",
            "            tab)\n",
            "                ((seen_project || seen_tab)) || ((seen_workspace && seen_standalone)) || verbs=\" list\"\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\"$verbs get\" ;;\n",
            "            panel)\n",
            "                ((seen_project || seen_workspace || seen_standalone)) || verbs=\" list\"\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\"$verbs get\" ;;\n",
            "            project|terminal)\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\" get list\" ;;\n",
            "            api)\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\" schema snapshot\" ;;\n",
            "            config)\n",
            "                ((seen_project || seen_workspace || seen_tab || seen_standalone)) || verbs=\" check\" ;;\n",
            "        esac\n",
            "        [[ -z $verbs ]] && dead=1\n",
            "    fi\n",
            "    if ((done || ended || dead)); then candidates=\"\"\n",
            "    elif [[ $value == format ]]; then candidates=\"{format_values}\"\n",
            "    elif [[ $value == filter ]]; then candidates=\"\"\n",
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
            "        if ((!seen_format && !seen_project && !seen_workspace && !seen_tab && !seen_standalone)); then\n",
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
            "{filter_cases}\n",
            "        esac\n",
            "        # Suffix `--help` is legal on every command path — including the\n",
            "        # operand-owed `get`/`api schema` paths — while no option was seen.\n",
            "        if ((!seen_format && !seen_project && !seen_workspace && !seen_tab && !seen_standalone)); then\n",
            "            candidates=\"$candidates --help\"\n",
            "        fi\n",
            "        [[ -n $extras ]] && candidates=\"$candidates $extras\"\n",
            "    elif [[ $state == operand ]]; then\n",
            "        ((seen_format)) || candidates=\"--format --format=\"\n",
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
        filter_cases = LIST_FILTERS
            .iter()
            .map(|(family, filters)| {
                let seen = filters
                    .iter()
                    .map(|filter| match *filter {
                        "--project" => "seen_project",
                        "--workspace" => "seen_workspace",
                        "--tab" => "seen_tab",
                        "--standalone" => "seen_standalone",
                        other => unreachable!("filter {other} lacks a seen bit"),
                    })
                    .collect::<Vec<_>>()
                    .join(" || ");
                format!(
                    "            {family}:list) (({seen})) || extras=\"{}\" ;;",
                    filters.join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
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

fn fish() -> String {
    format!(
        concat!(
            "# fish completion for orchbee — a static projection of the implemented\n",
            "# grammar. No dynamic state: no Site, resource, or runtime enumeration.\n",
            "function __orchbee_candidates\n",
            "    set -l words (commandline -opc)\n",
            "    set -l cur (commandline -ct)\n",
            "    set -e words[1]\n",
            "    set -l state verb; set -l help_a \"\"; set -l help_b \"\"\n",
            "    set -l family \"\"; set -l sub \"\"; set -l leaf \"\"\n",
            "    set -l seen_project 0; set -l seen_workspace 0; set -l seen_tab 0; set -l seen_standalone 0; set -l seen_format 0\n",
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
            "            case '--project' '--workspace' '--tab' '--standalone'\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "                            case 'workspace:--project'\n",
            "                            case 'tab:--workspace'; if test $seen_standalone -eq 1; set done 1; end\n",
            "                            case 'tab:--standalone'; if test $seen_workspace -eq 1; set done 1; end\n",
            "                            case 'panel:--tab'\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case after\n",
            "                        switch \"$family:$sub:$word\"\n",
            "                            case 'workspace:list:--project'\n",
            "                            case 'tab:list:--workspace'; if test $seen_standalone -eq 1; set done 1; end\n",
            "                            case 'tab:list:--standalone'; if test $seen_workspace -eq 1; set done 1; end\n",
            "                            case 'panel:list:--tab'\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case '*'; set done 1\n",
            "                end\n",
            "                if test $done -eq 0\n",
            "                    switch \"$word\"\n",
            "                        case '--project'\n",
            "                            if test $seen_project -eq 1; set done 1; else; set seen_project 1; set value filter; end\n",
            "                        case '--workspace'\n",
            "                            if test $seen_workspace -eq 1; set done 1; else; set seen_workspace 1; set value filter; end\n",
            "                        case '--tab'\n",
            "                            if test $seen_tab -eq 1; set done 1; else; set seen_tab 1; set value filter; end\n",
            "                        case '--standalone'\n",
            "                            if test $seen_standalone -eq 1; set done 1; else; set seen_standalone 1; end\n",
            "                    end\n",
            "                end\n",
            "            case '--help' '--version'; set done 1\n",
            "            case '-*'; set done 1\n",
            "            case '*'\n",
            "                switch \"$state\"\n",
            "                    case verb\n",
            "                        switch \"$word\"\n",
            "                            case status\n",
            "                                if test $seen_project -eq 1 -o $seen_workspace -eq 1 -o $seen_tab -eq 1 -o $seen_standalone -eq 1\n",
            "                                    set done 1\n",
            "                                else; set leaf status; set state leaf; end\n",
            "                            case tui\n",
            "                                if test $seen_format -eq 1 -o $seen_project -eq 1 -o $seen_workspace -eq 1 -o $seen_tab -eq 1 -o $seen_standalone -eq 1\n",
            "                                    set done 1\n",
            "                                else; set leaf tui; set state leaf; end\n",
            "                            case help\n",
            "                                if test $seen_format -eq 1 -o $seen_project -eq 1 -o $seen_workspace -eq 1 -o $seen_tab -eq 1 -o $seen_standalone -eq 1\n",
            "                                    set done 1\n",
            "                                else; set state help; end\n",
            "                            case completion\n",
            "                                if test $seen_format -eq 1 -o $seen_project -eq 1 -o $seen_workspace -eq 1 -o $seen_tab -eq 1 -o $seen_standalone -eq 1\n",
            "                                    set done 1\n",
            "                                else; set state completion; end\n",
            "                            case {families}; set family $word; set state sub\n",
            "                            case '*'; set done 1\n",
            "                        end\n",
            "                    case sub\n",
            "                        switch \"$family:$word\"\n",
            "                            case {paths}\n",
            "                                set sub $word; set state after\n",
            "                                switch \"$family:$sub\"\n",
            "                                    case 'workspace:list'\n",
            "                                        if test $seen_workspace -eq 1 -o $seen_tab -eq 1 -o $seen_standalone -eq 1; set done 1; end\n",
            "                                    case 'tab:list'\n",
            "                                        if test $seen_project -eq 1 -o $seen_tab -eq 1; set done 1; end\n",
            "                                        if test $seen_workspace -eq 1 -a $seen_standalone -eq 1; set done 1; end\n",
            "                                    case 'panel:list'\n",
            "                                        if test $seen_project -eq 1 -o $seen_workspace -eq 1 -o $seen_standalone -eq 1; set done 1; end\n",
            "                                    case '*'\n",
            "                                        if test $seen_project -eq 1 -o $seen_workspace -eq 1 -o $seen_tab -eq 1 -o $seen_standalone -eq 1; set done 1; end\n",
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
            "        if test $seen_project -eq 1; set possible workspace; end\n",
            "        if test $seen_workspace -eq 1 -a $seen_standalone -eq 1\n",
            "            set possible\n",
            "        else if test $seen_workspace -eq 1 -o $seen_standalone -eq 1\n",
            "            if contains tab $possible; set possible tab; else; set possible; end\n",
            "        end\n",
            "        if test $seen_tab -eq 1\n",
            "            if contains panel $possible; set possible panel; else; set possible; end\n",
            "        end\n",
            "        if test $seen_format -eq 1\n",
            "            if contains help $possible; or contains completion $possible; or contains tui $possible\n",
            "                set possible {format_roots}\n",
            "            end\n",
            "        end\n",
            "        if test (count $possible) -eq 0; set dead 1; end\n",
            "    else if test \"$state\" = sub\n",
            "        switch \"$family\"\n",
            "            case workspace\n",
            "                if test $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set -a verbs list; end\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set -a verbs get; end\n",
            "            case tab\n",
            "                if test $seen_project -eq 0 -a $seen_tab -eq 0\n",
            "                    if test $seen_workspace -eq 0 -o $seen_standalone -eq 0; set -a verbs list; end\n",
            "                end\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set -a verbs get; end\n",
            "            case panel\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_standalone -eq 0; set -a verbs list; end\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set -a verbs get; end\n",
            "            case project terminal\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set verbs get list; end\n",
            "            case api\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set verbs schema snapshot; end\n",
            "            case config\n",
            "                if test $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0; set verbs check; end\n",
            "        end\n",
            "        if test (count $verbs) -eq 0; set dead 1; end\n",
            "    end\n",
            "    set -l candidates\n",
            "    if test $done -eq 1 -o $ended -eq 1 -o $dead -eq 1; set candidates\n",
            "    else if test \"$value\" = format; set candidates {format_values}\n",
            "    else if test \"$value\" = filter; set candidates\n",
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
            "        if test $seen_format -eq 0 -a $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0\n",
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
            "{filter_cases}\n",
            "        end\n",
            "        if test $seen_format -eq 0 -a $seen_project -eq 0 -a $seen_workspace -eq 0 -a $seen_tab -eq 0 -a $seen_standalone -eq 0\n",
            "            set -a candidates --help\n",
            "        end\n",
            "    else if test $state = operand\n",
            "        if test $seen_format -eq 0; set candidates --format --format=; end\n",
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
        filter_cases = LIST_FILTERS
            .iter()
            .map(|(family, filters)| {
                let seen = filters
                    .iter()
                    .map(|filter| match *filter {
                        "--project" => "seen_project",
                        "--workspace" => "seen_workspace",
                        "--tab" => "seen_tab",
                        "--standalone" => "seen_standalone",
                        other => unreachable!("filter {other} lacks a seen bit"),
                    })
                    .map(|name| format!("${name} -eq 0"))
                    .collect::<Vec<_>>()
                    .join(" -a ");
                format!(
                    "            case {family}:list; if test {seen}; set -a candidates {}; end",
                    filters.join(" ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
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
        FAMILY_VERBS, FORMAT_VALUES, HELP_CHILDREN, HELP_TOPICS, LIST_FILTERS, OPERAND_PATHS,
        ROOT_WORDS, render,
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
            for (_family, filters) in LIST_FILTERS {
                for filter in *filters {
                    assert!(script.contains(filter), "{shell:?} lacks {filter}");
                }
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
    }
}
