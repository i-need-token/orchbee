use super::{CloseWait, PanelQueryTarget};
use super::{CreationCommand, CreationKind, ServerStopCommand};
use super::{CreationDirectory, ManifestCommand};
use crate::cli_text::CliText;
use crate::localization::Locale;
use std::ffi::OsString;

#[test]
fn unknown_options_point_to_the_recognized_command_help() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let failure = parse(&["workspace", "get", "work", "--fomat", "json"]).unwrap_err();
        let output = crate::cli::parse_failure(failure, locale);
        assert!(
            output.stderr.contains("orchbee workspace get --help"),
            "{}",
            output.stderr
        );
    }
}

fn expected_terminal_read(target: Target, unwrap: bool, lines: Option<u16>) -> QueryCommand {
    let target = match target {
        Target::Id(id) => Target::TerminalOrPanelId(id),
        other => other,
    };
    QueryCommand::TerminalOutput {
        target: super::OutputTarget::Terminal(target),
        selection: super::OutputSelection {
            source: if unwrap {
                super::OutputSource::RecentUnwrapped
            } else {
                super::OutputSource::Recent
            },
            lines: lines.map(|n| n.min(1000)),
        },
        plain: false,
        ansi: false,
    }
}

fn expected_output_wait(text: &str, lines: Option<u16>, timeout: Option<u32>) -> QueryCommand {
    QueryCommand::TerminalSourceWait {
        after: None,
        target: Target::TerminalOrPanelId(ID.into()),
        matcher: WaitPattern::Literal(text.to_owned()),
        selection: super::OutputSelection {
            source: super::OutputSource::Recent,
            lines,
        },
        timeout_ms: Some(timeout.unwrap_or(30_000)),
    }
}

#[cfg(windows)]
#[test]
fn client_workbench_queries_route_exact_clients_and_reject_unrelated_options() {
    for words in [
        vec!["workspace", "list"],
        vec!["workspace", "get", "main"],
        vec!["tab", "list"],
        vec!["tab", "get", "main"],
        vec!["panel", "list"],
        vec!["panel", "get", ID],
    ] {
        let mut selected = words.clone();
        selected.extend(["--client", ID, "--format", "json"]);
        let Request::ClientQuery { client, format, .. } = parse(&selected).unwrap() else {
            panic!("selected query");
        };
        assert_eq!(client.as_str(), ID);
        assert_eq!(format, OutputFormat::Json);
        assert!(matches!(parse(&words), Ok(Request::Query { .. })));
        for tail in [
            vec!["--client", "invalid"],
            vec!["--client", ID, "--client", ID],
            vec!["--focus"],
            vec!["--timeout", "10"],
        ] {
            let mut invalid = words.clone();
            invalid.extend(tail);
            assert!(parse(&invalid).is_err(), "{invalid:?}");
        }
    }
    assert!(parse(&["project", "list", "--client", ID]).is_err());
}

#[test]
fn runtime_status_scopes_preserve_bare_status_and_closed_options() {
    for (word, scope) in [
        ("server", super::RuntimeStatusScope::Server),
        ("client", super::RuntimeStatusScope::Client),
    ] {
        for words in [
            vec!["status", word, "--format=json"],
            vec!["--format", "json", "status", word],
            vec!["--format=json", "--", "status", word],
        ] {
            assert_eq!(
                parse_request(words.into_iter().map(OsString::from)).unwrap(),
                Request::RuntimeStatus {
                    scope,
                    format: OutputFormat::Json
                }
            );
        }
        for suffix in [
            vec!["extra"],
            vec!["--format"],
            vec!["--format=json", "--format=text"],
            vec!["--format=csv"],
            vec!["--current"],
            vec!["--client=x"],
            vec!["--timeout=1"],
        ] {
            assert!(
                parse_request(
                    ["status", word]
                        .into_iter()
                        .chain(suffix)
                        .map(OsString::from)
                )
                .is_err()
            );
        }
    }
    assert_eq!(
        parse_request(["status"].into_iter().map(OsString::from)).unwrap(),
        query(QueryCommand::Status, OutputFormat::Text)
    );
}

#[test]
fn agent_start_parses_literal_arguments_and_rejects_invalid_configuration_before_effects() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    let Request::AgentStart { command, format } = parse(&[
        "agent",
        "start",
        "--name",
        "alpha",
        "--provider=codex",
        "--timeout=3001",
        "--format=json",
        "--",
        "",
        "--format",
        "中文",
        "%PATH%",
        "a&b",
    ])
    .unwrap() else {
        panic!("start");
    };
    assert_eq!(
        command.name.as_ref().map(|name| name.as_str()),
        Some("alpha")
    );
    assert_eq!(command.provider, "codex");
    assert_eq!(command.timeout_ms, 3001);
    assert_eq!(format, OutputFormat::Json);
    assert_eq!(command.args, ["", "--format", "中文", "%PATH%", "a&b"]);
    let Request::AgentStart { command, .. } = parse(&[
        "agent",
        "start",
        "--provider=codex",
        "--arg=",
        "--arg=--",
        "--current",
    ])
    .unwrap() else {
        panic!("start");
    };
    assert_eq!(command.timeout_ms, 30000);
    assert_eq!(command.args, ["", "--"]);
    assert_eq!(command.name, None);
    for words in [
        vec!["agent", "start"],
        vec!["agent", "start", "--name=INVALID", "--provider=codex"],
        // A word before `--` is never passed to the Agent.
        vec!["agent", "start", "alpha", "--provider=codex"],
        vec!["agent", "start", "--provider=unknown"],
        vec!["agent", "start", "--provider=claude"],
        vec!["agent", "start", "--provider=codex", "--timeout=3000"],
        vec!["agent", "start", "--provider=codex", "--timeout=300001"],
        vec!["agent", "start", "--provider=codex", "--forever"],
        vec![
            "agent",
            "start",
            "--provider=codex",
            "--current",
            "--panel=11111111111111111111111111111111",
        ],
        vec!["agent", "start", "--provider=codex", "--arg=x", "--", "y"],
        vec!["agent", "start", "--provider=codex", "--", "a\nb"],
        vec!["agent", "start", "--provider=codex", "--provider=pi"],
        vec!["agent", "names", "--provider=codex"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for provider in crate::agent_provider::CANONICAL_IDS {
        assert!(
            parse(&["agent", "start", "--provider", provider]).is_ok(),
            "{provider}"
        );
    }
    // The invalid-value diagnostic and the option help both enumerate the
    // complete closed `--provider` set from the provider registry.
    let failure = parse(&["agent", "start", "--provider=claude"]).unwrap_err();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let stderr = crate::cli::parse_failure(failure.clone(), locale).stderr;
        let (_, help) =
            CliText::new(locale).cli_help_option(super::CommandPath::AgentStart, "--provider");
        for provider in crate::agent_provider::CANONICAL_IDS {
            assert!(stderr.contains(provider), "{provider}: {stderr}");
            assert!(help.contains(provider), "{provider}: {help}");
        }
    }
}

use orchbee_workbench_protocol::{
    LaunchProfileName, LaunchProfileSelector, LaunchSelection, WorkbenchDirectoryId,
    WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchPanelSplitDirection, WorkbenchRequestId,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabMoveDirection, WorkbenchTerminalGeometry,
    WorkbenchWorkspaceId,
};

use super::PanelRelocationDestination;
use super::{
    AgentTarget, CommandPath, CompletionShell, ExpectedValue, FailureEnvelope, Family, HelpTopic,
    MutationCommand, OutputFormat, ParseFailure, ParseFailureKind, QueryCommand, Request, Resource,
    TabContext, Target, TerminalSendInput, UsageContext, UsageReason, WaitPattern, parse_request,
};

fn parse(arguments: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(arguments.iter().map(OsString::from))
}

#[test]
fn agent_names_admit_one_selector_and_a_bounded_name_before_any_effect() {
    use super::AgentNamesCommand;
    use orchbee_agent_observation_protocol::{AgentName, AgentSessionId};
    let session = AgentSessionId::parse("s1").unwrap();
    let alias = AgentName::parse("alpha").unwrap();
    for (words, command) in [
        (vec!["agent", "names"], AgentNamesCommand::List),
        (
            vec!["agent", "resolve", "s1"],
            AgentNamesCommand::Resolve(AgentTarget::Auto {
                target: session.clone(),
            }),
        ),
        (
            vec!["agent", "resolve", "--name=alpha"],
            AgentNamesCommand::Resolve(AgentTarget::Name {
                name: alias.clone(),
            }),
        ),
        (
            vec!["agent", "rename", "--agent-session=s1", "alpha"],
            AgentNamesCommand::Assign {
                target: AgentTarget::Session {
                    session_id: session.clone(),
                },
                name: alias.clone(),
            },
        ),
        (
            vec!["agent", "rename", "--clear", "s1"],
            AgentNamesCommand::Release(AgentTarget::Auto { target: session }),
        ),
    ] {
        assert_eq!(
            parse(&words),
            Ok(Request::AgentNames {
                command,
                format: OutputFormat::Text
            })
        );
    }
    for words in [
        vec!["agent", "rename", "s1", "Upper"],
        vec!["agent", "rename", "s1", "two words"],
        vec!["agent", "rename", "s1", "名字"],
        vec!["agent", "rename", "s1", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
        vec!["agent", "rename", "s1", "alpha", "--clear"],
        vec!["agent", "resolve", "s1", "--name", "alpha"],
        vec![
            "agent",
            "resolve",
            "--agent-session",
            "s1",
            "--name",
            "alpha",
        ],
        vec!["agent", "names", "--agent-session", "s1"],
        vec!["status", "--clear"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for (verb, path) in [
        ("names", CommandPath::AgentNames),
        ("resolve", CommandPath::AgentResolve),
        ("rename", CommandPath::AgentRename),
    ] {
        assert_eq!(
            parse(&["agent", verb, "--help"]),
            Ok(Request::Help(HelpTopic::Command(path)))
        );
    }
}

#[test]
fn offline_skill_is_a_sole_root_mode() {
    assert_eq!(parse(&["--skill"]), Ok(Request::Skill));
    for words in [
        vec!["--skill", "--format", "json"],
        vec!["--skill", "status"],
        vec!["status", "--skill"],
        vec!["--skill", "--skill"],
        vec!["--skill", "--help"],
        vec!["--", "--skill"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    assert_eq!(
        parse(&["completion", "powershell"]),
        Ok(Request::Completion(CompletionShell::PowerShell))
    );
    assert_eq!(
        parse(&["completion", "elvish"]),
        Ok(Request::Completion(CompletionShell::Elvish))
    );
}

fn usage(envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope,
        usage: None,
    }
}

#[cfg(windows)]
#[test]
fn default_config_is_a_sole_offline_root_mode() {
    assert_eq!(parse(&["--default-config"]), Ok(Request::DefaultConfig));
    for words in [
        vec!["--default-config", "--format", "json"],
        vec!["--format=json", "--default-config"],
        vec!["--default-config", "status"],
        vec!["status", "--default-config"],
        vec!["--default-config", "--default-config"],
        vec!["--default-config", "--help"],
        vec!["--", "--default-config"],
        vec!["--session", "work", "--default-config"],
        vec!["--machine", "work", "--default-config"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

fn identifier(resource: Resource, target: &str, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier {
            resource,
            target: target.to_owned(),
        },
        envelope,
        usage: None,
    }
}

fn query(command: QueryCommand, format: OutputFormat) -> Request {
    Request::Query { command, format }
}

const ID: &str = "0123456789abcdef0123456789abcdef";
const ZERO: &str = "00000000000000000000000000000000";

#[test]
fn accepts_legacy_root_forms() {
    assert_eq!(parse(&[]), Ok(Request::Tui { mouse: true }));
    assert_eq!(parse(&["tui"]), Ok(Request::Tui { mouse: true }));
    assert_eq!(parse(&["--"]), Ok(Request::Tui { mouse: true }));
    assert_eq!(parse(&["--", "tui"]), Ok(Request::Tui { mouse: true }));
    assert_eq!(parse(&["--help"]), Ok(Request::RootHelp));
    assert_eq!(parse(&["-h"]), Ok(Request::RootHelp));
    assert_eq!(parse(&["help"]), Ok(Request::RootHelp));
    assert_eq!(parse(&["--version"]), Ok(Request::Version));
}

#[test]
fn version_aliases_share_root_grammar_and_preserve_literal_diagnostics() {
    for flag in ["-v", "-V", "--version"] {
        assert_eq!(parse(&[flag]), Ok(Request::Version), "{flag}");
        for arguments in [
            vec![flag, "status"],
            vec!["server", flag],
            vec![flag, "--help"],
            vec!["--help", flag],
            vec![flag, flag],
            vec![flag, "--version"],
        ] {
            let failure = parse(&arguments).unwrap_err();
            assert!(
                matches!(failure.usage, Some(UsageReason::MisplacedFlag { flag: literal, .. }) if literal == flag),
                "{arguments:?}: {failure:?}"
            );
        }
        for arguments in [
            vec![flag, "--format=json"],
            vec!["--format=json", flag],
            vec![flag, "--confirm"],
            vec![flag, "--mouse=off"],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
        assert_eq!(
            parse(&["project", "get", "--", flag]),
            Ok(query(
                QueryCommand::ProjectGet(Target::Label(flag.into())),
                OutputFormat::Text
            ))
        );
        assert_eq!(
            parse(&["project", "rename", ID, "--label", flag]),
            Ok(Request::Mutation {
                command: MutationCommand::ProjectRename {
                    target: Target::Id(ID.into()),
                    label: label(flag),
                },
                format: OutputFormat::Text,
            })
        );
    }
}

#[test]
fn help_command_accepts_help_flags_without_relaxing_its_path_or_option_scope() {
    for flag in ["-h", "--help"] {
        assert_eq!(parse(&["help", flag]), Ok(Request::RootHelp));
        assert_eq!(parse(&[flag, "help"]), Ok(Request::RootHelp));
        for arguments in [
            vec!["help", "help", flag],
            vec!["help", "server", "stpo", flag],
            vec!["help", "server", "stop", "extra", flag],
            vec!["help", "server", "--format=json", flag],
            vec!["help", "--", flag],
            vec!["help", flag, flag],
        ] {
            assert!(parse(&arguments).is_err(), "{arguments:?}");
        }
    }
}

#[test]
fn tui_mouse_option_parses_on_the_explicit_route() {
    assert_eq!(
        parse(&["tui", "--mouse", "off"]),
        Ok(Request::Tui { mouse: false })
    );
    assert_eq!(
        parse(&["tui", "--mouse", "on"]),
        Ok(Request::Tui { mouse: true })
    );
    // The option scan is positional-agnostic: the flag may precede `tui`.
    assert_eq!(
        parse(&["--mouse", "off", "tui"]),
        Ok(Request::Tui { mouse: false })
    );
    // The joined spelling is equivalent, still through the option's own
    // closed-vocabulary validation.
    assert_eq!(
        parse(&["tui", "--mouse=off"]),
        Ok(Request::Tui { mouse: false })
    );
    assert_eq!(
        parse(&["tui", "--mouse=on"]),
        Ok(Request::Tui { mouse: true })
    );
    assert_eq!(
        parse(&["--mouse=off", "tui"]),
        Ok(Request::Tui { mouse: false })
    );
}

#[test]
fn accepts_every_query_path_with_default_text() {
    let cases: &[(&[&str], QueryCommand)] = &[
        (&["status"], QueryCommand::Status),
        (&["project", "list"], QueryCommand::ProjectList),
        (
            &["project", "get", ID],
            QueryCommand::ProjectGet(Target::Id(ID.into())),
        ),
        (
            &["project", "get", "web"],
            QueryCommand::ProjectGet(Target::Label("web".into())),
        ),
        (
            &["workspace", "list"],
            QueryCommand::WorkspaceList {
                project: None,
                worktrees: false,
            },
        ),
        (
            &["workspace", "get", "main"],
            QueryCommand::WorkspaceGet(Target::Label("main".into())),
        ),
        (&["tab", "list"], QueryCommand::TabList { workspace: None }),
        (
            &["tab", "get", ID],
            QueryCommand::TabGet(Target::Id(ID.into())),
        ),
        (
            &["panel", "list"],
            QueryCommand::PanelList {
                tab: None,
                workspace: None,
            },
        ),
        (
            &["panel", "get", ID],
            QueryCommand::PanelGet(Target::Id(ID.into())),
        ),
        (
            &["terminal", "list"],
            QueryCommand::TerminalList { filter: None },
        ),
        (
            &["terminal", "get", ID],
            QueryCommand::TerminalGet(Target::Id(ID.into())),
        ),
        (
            &["terminal", "read", ID],
            expected_terminal_read(Target::Id(ID.into()), false, None),
        ),
        (&["api", "snapshot"], QueryCommand::ApiSnapshot),
        (
            &["api", "schema"],
            QueryCommand::ApiSchema { schema_id: None },
        ),
        (
            &["api", "schema", "orchbee.status"],
            QueryCommand::ApiSchema {
                schema_id: Some("orchbee.status".into()),
            },
        ),
        (&["config", "check"], QueryCommand::ConfigCheck),
    ];
    for (arguments, command) in cases {
        assert_eq!(
            parse(arguments),
            Ok(query(command.clone(), OutputFormat::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn session_start_admits_only_format() {
    // The bare path defaults to text; `--format` is the sole option.
    assert_eq!(
        parse(&["server", "start"]),
        Ok(Request::ServerStart {
            format: OutputFormat::Text
        })
    );
    assert_eq!(
        parse(&["server", "start", "--format", "json"]),
        Ok(Request::ServerStart {
            format: OutputFormat::Json
        })
    );
    assert_eq!(
        parse(&["--format=json", "server", "start"]),
        Ok(Request::ServerStart {
            format: OutputFormat::Json
        })
    );
    let cases: &[&[&str]] = &[
        // Operands, filters, and business flags are all out of scope.
        &["server"],
        &["server", "stop"],
        &["server", "start", "extra"],
        &["server", "start", "--project", "x"],
        &["server", "start", "--confirm"],
        &["server", "start", "--mouse", "off"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // A missing or unsupported `--format` value names the concrete cause.
    assert_eq!(
        parse(&["server", "start", "--format"]),
        Err(ParseFailure {
            kind: ParseFailureKind::InvalidUsage,
            envelope: FailureEnvelope::Silent,
            usage: Some(UsageReason::MissingValue { option: "--format" }),
        })
    );
    assert_eq!(
        parse(&["server", "start", "--format", "yaml"]),
        Err(ParseFailure {
            kind: ParseFailureKind::InvalidUsage,
            envelope: FailureEnvelope::Silent,
            usage: Some(UsageReason::InvalidValue {
                context: UsageContext::Path("server start"),
                option: "--format",
                value: "yaml".to_owned(),
                expected: ExpectedValue::Format,
            }),
        })
    );
    // A json envelope selection survives into the usage failure.
    assert_eq!(
        parse(&["server", "start", "extra", "--format", "json"]),
        Err(ParseFailure {
            kind: ParseFailureKind::InvalidUsage,
            envelope: FailureEnvelope::Json,
            usage: Some(UsageReason::UnexpectedArgument {
                context: UsageContext::Path("server start"),
                value: "extra".to_owned(),
            }),
        })
    );
    // Help-path resolution: family overview and command help.
    assert_eq!(
        parse(&["server", "--help"]),
        Ok(Request::Help(HelpTopic::Family(Family::Server)))
    );
    assert_eq!(
        parse(&["server", "start", "--help"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::ServerStart)))
    );
    assert_eq!(
        parse(&["help", "server", "start"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::ServerStart)))
    );
}

#[test]
fn session_stop_requires_confirmation_and_preserves_explicit_recovery_identity() {
    let id = "11111111111111111111111111111111";
    assert!(matches!(
        parse(&["server", "stop", "--confirm"]),
        Ok(Request::ServerStop {
            command: ServerStopCommand::Stop {
                id: None,
                confirm: true,
                retry: false,
                detach: false,
                timeout_ms: 65_000
            },
            format: OutputFormat::Text
        })
    ));
    assert!(matches!(
        parse(&[
            "server",
            "stop",
            "--confirm",
            "--request-id",
            id,
            "--retry",
            "--detach",
            "--timeout=1",
            "--format=json"
        ]),
        Ok(Request::ServerStop {
            command: ServerStopCommand::Stop {
                id: Some(_),
                confirm: true,
                retry: true,
                detach: true,
                timeout_ms: 1
            },
            format: OutputFormat::Json
        })
    ));
    assert!(matches!(
        parse(&["server", "stop-status", id]),
        Ok(Request::ServerStop {
            command: ServerStopCommand::Status { .. },
            ..
        })
    ));
    for args in [
        vec!["server", "stop"],
        vec!["server", "stop", "--confirm", "--retry"],
        vec![
            "server",
            "stop",
            "--confirm",
            "--request-id",
            "00000000000000000000000000000000",
        ],
        vec![
            "server",
            "stop",
            "--confirm",
            "--request-id",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ],
        vec!["server", "stop", "--confirm", "--timeout=0"],
        vec!["server", "stop", "--confirm", "--timeout=3600001"],
        vec!["server", "stop", "--confirm", "--detach", "--detach"],
        vec!["server", "stop-status", id, "--confirm"],
        vec!["server", "stop-status", id, "--retry"],
        vec!["server", "stop-status", "short"],
        vec!["server", "stop-status"],
        vec!["server", "start", "--detach"],
        vec!["server", "diagnose", "--request-id", id],
        vec!["--retry", "status"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn session_diagnose_admits_only_format() {
    // The bare path defaults to text; `--format` is the sole option.
    assert_eq!(
        parse(&["server", "diagnose"]),
        Ok(Request::ServerDiagnose {
            format: OutputFormat::Text
        })
    );
    assert_eq!(
        parse(&["server", "diagnose", "--format", "json"]),
        Ok(Request::ServerDiagnose {
            format: OutputFormat::Json
        })
    );
    assert_eq!(
        parse(&["--format=json", "server", "diagnose"]),
        Ok(Request::ServerDiagnose {
            format: OutputFormat::Json
        })
    );
    let cases: &[&[&str]] = &[
        // Operands, filters, and business flags are all out of scope —
        // the command needs no `--confirm`: it performs no effect.
        &["server", "diagnose", "extra"],
        &["server", "diagnose", "--project", "x"],
        &["server", "diagnose", "--confirm"],
        &["server", "diagnose", "--mouse", "off"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // A json envelope selection survives into the usage failure.
    assert_eq!(
        parse(&["server", "diagnose", "extra", "--format", "json"]),
        Err(ParseFailure {
            kind: ParseFailureKind::InvalidUsage,
            envelope: FailureEnvelope::Json,
            usage: Some(UsageReason::UnexpectedArgument {
                context: UsageContext::Path("server diagnose"),
                value: "extra".to_owned(),
            }),
        })
    );
    // Help-path resolution: command help is its own topic.
    assert_eq!(
        parse(&["server", "diagnose", "--help"]),
        Ok(Request::Help(HelpTopic::Command(
            CommandPath::ServerDiagnose
        )))
    );
    assert_eq!(
        parse(&["help", "server", "diagnose"]),
        Ok(Request::Help(HelpTopic::Command(
            CommandPath::ServerDiagnose
        )))
    );
}

#[test]
fn accepts_format_in_every_position_and_both_spellings() {
    let cases: &[(&[&str], QueryCommand)] = &[
        (&["--format", "json", "status"], QueryCommand::Status),
        (&["--format=json", "status"], QueryCommand::Status),
        (&["status", "--format", "json"], QueryCommand::Status),
        (&["status", "--format=json"], QueryCommand::Status),
        (
            &["project", "--format", "json", "list"],
            QueryCommand::ProjectList,
        ),
        (
            &["project", "list", "--format", "json"],
            QueryCommand::ProjectList,
        ),
        (&["--format=json", "--", "status"], QueryCommand::Status),
    ];
    for (arguments, command) in cases {
        assert_eq!(
            parse(arguments),
            Ok(query(command.clone(), OutputFormat::Json)),
            "{arguments:?}"
        );
    }
    assert_eq!(
        parse(&["status", "--format", "text"]),
        Ok(query(QueryCommand::Status, OutputFormat::Text))
    );
}

#[test]
fn option_terminator_makes_following_tokens_positional() {
    assert_eq!(
        parse(&["--", "status"]),
        Ok(query(QueryCommand::Status, OutputFormat::Text))
    );
    // A literal --help after -- is a label, not the help flag.
    assert_eq!(
        parse(&["project", "get", "--", "--help"]),
        Ok(query(
            QueryCommand::ProjectGet(Target::Label("--help".into())),
            OutputFormat::Text
        ))
    );
    // Format after -- is positional, making the path malformed.
    assert_eq!(
        parse(&["status", "--", "--format", "json"]),
        Err(usage(FailureEnvelope::Text))
    );
}

#[test]
fn format_failures_never_emit_stdout() {
    let cases: &[&[&str]] = &[
        &["status", "--format"],
        &["status", "--format", "--"],
        &["status", "--format", "yaml"],
        &["status", "--format="],
        &["status", "--format=JSon"],
        &["status", "--format", "json", "--format", "text"],
        &["status", "--format=json", "--format", "json"],
        // Even an earlier valid json selection does not rescue a bad selector.
        &["--format", "json", "--format", "yaml", "status"],
        // A format defect anywhere in the envelope is silent, even when the
        // grammar error sits before it or a filter legally consumed a token
        // that merely resembles a selector.
        &["--bogus", "--format="],
        &["status", "extra", "--format", "yaml"],
        &["status", "extra", "--format="],
        &["--bogus", "--format", "yaml"],
        // A filter owns exactly one token; a real selector after its value is
        // still evaluated and fails the whole envelope.
        &["workspace", "list", "--project", "x", "--format", "yaml"],
        &["workspace", "list", "--project", "x", "--format="],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Silent)),
            "{arguments:?}"
        );
    }
}

#[test]
fn valid_json_selection_turns_later_syntax_failure_into_json() {
    let cases: &[&[&str]] = &[
        &["--format", "json"],
        &["--format=json", "tui"],
        &["--format=json", "help"],
        &["--format=json", "help", "status"],
        &["--format=json", "completion", "bash"],
        &["--format=json", "--version"],
        &["status", "--format=json", "extra"],
        &["status", "extra", "--format=json"],
        &["--format=json", "--bogus"],
        &["project", "--format=json"],
        &["--format=json", "get"],
        // A unique valid selection controls the envelope no matter where the
        // grammar failure sits relative to it.
        &["--bogus", "--format", "json"],
        &["status", "--unknown", "--format", "json"],
        &["status", "--unknown", "--format=json"],
        // A selector-like token after `--` is positional, not a format defect:
        // the earlier valid selection still shapes the failure.
        &["--format=json", "--", "status", "--format=yaml"],
        &["--format=json", "--", "bogus"],
        // A valid `--format` selection still shapes the failure when the
        // `tui` route then rejects it or `--mouse` is malformed.
        &["--mouse", "off", "--format=json", "tui"],
        &["--mouse", "bogus", "--format=json"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Json)),
            "{arguments:?}"
        );
    }
}

#[test]
fn text_and_unselected_syntax_failures_stay_silent() {
    let cases: &[&[&str]] = &[
        &["bogus"],
        &["status", "extra"],
        &["project"],
        &["project", "get"],
        &["project", "get", "a", "b"],
        &["project", "create"],
        &["project", "create", "--label"],
        &["project", "create", "--bogus"],
        &["project", "create", "--label", "x", "--label", "y"],
        &["project", "create", "--label", "x", "extra"],
        &["project", "create", "--", "--label", "x"],
        &["project", "rename"],
        &["project", "rename", ID],
        &["project", "rename", ID, ID, "--label", "x"],
        &["project", "rename", ID, "--label", "x", "--label", "y"],
        &["project", "rename", ID, "--label", "x", "--confirm"],
        &["project", "rename", ID, "--label", "x", "extra"],
        &["project", "rename", "--", "--label", "x"],
        &["-x"],
        &["--unknown"],
        &["status", "--unknown"],
        &["help", "help"],
        &["help", "version"],
        &["help", "--version"],
        &["status", "--version"],
        &["--version", "status"],
        &["--version", "--help"],
        &["tui", "--format", "text"],
        // `--mouse` belongs to the explicit `tui` route alone: the bare
        // invocation and every other family reject it in either spelling,
        // duplicates and unknown values are usage, and `--` never rescues
        // it.
        &["--mouse", "off"],
        &["--mouse=off"],
        &["status", "--mouse", "off"],
        &["--mouse", "off", "status"],
        &["help", "--mouse", "off"],
        &["completion", "bash", "--mouse", "off"],
        &["workspace", "list", "--mouse", "off"],
        &["--version", "--mouse", "off"],
        &["tui", "--mouse"],
        &["tui", "--mouse=bogus"],
        &["tui", "--mouse", "bogus"],
        &["tui", "--mouse", "off", "--mouse", "on"],
        &["tui", "--", "--mouse", "off"],
        &["completion"],
        &["completion", "unknown-shell"],
        &["completion", "bash", "extra"],
        &["api", "snapshot", "extra"],
        &["config"],
        &["config", "check", "extra"],
        // `terminal read` takes exactly one typed-id operand.
        &["terminal", "read"],
        &["terminal", "read", ID, "extra"],
        &["terminal", "read", ID, "--standalone"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn help_recognizes_command_paths_with_operands_and_options_without_execution() {
    assert_eq!(parse(&["--format=json", "--help"]), Ok(Request::RootHelp));
    for args in [
        vec!["workspace", "get", "work", "--help"],
        vec!["workspace", "get", "--format", "json", "--help"],
        vec!["--help", "workspace", "get", "work"],
        vec!["workspace", "--help", "get", "work"],
    ] {
        assert_eq!(
            parse(&args),
            Ok(Request::Help(HelpTopic::Command(CommandPath::WorkspaceGet))),
            "{args:?}"
        );
    }
    for args in [
        vec!["workspace", "get", "--bogus", "--help"],
        vec!["workspace", "get", "--label", "--help"],
        vec!["workspace", "get", "--help", "--help"],
        vec!["workspace", "misspelled", "--help"],
        vec!["terminal", "observe", "--help"],
        vec!["help", "terminal", "control"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    for path in super::help::help_paths() {
        if let HelpTopic::Command(command) = path.topic() {
            assert_eq!(path.words().join(" "), command.as_str());
        }
    }
}

#[test]
fn filters_have_declared_scope_single_use_and_values() {
    // Correct scopes.
    assert_eq!(
        parse(&["workspace", "list", "--project", "web"]),
        Ok(query(
            QueryCommand::WorkspaceList {
                project: Some(Target::Label("web".into())),
                worktrees: false,
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["tab", "list", "--workspace", ID]),
        Ok(query(
            QueryCommand::TabList {
                workspace: Some(Target::Id(ID.into())),
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["tab", "list", "--workspace", "Workspace"]),
        Ok(query(
            QueryCommand::TabList {
                workspace: Some(Target::Label("Workspace".into()))
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["panel", "list", "--tab", ID]),
        Ok(query(
            QueryCommand::PanelList {
                tab: Some(Target::Id(ID.into())),
                workspace: None,
            },
            OutputFormat::Text
        ))
    );

    let cases: &[&[&str]] = &[
        // Filters on wrong commands or get paths.
        &["project", "list", "--project", "x"],
        &["project", "get", "x", "--project", "y"],
        &["status", "--workspace", "x"],
        &["terminal", "list", "--standalone"],
        &["terminal", "list", "--project", "x"],
        &["api", "snapshot", "--tab", "x"],
        &["api", "schema", "--workspace", "x"],
        &["config", "check", "--standalone"],
        &["help", "status", "--project", "x"],
        &["completion", "bash", "--format", "text"],
        &["--project", "x", "status"],
        // `workspace list` declares only --project: no other filter may slip
        // through silently.
        &["workspace", "list", "--workspace", "x"],
        &["workspace", "list", "--tab", "x"],
        &["workspace", "list", "--standalone"],
        &["workspace", "list", "--project", "x", "--workspace", "y"],
        // `tab list` declares only --workspace.
        &["tab", "list", "--project", "x"],
        &["tab", "list", "--tab", "x"],
        // `panel list` declares only --tab/--workspace, one at a time.
        &["panel", "list", "--project", "x"],
        &["panel", "list", "--tab", "x", "--workspace", "y"],
        &["panel", "list", "--standalone"],
        // Mutual exclusion and duplication.
        &["tab", "list", "--workspace", "x", "--standalone"],
        &["tab", "list", "--standalone", "--workspace", "x"],
        &["tab", "list", "--standalone", "--standalone"],
        &["workspace", "list", "--project", "a", "--project", "b"],
        &["tab", "list", "--workspace", "a", "--workspace", "b"],
        &["panel", "list", "--tab", "a", "--tab", "b"],
        // Missing values, including a bare terminator that cannot be a value.
        &["workspace", "list", "--project"],
        &["workspace", "list", "--project", "--"],
        &["--project"],
        // A boolean never takes a joined value.
        &["tab", "list", "--standalone=true"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn dash_leading_tokens_can_be_filter_values() {
    // The next token after a named filter is consumed as its value even when it
    // begins with a dash; it is then never also a format selector.
    assert_eq!(
        parse(&["workspace", "list", "--project", "--format"]),
        Ok(query(
            QueryCommand::WorkspaceList {
                project: Some(Target::Label("--format".into())),
                worktrees: false,
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["workspace", "list", "--project", "--format", "json"]),
        Err(usage(FailureEnvelope::Text))
    );
}

#[test]
fn suffix_help_resolves_any_recognized_help_path() {
    let topics: &[(&[&str], HelpTopic)] = &[
        // Families resolve to their overview, exactly like `help <family>`.
        (&["project", "--help"], HelpTopic::Family(Family::Project)),
        (
            &["workspace", "--help"],
            HelpTopic::Family(Family::Workspace),
        ),
        (&["tab", "--help"], HelpTopic::Family(Family::Tab)),
        (&["panel", "--help"], HelpTopic::Family(Family::Panel)),
        (&["terminal", "--help"], HelpTopic::Family(Family::Terminal)),
        (&["api", "--help"], HelpTopic::Family(Family::Api)),
        (&["config", "--help"], HelpTopic::Family(Family::Config)),
        (
            &["completion", "--help"],
            HelpTopic::Family(Family::Completion),
        ),
        (&["tui", "--help"], HelpTopic::Command(CommandPath::Tui)),
        (
            &["status", "--help"],
            HelpTopic::Command(CommandPath::Status),
        ),
        (
            &["project", "list", "--help"],
            HelpTopic::Command(CommandPath::ProjectList),
        ),
        (
            &["project", "get", "--help"],
            HelpTopic::Command(CommandPath::ProjectGet),
        ),
        (
            &["project", "rename", "--help"],
            HelpTopic::Command(CommandPath::ProjectRename),
        ),
        (
            &["workspace", "list", "--help"],
            HelpTopic::Command(CommandPath::WorkspaceList),
        ),
        (
            &["workspace", "get", "--help"],
            HelpTopic::Command(CommandPath::WorkspaceGet),
        ),
        (
            &["tab", "list", "--help"],
            HelpTopic::Command(CommandPath::TabList),
        ),
        (
            &["tab", "get", "--help"],
            HelpTopic::Command(CommandPath::TabGet),
        ),
        (
            &["panel", "list", "--help"],
            HelpTopic::Command(CommandPath::PanelList),
        ),
        (
            &["panel", "get", "--help"],
            HelpTopic::Command(CommandPath::PanelGet),
        ),
        (
            &["terminal", "list", "--help"],
            HelpTopic::Command(CommandPath::TerminalList),
        ),
        (
            &["terminal", "get", "--help"],
            HelpTopic::Command(CommandPath::TerminalGet),
        ),
        (
            &["terminal", "read", "--help"],
            HelpTopic::Command(CommandPath::TerminalRead),
        ),
        (
            &["terminal", "send", "--help"],
            HelpTopic::Command(CommandPath::TerminalSend),
        ),
        (
            &["api", "snapshot", "--help"],
            HelpTopic::Command(CommandPath::ApiSnapshot),
        ),
        (
            &["api", "schema", "--help"],
            HelpTopic::Command(CommandPath::ApiSchema),
        ),
        (
            &["config", "check", "--help"],
            HelpTopic::Command(CommandPath::ConfigCheck),
        ),
    ];
    for (arguments, topic) in topics {
        assert_eq!(parse(arguments), Ok(Request::Help(*topic)), "{arguments:?}");
        // The -h short alias is the same suffix flag at the same position.
        let mut aliased = arguments.to_vec();
        *aliased.last_mut().expect("suffix flag") = "-h";
        assert_eq!(parse(&aliased), Ok(Request::Help(*topic)), "{aliased:?}");
    }

    let cases: &[(&[&str], HelpTopic)] = &[
        (
            &["project", "get", "x", "--help"],
            HelpTopic::Command(CommandPath::ProjectGet),
        ),
        (
            &["api", "schema", "orchbee.status", "--help"],
            HelpTopic::Command(CommandPath::ApiSchema),
        ),
        (
            &["completion", "bash", "--help"],
            HelpTopic::Family(Family::Completion),
        ),
        (
            &["workspace", "list", "--project", "x", "--help"],
            HelpTopic::Command(CommandPath::WorkspaceList),
        ),
        (
            &["tab", "list", "--workspace", "x", "--help"],
            HelpTopic::Command(CommandPath::TabList),
        ),
        (
            &["status", "--help", "--format", "text"],
            HelpTopic::Command(CommandPath::Status),
        ),
        (
            &["--format", "text", "status", "--help"],
            HelpTopic::Command(CommandPath::Status),
        ),
    ];
    for (arguments, topic) in cases {
        assert_eq!(parse(arguments), Ok(Request::Help(*topic)), "{arguments:?}");
    }
    assert_eq!(
        parse(&["status", "--help", "--format", "json"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::Status)))
    );
}

#[test]
fn help_command_accepts_recognized_paths_only() {
    let topics: &[(&[&str], HelpTopic)] = &[
        (&["help", "tui"], HelpTopic::Command(CommandPath::Tui)),
        (&["help", "status"], HelpTopic::Command(CommandPath::Status)),
        (&["help", "project"], HelpTopic::Family(Family::Project)),
        (&["help", "workspace"], HelpTopic::Family(Family::Workspace)),
        (&["help", "tab"], HelpTopic::Family(Family::Tab)),
        (&["help", "panel"], HelpTopic::Family(Family::Panel)),
        (&["help", "terminal"], HelpTopic::Family(Family::Terminal)),
        (&["help", "api"], HelpTopic::Family(Family::Api)),
        (&["help", "config"], HelpTopic::Family(Family::Config)),
        (
            &["help", "completion"],
            HelpTopic::Family(Family::Completion),
        ),
        (
            &["help", "project", "list"],
            HelpTopic::Command(CommandPath::ProjectList),
        ),
        (
            &["help", "project", "get"],
            HelpTopic::Command(CommandPath::ProjectGet),
        ),
        (
            &["help", "project", "rename"],
            HelpTopic::Command(CommandPath::ProjectRename),
        ),
        (
            &["help", "workspace", "list"],
            HelpTopic::Command(CommandPath::WorkspaceList),
        ),
        (
            &["help", "workspace", "get"],
            HelpTopic::Command(CommandPath::WorkspaceGet),
        ),
        (
            &["help", "tab", "list"],
            HelpTopic::Command(CommandPath::TabList),
        ),
        (
            &["help", "tab", "get"],
            HelpTopic::Command(CommandPath::TabGet),
        ),
        (
            &["help", "panel", "list"],
            HelpTopic::Command(CommandPath::PanelList),
        ),
        (
            &["help", "panel", "get"],
            HelpTopic::Command(CommandPath::PanelGet),
        ),
        (
            &["help", "terminal", "list"],
            HelpTopic::Command(CommandPath::TerminalList),
        ),
        (
            &["help", "terminal", "get"],
            HelpTopic::Command(CommandPath::TerminalGet),
        ),
        (
            &["help", "terminal", "read"],
            HelpTopic::Command(CommandPath::TerminalRead),
        ),
        (
            &["help", "terminal", "send"],
            HelpTopic::Command(CommandPath::TerminalSend),
        ),
        (
            &["help", "api", "snapshot"],
            HelpTopic::Command(CommandPath::ApiSnapshot),
        ),
        (
            &["help", "api", "schema"],
            HelpTopic::Command(CommandPath::ApiSchema),
        ),
        (
            &["help", "config", "check"],
            HelpTopic::Command(CommandPath::ConfigCheck),
        ),
    ];
    for (arguments, topic) in topics {
        assert_eq!(parse(arguments), Ok(Request::Help(*topic)), "{arguments:?}");
    }
    for arguments in [
        &["help", "bogus"][..],
        &["help", "status", "extra"][..],
        &["help", "api", "schema", "x"][..],
    ] {
        assert_eq!(parse(arguments), Err(usage(FailureEnvelope::Text)));
    }
}

#[test]
fn completion_requires_a_known_shell() {
    for (word, shell) in [
        ("bash", CompletionShell::Bash),
        ("zsh", CompletionShell::Zsh),
        ("fish", CompletionShell::Fish),
    ] {
        assert_eq!(parse(&["completion", word]), Ok(Request::Completion(shell)));
    }
}

#[test]
fn target_rules_follow_each_resource() {
    // Zero-shaped IDs are invalid identifiers everywhere.
    for arguments in [
        &["project", "get", ZERO][..],
        &["workspace", "get", ZERO][..],
        &["tab", "get", ZERO][..],
        &["panel", "get", ZERO][..],
        &["terminal", "get", ZERO][..],
        &["terminal", "read", ZERO][..],
    ] {
        assert!(matches!(
            parse(arguments),
            Err(ParseFailure {
                kind: ParseFailureKind::InvalidIdentifier { .. },
                envelope: FailureEnvelope::Text,
                ..
            })
        ));
    }
    // Panel and terminal accept only complete typed IDs.
    for (arguments, resource, value) in [
        (&["panel", "get", "short"][..], Resource::Panel, "short"),
        (&["panel", "get", ""][..], Resource::Panel, ""),
        (&["terminal", "get", "X234"][..], Resource::Terminal, "X234"),
        (&["terminal", "get", ZERO][..], Resource::Terminal, ZERO),
        (
            &["terminal", "read", "label"][..],
            Resource::Terminal,
            "label",
        ),
        (&["terminal", "read", ""][..], Resource::Terminal, ""),
    ] {
        assert_eq!(
            parse(arguments),
            Err(identifier(resource, value, FailureEnvelope::Text))
        );
    }
    // Label-capable resources reject empty and overlong labels only.
    assert_eq!(
        parse(&["project", "get", ""]),
        Err(identifier(Resource::Project, "", FailureEnvelope::Text))
    );
    let overlong = "x".repeat(513);
    assert_eq!(
        parse(&["workspace", "get", overlong.as_str()]),
        Err(identifier(
            Resource::Workspace,
            overlong.as_str(),
            FailureEnvelope::Text
        ))
    );
    let maximum = "x".repeat(512);
    assert_eq!(
        parse(&["tab", "get", maximum.as_str()]),
        Ok(query(
            QueryCommand::TabGet(Target::Label(maximum)),
            OutputFormat::Text
        ))
    );
    // Uppercase or wrong-length hex is a label, not an ID.
    assert_eq!(
        parse(&["project", "get", "0123456789ABCDEF0123456789ABCDEF"]),
        Ok(query(
            QueryCommand::ProjectGet(Target::Label("0123456789ABCDEF0123456789ABCDEF".into())),
            OutputFormat::Text
        ))
    );
    // Invalid identifiers carry the JSON envelope when json was selected.
    assert_eq!(
        parse(&["--format=json", "panel", "get", "nope"]),
        Err(identifier(Resource::Panel, "nope", FailureEnvelope::Json))
    );
    // Empty schema id is an invalid identifier on the schema resource.
    assert_eq!(
        parse(&["api", "schema", ""]),
        Err(identifier(Resource::Schema, "", FailureEnvelope::Text))
    );
}

#[test]
fn selector_targets_reject_malformed_and_foreign_forms() {
    // A recognized `@<kind>:` prefix with an empty, uppercase, non-hex, or
    // unprefixed hexadecimal payload is an invalid identifier before any runtime
    // access — and so is a selector whose kind is not the operand's
    // resource. Panel and terminal additionally reject every label.
    for (arguments, resource, value) in [
        (
            &["project", "get", "@project:"][..],
            Resource::Project,
            "@project:",
        ),
        (
            &["project", "get", "@project:AB"][..],
            Resource::Project,
            "@project:AB",
        ),
        (
            &["project", "get", "@project:xy"][..],
            Resource::Project,
            "@project:xy",
        ),
        (
            &[
                "project",
                "get",
                "@project:0123456789abcdef0123456789abcdef",
            ][..],
            Resource::Project,
            "@project:0123456789abcdef0123456789abcdef",
        ),
        (
            &["project", "get", "@workspace:0xa"][..],
            Resource::Project,
            "@workspace:0xa",
        ),
        (
            &["tab", "get", "@panel:0xa"][..],
            Resource::Tab,
            "@panel:0xa",
        ),
        (
            &["panel", "get", "@tab:0xa"][..],
            Resource::Panel,
            "@tab:0xa",
        ),
        (
            &["terminal", "get", "@terminal:"][..],
            Resource::Terminal,
            "@terminal:",
        ),
        (
            &["terminal", "read", "@terminal:G"][..],
            Resource::Terminal,
            "@terminal:G",
        ),
        (
            &["panel", "get", "@panel:0123456789abcdef0123456789abcdef"][..],
            Resource::Panel,
            "@panel:0123456789abcdef0123456789abcdef",
        ),
    ] {
        assert_eq!(
            parse(arguments),
            Err(identifier(resource, value, FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // `@` tokens without a recognized `<kind>:` prefix keep their label
    // reading where labels are admitted — no `@` namespace is reserved.
    for (arguments, command) in [
        (
            &["project", "get", "@foo:a"][..],
            QueryCommand::ProjectGet(Target::Label("@foo:a".into())),
        ),
        (
            &["project", "get", "@project"][..],
            QueryCommand::ProjectGet(Target::Label("@project".into())),
        ),
        (
            &["project", "get", "@PROJECT:a"][..],
            QueryCommand::ProjectGet(Target::Label("@PROJECT:a".into())),
        ),
        (
            &["tab", "get", "@"][..],
            QueryCommand::TabGet(Target::Label("@".into())),
        ),
    ] {
        assert_eq!(
            parse(arguments),
            Ok(query(command, OutputFormat::Text)),
            "{arguments:?}"
        );
    }
    // Bare hex-looking words stay labels; only the explicit prefixes claim
    // selector syntax. `face`, `a`, and `1` are still exact labels.
    for value in ["face", "a", "1"] {
        assert_eq!(
            parse(&["project", "get", value]),
            Ok(query(
                QueryCommand::ProjectGet(Target::Label(value.into())),
                OutputFormat::Text
            ))
        );
    }
}

#[test]
fn single_dash_is_a_positional_label() {
    assert_eq!(
        parse(&["project", "get", "-"]),
        Ok(query(
            QueryCommand::ProjectGet(Target::Label("-".into())),
            OutputFormat::Text
        ))
    );
}

#[test]
fn rejects_arguments_beyond_the_byte_envelope() {
    let big = "x".repeat(64 * 1024);
    assert_eq!(
        parse(&["status", big.as_str()]),
        Err(usage(FailureEnvelope::Silent))
    );
    // The envelope check runs before option interpretation.
    let big = "x".repeat(64 * 1024 - 16);
    assert_eq!(
        parse(&["--format=json", "status", big.as_str(), "--format=json"]),
        Err(usage(FailureEnvelope::Silent))
    );
}

#[cfg(unix)]
#[test]
fn rejects_non_unicode_arguments() {
    use std::os::unix::ffi::OsStringExt;

    let argument = OsString::from_vec(vec![0xff]);
    assert_eq!(
        parse_request(vec![argument].into_iter()),
        Err(ParseFailure {
            kind: ParseFailureKind::InvalidUsage,
            envelope: FailureEnvelope::Silent,
            usage: Some(UsageReason::ArgumentEnvelope),
        })
    );
}

// ---- mutation commands ------------------------------------------------------

fn mutation(command: MutationCommand, format: OutputFormat) -> Request {
    let (kind, label, geometry, launch, profile, options) = match command {
        MutationCommand::TabCreate {
            automatic_label: false,
            context,
            label,
            geometry,
            launch,
            profile,
            options,
        } => (
            CreationKind::Tab {
                context: Some(context),
            },
            label,
            geometry,
            launch,
            profile,
            options,
        ),
        MutationCommand::PanelSplit {
            right_click,
            target,
            direction,
            label,
            custom_label: _,
            geometry,
            launch,
            profile,
            options,
        } => (
            CreationKind::Split {
                right_click,
                target: Some(target),
                direction,
                ratio: None,
            },
            label,
            geometry,
            launch,
            profile,
            options,
        ),
        _ => return Request::Mutation { command, format },
    };
    Request::Creation {
        command: CreationCommand {
            directory: if options.cwd.is_some() {
                CreationDirectory::Explicit
            } else {
                CreationDirectory::Default
            },
            kind,
            label: Some(label),
            geometry,
            launch: launch.map(super::program::ParsedLaunch::Absolute),
            profile,
            options,
            client: None,
            focus: false,
            zoom: false,
            overlay: false,
            timeout_ms: 30_000,
        },
        format,
    }
}

fn label(value: &str) -> WorkbenchLabel {
    WorkbenchLabel::parse(value).expect("test label is protocol-valid")
}

fn directory_path(value: &str) -> WorkbenchDirectoryPath {
    WorkbenchDirectoryPath::parse(value).expect("test path is protocol-valid")
}

fn geometry() -> WorkbenchTerminalGeometry {
    WorkbenchTerminalGeometry::new(80, 24, 9, 18).expect("test geometry is protocol-valid")
}

const ID2: &str = "fedcba9876543210fedcba9876543210";
const GEOMETRY_ARGS: &[&str] = &[
    "--cols",
    "80",
    "--rows",
    "24",
    "--cell-width",
    "9",
    "--cell-height",
    "18",
];

fn tab_create_args(context: &str) -> Vec<&str> {
    let mut arguments = vec!["tab", "create"];
    arguments.extend(["--workspace", context]);
    arguments
}

#[test]
fn accepts_every_mutation_path() {
    let cases: &[(&[&str], MutationCommand)] = &[
        (
            &["project", "create", "--label", "api"],
            MutationCommand::ProjectCreate {
                label: label("api"),
            },
        ),
        (
            &["project", "rename", "web", "--label", "session"],
            MutationCommand::ProjectRename {
                target: Target::Label("web".into()),
                label: label("session"),
            },
        ),
        (
            &["project", "rename", ID, "--label", "session"],
            MutationCommand::ProjectRename {
                target: Target::Id(ID.into()),
                label: label("session"),
            },
        ),
        // A non-ID token — uppercase hex included — is a label candidate, not
        // an identifier failure; only empty/overlong/zero-shaped operands are.
        (
            &[
                "project",
                "rename",
                "0123456789ABCDEF0123456789ABCDEF",
                "--label",
                "session",
            ],
            MutationCommand::ProjectRename {
                target: Target::Label("0123456789ABCDEF0123456789ABCDEF".into()),
                label: label("session"),
            },
        ),
        (
            &["project", "rename", "--label", "session", "web"],
            MutationCommand::ProjectRename {
                target: Target::Label("web".into()),
                label: label("session"),
            },
        ),
        // `--` shields a flag-spelling operand: the literal label `--help`
        // still resolves through the shared target rules.
        (
            &["project", "rename", "--label", "session", "--", "--help"],
            MutationCommand::ProjectRename {
                target: Target::Label("--help".into()),
                label: label("session"),
            },
        ),
        (
            &["workspace", "rename", "main", "--label", "ops"],
            MutationCommand::WorkspaceRename {
                target: Target::Label("main".into()),
                label: label("ops"),
            },
        ),
        (
            &[
                "workspace",
                "accept-directory",
                "main",
                "--path",
                "/srv/repo",
            ],
            MutationCommand::WorkspaceAcceptDirectory {
                target: Target::Label("main".into()),
                path: directory_path("/srv/repo"),
            },
        ),
        (
            &["workspace", "accept-directory", ID, "--path", "/srv/repo"],
            MutationCommand::WorkspaceAcceptDirectory {
                target: Target::Id(ID.into()),
                path: directory_path("/srv/repo"),
            },
        ),
        (
            &["tab", "rename", ID, "--label", "renamed"],
            MutationCommand::TabRename {
                target: Target::Id(ID.into()),
                label: label("renamed"),
            },
        ),
        (
            &["tab", "move", "logs", "--order", "first"],
            MutationCommand::TabMove {
                target: Target::Label("logs".into()),
                direction: WorkbenchTabMoveDirection::First,
            },
        ),
        (
            &["tab", "close", ID, "--confirm"],
            MutationCommand::TabClose {
                target: Target::Id(ID.into()),
                wait: CloseWait::Wait { timeout_ms: 30_000 },
            },
        ),
        (
            &["panel", "close", ID, "--confirm"],
            MutationCommand::PanelClose {
                target: Target::Id(ID.into()),
            },
        ),
    ];
    for (arguments, command) in cases {
        assert_eq!(
            parse(arguments),
            Ok(mutation(command.clone(), OutputFormat::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn geometry_bearing_commands_require_the_whole_geometry() {
    let mut arguments = vec!["tab", "create", "--workspace", "main", "--label", "t"];
    arguments.extend(GEOMETRY_ARGS);
    assert_eq!(
        parse(&arguments),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: geometry(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );

    let mut arguments = vec!["panel", "split", ID, "--direction", "right", "--label", "p"];
    arguments.extend(GEOMETRY_ARGS);
    assert_eq!(
        parse(&arguments),
        Ok(mutation(
            MutationCommand::PanelSplit {
                right_click: Default::default(),
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                custom_label: None,
                geometry: geometry(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );

    // Every non-empty proper subset of the all-or-none group rejects the
    // whole command on both geometry-bearing commands — including `--rows`
    // alone, which keeps its geometry meaning here (never history's).
    for present in 1u8..0b1111 {
        for prefix in [
            vec!["tab", "create", "--workspace", "main", "--label", "t"],
            vec!["panel", "split", ID, "--direction", "right", "--label", "p"],
        ] {
            let mut arguments = prefix;
            for (bit, pair) in GEOMETRY_ARGS.chunks(2).enumerate() {
                if present & (1 << bit) != 0 {
                    arguments.extend(pair);
                }
            }
            assert_eq!(
                parse(&arguments),
                Err(usage(FailureEnvelope::Text)),
                "{arguments:?}"
            );
        }
    }
}

#[test]
fn geometry_bearing_commands_default_the_whole_geometry() {
    // All four flags absent requests the documented default: an 80x24
    // logical grid with the paired-zero (unknown) cell pixel pair.
    let default =
        WorkbenchTerminalGeometry::new(80, 24, 0, 0).expect("default geometry is protocol-valid");
    {
        let arguments = ["tab", "create", "--workspace", "main", "--label", "t"];
        let context = TabContext::Workspace(Target::Label("main".into()));
        assert_eq!(
            parse(&arguments),
            Ok(mutation(
                MutationCommand::TabCreate {
                    automatic_label: false,
                    context,
                    label: label("t"),
                    geometry: default,
                    launch: None,
                    profile: None,
                    options: Default::default(),
                },
                OutputFormat::Text
            )),
            "{arguments:?}"
        );
    }
    assert_eq!(
        parse(&["panel", "split", ID, "--direction", "right", "--label", "p"]),
        Ok(mutation(
            MutationCommand::PanelSplit {
                right_click: Default::default(),
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                custom_label: None,
                geometry: default,
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );

    // The parsed default is the request-bound value verbatim: the action
    // document carries the explicit 80x24@(0,0) geometry the protocol
    // already admits — the same wire form as explicit zero flags.
    assert_eq!(
        serde_json::to_value(default).expect("geometry serializes"),
        serde_json::json!({
            "columns": 80,
            "rows": 24,
            "cellPixelWidth": 0,
            "cellPixelHeight": 0,
        })
    );

    // The default composes with launch selection and output format
    // unchanged: --exec/--arg, --profile, and --format stay orthogonal.
    assert_eq!(
        parse(&[
            "tab",
            "create",
            "--workspace",
            "main",
            "--label",
            "t",
            "--exec",
            "/bin/sh",
            "--arg",
            "-lc",
        ]),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: default,
                launch: launch("/bin/sh", &["-lc"]),
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&[
            "tab",
            "create",
            "--workspace",
            "main",
            "--label",
            "t",
            "--profile",
            "dev",
            "--format",
            "json",
        ]),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: default,
                launch: None,
                profile: Some(profile_selector("dev")),
                options: Default::default(),
            },
            OutputFormat::Json
        ))
    );
}

#[test]
fn tab_create_admits_at_most_one_explicit_context() {
    let mut workspace = tab_create_args("main");
    workspace.extend(["--label", "t"]);
    workspace.extend(GEOMETRY_ARGS);
    assert_eq!(
        parse(&workspace),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: geometry(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );

    // Omitted context is resolved at runtime; explicit contexts stay exclusive.
    let mut neither = vec!["tab", "create", "--label", "t"];
    neither.extend(GEOMETRY_ARGS);
    assert!(matches!(
        parse(&neither),
        Ok(Request::Creation {
            command: CreationCommand {
                kind: CreationKind::Tab { context: None },
                ..
            },
            ..
        })
    ));

    let mut both = tab_create_args("main");
    both.extend(["--standalone", "--label", "t"]);
    both.extend(GEOMETRY_ARGS);
    assert_eq!(parse(&both), Err(usage(FailureEnvelope::Text)));

    let mut swapped = vec![
        "tab",
        "create",
        "--workspace",
        "main",
        "--workspace",
        "main",
    ];
    swapped.extend(["--label", "t"]);
    swapped.extend(GEOMETRY_ARGS);
    assert_eq!(parse(&swapped), Err(usage(FailureEnvelope::Text)));
}

fn launch(executable: &str, arguments: &[&str]) -> Option<LaunchSelection> {
    Some(
        LaunchSelection::new(
            executable.to_owned(),
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        )
        .expect("test launch selection is protocol-valid"),
    )
}

fn tab_create_invocation(launch_args: &[&str]) -> Vec<String> {
    let mut arguments: Vec<String> = ["tab", "create", "--workspace", "main", "--label", "t"]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
    arguments.extend(GEOMETRY_ARGS.iter().map(|word| (*word).to_owned()));
    arguments.extend(launch_args.iter().map(|word| (*word).to_owned()));
    arguments
}

fn tab_create_with(launch_args: &[&str]) -> Result<Request, ParseFailure> {
    let arguments = tab_create_invocation(launch_args);
    parse_request(arguments.iter().map(OsString::from))
}

#[test]
fn windows_launch_paths_preserve_arguments_and_reject_ambiguous_roots() {
    for executable in [
        r"C:\Program Files\工具\shell.exe",
        "C:/tools/shell.exe",
        r"\\?\C:\tools\shell.exe",
    ] {
        assert_eq!(
            tab_create_with(&["--exec", executable, "--arg", "two words", "--arg=中文"]),
            Ok(mutation(
                MutationCommand::TabCreate {
                    automatic_label: false,
                    context: TabContext::Workspace(Target::Label("main".into())),
                    label: label("t"),
                    geometry: geometry(),
                    launch: launch(executable, &["two words", "中文"]),
                    profile: None,
                    options: Default::default(),
                },
                OutputFormat::Text,
            )),
        );
    }
    for executable in [
        r"C:relative.exe",
        r"\tools\shell.exe",
        r"\\host\share\shell.exe",
        r"\\.\pipe\fake",
    ] {
        assert!(
            tab_create_with(&["--exec", executable]).is_err(),
            "{executable}"
        );
    }
}

#[test]
fn launch_selection_options_parse_in_every_spelling() {
    let expect = |launch_args: &[&str], arguments: &[&str]| {
        assert_eq!(
            tab_create_with(launch_args),
            Ok(mutation(
                MutationCommand::TabCreate {
                    automatic_label: false,
                    context: TabContext::Workspace(Target::Label("main".into())),
                    label: label("t"),
                    geometry: geometry(),
                    launch: launch("/bin/sh", arguments),
                    profile: None,
                    options: Default::default(),
                },
                OutputFormat::Text
            )),
            "{launch_args:?}"
        );
    };
    // `--exec` alone, both spellings.
    expect(&["--exec", "/bin/sh"], &[]);
    expect(&["--exec=/bin/sh"], &[]);
    // `--arg` preserves order verbatim: empty, dash-prefixed, and the literal
    // terminator via the equals spelling.
    expect(
        &["--exec", "/bin/sh", "--arg", "-c", "--arg=--", "--arg="],
        &["-c", "--", ""],
    );
    // A `--arg` value that spells a flag is consumed as a value.
    expect(&["--exec", "/bin/sh", "--arg", "--exec"], &["--exec"]);
    // `--arg` may precede `--exec` in argv order — the pair is positional-free.
    expect(&["--arg", "-l", "--exec", "/bin/sh"], &["-l"]);

    // panel split shares the selection.
    let mut split: Vec<String> = ["panel", "split", ID, "--direction", "right", "--label", "p"]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
    split.extend(GEOMETRY_ARGS.iter().map(|word| (*word).to_owned()));
    split.extend(
        ["--exec=/bin/sh", "--arg", "-l"]
            .iter()
            .map(|word| (*word).to_owned()),
    );
    assert_eq!(
        parse_request(split.iter().map(OsString::from)),
        Ok(mutation(
            MutationCommand::PanelSplit {
                right_click: Default::default(),
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                custom_label: None,
                geometry: geometry(),
                launch: launch("/bin/sh", &["-l"]),
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );
}

/// `--name value` and `--name=value` are the declared equivalent spellings
/// for every value option (command-surface Options.Values): the joined token
/// splits at the first `=` and the remainder is the verbatim value — embedded
/// `=`, leading `-`, spaces, and Unicode are all data. Equivalence covers the
/// produced invocation AND the failure class.
#[test]
fn joined_value_spelling_is_equivalent() {
    for (space, joined) in [
        // Selectors and filters across families.
        (
            &["workspace", "list", "--project", "proj"][..],
            &["workspace", "list", "--project=proj"][..],
        ),
        (
            &["tab", "list", "--workspace", "ws"][..],
            &["tab", "list", "--workspace=ws"][..],
        ),
        (
            &["panel", "list", "--tab", "t"][..],
            &["panel", "list", "--tab=t"][..],
        ),
        (
            &["worktree", "list", "--project", "p", "--directory", "d"][..],
            &["worktree", "list", "--project=p", "--directory=d"][..],
        ),
        (
            &["workspace", "create", "--project", "p", "--label", "l"][..],
            &["workspace", "create", "--project=p", "--label=l"][..],
        ),
        (
            &["workspace", "rename", "w", "--label", "x", "--confirm"][..],
            &["workspace", "rename", "w", "--label=x", "--confirm"][..],
        ),
        (
            &["workspace", "accept-directory", "w", "--path", "/srv/repo"][..],
            &["workspace", "accept-directory", "w", "--path=/srv/repo"][..],
        ),
        // Geometry values.
        (
            &[
                "tab",
                "create",
                "--workspace",
                "w",
                "--label",
                "t",
                "--cols",
                "80",
            ][..],
            &["tab", "create", "--workspace=w", "--label=t", "--cols=80"][..],
        ),
        // Direction, name, launch selection.
        (
            &["tab", "move", "t", "--order", "next"][..],
            &["tab", "move", "t", "--order=next"][..],
        ),
        (
            &[
                "panel",
                "move",
                "p",
                "--target-panel",
                "q",
                "--direction",
                "left",
            ][..],
            &["panel", "move", "p", "--target-panel=q", "--direction=left"][..],
        ),
        (
            &[
                "launch-profile",
                "create",
                "--name",
                "n",
                "--exec",
                "/bin/sh",
            ][..],
            &["launch-profile", "create", "--name=n", "--exec=/bin/sh"][..],
        ),
        // Terminal send sources and modifiers.
        (
            &["terminal", "send", ID, "--text", "hi", "--enter"][..],
            &["terminal", "send", ID, "--text=hi", "--enter"][..],
        ),
        (
            &["terminal", "send", ID, "--key", "enter"][..],
            &["terminal", "send", ID, "--key=enter"][..],
        ),
        (
            &["terminal", "send", ID, "--file", "/tmp/x"][..],
            &["terminal", "send", ID, "--file=/tmp/x"][..],
        ),
        (
            &[
                "terminal",
                "wait-output",
                ID,
                "--match",
                "x",
                "--timeout",
                "5",
            ][..],
            &["terminal", "wait-output", ID, "--match=x", "--timeout=5"][..],
        ),
        (
            &["terminal", "read", ID, "--lines", "10"][..],
            &["terminal", "read", ID, "--lines=10"][..],
        ),
        // Agent families: paging, wait conditions, doctor selectors,
        // integration flags.
        (
            &["agent", "list", "--limit", "10", "--cursor", "c"][..],
            &["agent", "list", "--limit=10", "--cursor=c"][..],
        ),
        (
            &["agent", "wait", "a", "--until", "idle", "--timeout", "30"][..],
            &["agent", "wait", "a", "--until=idle", "--timeout=30"][..],
        ),
        (
            &["agent", "wait", "a", "--after-revision", "7"][..],
            &["agent", "wait", "a", "--after-revision=7"][..],
        ),
        (
            &[
                "agent",
                "doctor",
                "--provider",
                "codex",
                "--asset-root",
                &doctor_test_root("/r"),
            ][..],
            &[
                "agent",
                "doctor",
                "--provider=codex",
                &format!("--asset-root={}", doctor_test_root("/r")),
            ][..],
        ),
        (
            &[
                "agent",
                "integration",
                "list",
                "--provider",
                "pi",
                "--adapter-source",
                "s",
            ][..],
            &[
                "agent",
                "integration",
                "list",
                "--provider=pi",
                "--adapter-source=s",
            ][..],
        ),
        // Repeatables accumulate identically in either spelling, mixed freely.
        (
            &[
                "tab",
                "create",
                "--workspace",
                "main",
                "--exec",
                "/x",
                "--arg",
                "a",
                "--arg",
                "b",
            ][..],
            &[
                "tab",
                "create",
                "--workspace",
                "main",
                "--exec=/x",
                "--arg=a",
                "--arg=b",
            ][..],
        ),
        (
            &["agent", "wait", "a", "--until", "idle", "--until", "failed"][..],
            &["agent", "wait", "a", "--until=idle", "--until=failed"][..],
        ),
        // Value text that embeds `=` or begins with a dash stays verbatim.
        (
            &["terminal", "send", ID, "--text", "a=b=c"][..],
            &["terminal", "send", ID, "--text=a=b=c"][..],
        ),
        (
            &["terminal", "send", ID, "--text", "--help"][..],
            &["terminal", "send", ID, "--text=--help"][..],
        ),
        (
            &["workspace", "list", "--project", "--format"][..],
            &["workspace", "list", "--project=--format"][..],
        ),
        (
            &["terminal", "send", ID, "--text", "值 "][..],
            &["terminal", "send", ID, "--text=值 "][..],
        ),
        // Scope failures are identical in either spelling.
        (
            &["workspace", "list", "--label", "x"][..],
            &["workspace", "list", "--label=x"][..],
        ),
        (
            &["project", "list", "--workspace", "w"][..],
            &["project", "list", "--workspace=w"][..],
        ),
        // Spelling-blind duplicates.
        (
            &["workspace", "list", "--project", "a", "--project=b"][..],
            &["workspace", "list", "--project=a", "--project", "b"][..],
        ),
        // Empty and invalid values resolve identically.
        (
            &[
                "tab",
                "create",
                "--workspace",
                "w",
                "--label",
                "t",
                "--cols",
                "",
            ][..],
            &["tab", "create", "--workspace=w", "--label=t", "--cols="][..],
        ),
        (
            &["workspace", "list", "--project", ""][..],
            &["workspace", "list", "--project="][..],
        ),
        (
            &["terminal", "wait-output", ID, "--timeout", "bogus"][..],
            &["terminal", "wait-output", ID, "--timeout=bogus"][..],
        ),
    ] {
        assert_eq!(parse(space), parse(joined), "{space:?} vs {joined:?}");
    }
    // `--mouse` admits the joined spelling too, but its value stays the
    // closed {off,on} vocabulary validated by its own arm: unknown or
    // empty values are the same InvalidValue failure as the space form,
    // duplicates are spelling-blind, and the route scope is unchanged.
    for (space, joined) in [
        (&["tui", "--mouse", "off"][..], &["tui", "--mouse=off"][..]),
        (&["tui", "--mouse", "on"][..], &["tui", "--mouse=on"][..]),
        (
            &["tui", "--mouse", "bogus"][..],
            &["tui", "--mouse=bogus"][..],
        ),
        (&["tui", "--mouse", ""][..], &["tui", "--mouse="][..]),
        (
            &["tui", "--mouse", "off", "--mouse", "on"][..],
            &["tui", "--mouse=off", "--mouse=on"][..],
        ),
        // Off the `tui` route both spellings fail identically.
        (&["--mouse", "off"][..], &["--mouse=off"][..]),
        (
            &["status", "--mouse", "off"][..],
            &["status", "--mouse=off"][..],
        ),
        (
            &["tui", "--", "--mouse", "off"][..],
            &["tui", "--", "--mouse=off"][..],
        ),
    ] {
        assert_eq!(parse(space), parse(joined), "{space:?} vs {joined:?}");
    }
    // A joined token after `--` is a positional word, never an option —
    // `status` takes no operand, so the word is an unexpected argument
    // rather than a project filter.
    for arguments in [
        &["status", "--", "--project=x"][..],
        &["--", "status", "--project=x"][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn launch_selection_options_fail_closed_on_misuse() {
    let usage = |launch_args: &[&str]| {
        assert_eq!(
            tab_create_with(launch_args),
            Err(self::usage(FailureEnvelope::Text)),
            "{launch_args:?}"
        );
    };
    // `--arg` without `--exec` is a missing requirement, not an empty program.
    usage(&["--arg", "-c"]);
    usage(&["--arg=--"]);
    // `--exec` is single-use in either spelling.
    usage(&["--exec", "/bin/sh", "--exec", "/bin/zsh"]);
    usage(&["--exec=/bin/sh", "--exec=/bin/zsh"]);
    usage(&["--exec", "/bin/sh", "--exec=/bin/zsh"]);
    // Missing values keep the existing conventions: `--exec` at end and
    // `--exec --` are both missing values.
    usage(&["--exec"]);
    usage(&["--exec", "--"]);
    usage(&["--arg"]);
    // Empty `--exec=` is an invalid executable, not a missing value.
    usage(&["--exec="]);
    // Bare names stay unresolved until the final CLI; relative paths are invalid.
    assert!(
        matches!(tab_create_with(&["--exec", "sh"]), Ok(Request::Creation {
        command: CreationCommand { launch: Some(super::program::ParsedLaunch::Path { name, .. }), .. }, ..
    }) if name == "sh")
    );
    usage(&["--exec", "bin/sh"]);
    // NUL-bearing values cannot arrive through &str argv — NUL input is
    // exercised through OsString at the byte envelope instead.

    // The pair is scoped: unrelated commands reject both spellings.
    for arguments in [
        &["tab", "rename", ID, "--label", "x", "--exec", "/bin/sh"][..],
        &["tab", "close", ID, "--confirm", "--arg", "x"][..],
        &["terminal", "send", ID, "--text", "x", "--exec=/bin/sh"][..],
        &[
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "x",
            "--arg=v",
        ][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(self::usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn launch_selection_enforces_count_and_byte_bounds() {
    // 128 arguments admit; the 129th is a usage failure.
    let mut arguments = tab_create_invocation(&["--exec", "/bin/sh"]);
    for _ in 0..128 {
        arguments.push("--arg".to_owned());
        arguments.push("x".to_owned());
    }
    assert!(parse_request(arguments.iter().map(OsString::from)).is_ok());
    arguments.extend(["--arg".to_owned(), "x".to_owned()]);
    assert_eq!(
        parse_request(arguments.iter().map(OsString::from)),
        Err(usage(FailureEnvelope::Text))
    );

    // The protocol's enlarged resume-command budget governs both raw UTF-8 and
    // escaped JSON string contents; invocation must refuse either overflow.
    let available = orchbee_workbench_protocol::MAXIMUM_LAUNCH_SELECTION_BYTES - "/bin/sh".len();
    for (character, count) in [('a', available), ('"', available / 2)] {
        let within = tab_create_invocation(&[
            "--exec",
            "/bin/sh",
            "--arg",
            &character.to_string().repeat(count),
        ]);
        assert!(parse_request(within.iter().map(OsString::from)).is_ok());
        let over = tab_create_invocation(&[
            "--exec",
            "/bin/sh",
            "--arg",
            &character.to_string().repeat(count + 1),
        ]);
        assert_eq!(
            parse_request(over.iter().map(OsString::from)),
            Err(usage(FailureEnvelope::Text))
        );
    }
}

fn profile_selector(value: &str) -> LaunchProfileSelector {
    LaunchProfileSelector::parse(value).expect("test selector is protocol-valid")
}

/// `launch-profile list|get|create|delete`: the four paths, the verbatim
/// selector operand, and the required `--name`/`--exec` pair on create.
#[test]
fn launch_profile_grammar() {
    assert_eq!(
        parse(&["launch-profile", "list"]),
        Ok(query(QueryCommand::LaunchProfileList, OutputFormat::Text))
    );
    // The selector is verbatim: ID-shaped, name-shaped, and spaced names all
    // pass through as the same opaque operand resolved Server-side.
    for selector in ["abc123:0001a", "dev", "dev shell"] {
        assert_eq!(
            parse(&["launch-profile", "get", selector]),
            Ok(query(
                QueryCommand::LaunchProfileGet(profile_selector(selector)),
                OutputFormat::Text
            )),
            "{selector:?}"
        );
        assert_eq!(
            parse(&["launch-profile", "delete", selector]),
            Ok(mutation(
                MutationCommand::LaunchProfileDelete {
                    selector: profile_selector(selector),
                },
                OutputFormat::Text
            )),
            "{selector:?}"
        );
    }
    assert_eq!(
        parse(&[
            "launch-profile",
            "create",
            "--name",
            "dev shell",
            "--exec",
            "/bin/sh",
            "--arg",
            "-l",
        ]),
        Ok(mutation(
            MutationCommand::LaunchProfileCreate {
                name: LaunchProfileName::parse("dev shell").expect("valid name"),
                launch: launch("/bin/sh", &["-l"]).expect("valid launch"),
            },
            OutputFormat::Text
        ))
    );
    // Option order is free inside the command scope; `--exec`/`--arg` keep
    // their equals spellings while `--name` only takes the two-token form.
    assert_eq!(
        parse(&[
            "launch-profile",
            "create",
            "--arg=-l",
            "--exec=/bin/sh",
            "--name",
            "dev",
        ]),
        Ok(mutation(
            MutationCommand::LaunchProfileCreate {
                name: LaunchProfileName::parse("dev").expect("valid name"),
                launch: launch("/bin/sh", &["-l"]).expect("valid launch"),
            },
            OutputFormat::Text
        ))
    );
}

#[test]
fn launch_profile_grammar_fails_closed_on_misuse() {
    let usage = |arguments: &[&str]| {
        assert_eq!(
            parse(arguments),
            Err(self::usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    };
    // Missing verb, unknown verb, missing/extra operands.
    usage(&["launch-profile"]);
    usage(&["launch-profile", "rename", "x"]);
    usage(&["launch-profile", "get"]);
    usage(&["launch-profile", "get", "a", "b"]);
    usage(&["launch-profile", "delete"]);
    usage(&["launch-profile", "delete", "a", "b"]);
    usage(&["launch-profile", "list", "x"]);
    usage(&["launch-profile", "create", "x"]);
    // Empty and overlong selectors are invalid values.
    usage(&["launch-profile", "get", ""]);
    let overlong = "a".repeat(513);
    usage(&["launch-profile", "get", &overlong]);
    usage(&["launch-profile", "delete", &overlong]);
    // Out-of-scope options fail the same generic way.
    usage(&["launch-profile", "list", "--name", "x"]);
    usage(&["launch-profile", "get", "a", "--profile", "x"]);
    usage(&["launch-profile", "delete", "a", "--exec", "/bin/sh"]);
    usage(&["launch-profile", "delete", "a", "--arg", "x"]);
    // Create requires both `--name` and `--exec`; each is single-use.
    usage(&["launch-profile", "create", "--exec", "/bin/sh"]);
    usage(&["launch-profile", "create", "--name", "n"]);
    usage(&[
        "launch-profile",
        "create",
        "--name",
        "n",
        "--exec",
        "/bin/sh",
        "--name",
        "m",
    ]);
    usage(&[
        "launch-profile",
        "create",
        "--name",
        "n",
        "--exec",
        "/bin/sh",
        "--exec=/bin/zsh",
    ]);
    // Empty/overlong names and non-absolute executables are invalid values.
    usage(&[
        "launch-profile",
        "create",
        "--name",
        "",
        "--exec",
        "/bin/sh",
    ]);
    usage(&[
        "launch-profile",
        "create",
        "--name",
        &overlong,
        "--exec",
        "/bin/sh",
    ]);
    usage(&["launch-profile", "create", "--name", "n", "--exec", "sh"]);
    // `--arg` without `--exec` is a missing requirement on create too.
    usage(&["launch-profile", "create", "--name", "n", "--arg", "-l"]);
    // `--profile` is scoped to the launch-bearing commands only.
    usage(&[
        "launch-profile",
        "create",
        "--name",
        "n",
        "--exec",
        "/bin/sh",
        "--profile",
        "p",
    ]);
    usage(&["launch-profile", "list", "--profile", "p"]);
    usage(&["tab", "rename", ID, "--label", "x", "--profile", "p"]);
    // `--name` is scoped to create only.
    let mut scoped = tab_create_invocation(&[]);
    scoped.extend(["--name".to_owned(), "n".to_owned()]);
    assert_eq!(
        parse_request(scoped.iter().map(OsString::from)),
        Err(self::usage(FailureEnvelope::Text))
    );
}

/// `--profile <selector>` threads into `tab create`/`panel split` and is
/// mutually exclusive with the explicit `--exec`/`--arg` selection.
#[test]
fn profile_option_threads_into_launch_bearing_mutations() {
    let mut arguments = tab_create_invocation(&["--profile", "dev shell"]);
    assert_eq!(
        parse_request(arguments.iter().map(OsString::from)),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: geometry(),
                launch: None,
                profile: Some(profile_selector("dev shell")),
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );
    // ID-shaped selectors pass through the same two-token form.
    arguments = tab_create_invocation(&["--profile", "abc123:0001a"]);
    assert_eq!(
        parse_request(arguments.iter().map(OsString::from)),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: geometry(),
                launch: None,
                profile: Some(profile_selector("abc123:0001a")),
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );

    let mut split: Vec<String> = ["panel", "split", ID, "--direction", "right", "--label", "p"]
        .iter()
        .map(|word| (*word).to_owned())
        .collect();
    split.extend(GEOMETRY_ARGS.iter().map(|word| (*word).to_owned()));
    split.push("--profile".to_owned());
    split.push("dev".to_owned());
    assert_eq!(
        parse_request(split.iter().map(OsString::from)),
        Ok(mutation(
            MutationCommand::PanelSplit {
                right_click: Default::default(),
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                custom_label: None,
                geometry: geometry(),
                launch: None,
                profile: Some(profile_selector("dev")),
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );

    // `--profile` rejects alongside either explicit-launch flag.
    for extra in [
        &["--profile", "p", "--exec", "/bin/sh"][..],
        &["--exec", "/bin/sh", "--profile", "p"][..],
        &["--profile", "p", "--arg", "-l"][..],
        &["--arg", "-l", "--profile", "p"][..],
        // `--profile` is single-use in either spelling.
        &["--profile", "p", "--profile", "q"][..],
        &["--profile", "p", "--profile=q"][..],
        &["--profile=p", "--profile", "q"][..],
    ] {
        let arguments = tab_create_invocation(extra);
        assert_eq!(
            parse_request(arguments.iter().map(OsString::from)),
            Err(usage(FailureEnvelope::Text)),
            "{extra:?}"
        );
        let mut split: Vec<String> = ["panel", "split", ID, "--direction", "right", "--label", "p"]
            .iter()
            .map(|word| (*word).to_owned())
            .collect();
        split.extend(GEOMETRY_ARGS.iter().map(|word| (*word).to_owned()));
        split.extend(extra.iter().map(|word| (*word).to_owned()));
        assert_eq!(
            parse_request(split.iter().map(OsString::from)),
            Err(usage(FailureEnvelope::Text)),
            "{extra:?}"
        );
    }
}

#[test]
fn launch_profile_help_paths_resolve() {
    for (arguments, topic) in [
        (
            &["help", "launch-profile"][..],
            HelpTopic::Family(Family::LaunchProfile),
        ),
        (
            &["help", "launch-profile", "list"][..],
            HelpTopic::Command(CommandPath::LaunchProfileList),
        ),
        (
            &["help", "launch-profile", "get"][..],
            HelpTopic::Command(CommandPath::LaunchProfileGet),
        ),
        (
            &["help", "launch-profile", "create"][..],
            HelpTopic::Command(CommandPath::LaunchProfileCreate),
        ),
        (
            &["help", "launch-profile", "delete"][..],
            HelpTopic::Command(CommandPath::LaunchProfileDelete),
        ),
        (
            &["launch-profile", "--help"][..],
            HelpTopic::Family(Family::LaunchProfile),
        ),
        (
            &["launch-profile", "list", "--help"][..],
            HelpTopic::Command(CommandPath::LaunchProfileList),
        ),
        (
            &["launch-profile", "create", "--help"][..],
            HelpTopic::Command(CommandPath::LaunchProfileCreate),
        ),
    ] {
        assert_eq!(parse(arguments), Ok(Request::Help(topic)), "{arguments:?}");
    }
}

#[test]
fn close_commands_require_explicit_confirm() {
    for arguments in [
        &["tab", "close", ID][..],
        &["panel", "close", ID][..],
        // --confirm is scoped to the close commands only.
        &["tab", "rename", ID, "--label", "x", "--confirm"][..],
        &["workspace", "rename", "w", "--label", "x", "--confirm"][..],
        &["tab", "move", ID, "--order", "next", "--confirm"][..],
        &[
            "panel",
            "move",
            ID,
            "--target-panel",
            ID2,
            "--direction",
            "left",
            "--confirm",
        ][..],
        &["status", "--confirm"][..],
        // The flag is single-use.
        &["tab", "close", ID, "--confirm", "--confirm"][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn mutation_direction_values_follow_the_command_family() {
    for (word, direction) in [
        ("previous", WorkbenchTabMoveDirection::Previous),
        ("next", WorkbenchTabMoveDirection::Next),
        ("first", WorkbenchTabMoveDirection::First),
        ("last", WorkbenchTabMoveDirection::Last),
    ] {
        let arguments = ["tab", "move", ID, "--order", word];
        assert_eq!(
            parse(&arguments),
            Ok(mutation(
                MutationCommand::TabMove {
                    target: Target::Id(ID.into()),
                    direction,
                },
                OutputFormat::Text
            )),
            "{arguments:?}"
        );
    }
    for (word, direction) in [
        ("up", WorkbenchPanelSplitDirection::Above),
        ("down", WorkbenchPanelSplitDirection::Below),
        ("left", WorkbenchPanelSplitDirection::Left),
        ("right", WorkbenchPanelSplitDirection::Right),
    ] {
        let arguments = [
            "panel",
            "move",
            ID,
            "--into-tab",
            "target-tab",
            "--target-panel",
            ID2,
            "--direction",
            word,
        ];
        assert!(
            matches!(parse(&arguments), Ok(Request::PanelRelocation { command, .. })
            if matches!(command.destination, PanelRelocationDestination::Tab { direction: actual, .. } if actual == direction))
        );
    }
    // Cross-family direction words are usage, not a nearest match.
    for arguments in [
        &["tab", "move", ID, "--order", "above"][..],
        &["tab", "move", ID, "--order", "right"][..],
        &[
            "panel",
            "move",
            ID,
            "--into-tab",
            "target-tab",
            "--target-panel",
            ID2,
            "--direction",
            "next",
        ][..],
        &[
            "panel",
            "move",
            ID,
            "--into-tab",
            "target-tab",
            "--target-panel",
            ID2,
            "--direction",
            "sideways",
        ][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
}

#[test]
fn mutation_required_options_cannot_be_dropped() {
    let mut full_split = vec!["panel", "split", ID, "--direction", "right", "--label", "p"];
    full_split.extend(GEOMETRY_ARGS);
    let cases: &[&[&str]] = &[
        &["project", "create"],
        &["workspace", "rename", "w"],
        &["workspace", "accept-directory", "w"],
        &["tab", "rename", "t"],
        &["tab", "move", "t"],
        &["panel", "split", ID, "--label", "p"],
        &["panel", "move", ID, "--direction", "left"],
        &["panel", "move", ID, "--target-panel", ID2],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    assert_eq!(
        parse(&full_split),
        Ok(mutation(
            MutationCommand::PanelSplit {
                right_click: Default::default(),
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                custom_label: None,
                geometry: geometry(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );
}

#[test]
fn mutation_options_stay_inside_their_command_scope() {
    let mut create = vec!["workspace", "create", "--project", "p", "--label", "x"];
    let cases: &[&[&str]] = &[
        // Query-only filters are not mutation flags, and vice versa.
        &["project", "create", "--label", "x", "--project", "p"],
        &["project", "create", "--label", "x", "--standalone"],
        &["project", "create", "--label", "x", "--confirm"],
        &[
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "x",
            "--standalone",
        ],
        &[
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "x",
            "--tab",
            "t",
        ],
        &["workspace", "rename", "w", "--label", "x", "--project", "p"],
        &[
            "workspace",
            "rename",
            "w",
            "--label",
            "x",
            "--path",
            "/srv/repo",
        ],
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/srv/repo",
            "--label",
            "x",
        ],
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/srv/repo",
            "--project",
            "p",
        ],
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/srv/repo",
            "--confirm",
        ],
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/srv/repo",
            "--standalone",
        ],
        &["tab", "rename", "t", "--label", "x", "--workspace", "w"],
        &["tab", "move", "t", "--order", "next", "--label", "x"],
        &[
            "tab",
            "move",
            "t",
            "--direction",
            "next",
            "--workspace",
            "w",
        ],
        &["tab", "close", "t", "--confirm", "--label", "x"],
        &[
            "panel",
            "split",
            ID,
            "--direction",
            "right",
            "--label",
            "p",
            "--tab",
            "t",
        ],
        &[
            "panel",
            "move",
            ID,
            "--target-panel",
            ID2,
            "--direction",
            "left",
            "--label",
            "x",
        ],
        &[
            "panel",
            "move",
            ID,
            "--target-panel",
            ID2,
            "--direction",
            "left",
            "--standalone",
        ],
        // Mutation flags on queries or the bare invocation are usage.
        &["status", "--label", "x"],
        &["status", "--direction", "next"],
        &["project", "list", "--confirm"],
        &["tab", "list", "--direction", "next"],
        &["tab", "list", "--cols", "80"],
        &["panel", "get", ID, "--label", "x"],
        &["--label", "x"],
        &["--confirm"],
        &["--direction", "next"],
        &[
            "--cols",
            "80",
            "--rows",
            "24",
            "--cell-width",
            "9",
            "--cell-height",
            "18",
        ],
        &["--target-panel", ID],
        &["--path", "/srv/repo"],
        &["status", "--path", "/srv/repo"],
        &["workspace", "list", "--path", "/srv/repo"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // The legal form preserves creation intent on every host.
    assert!(matches!(
        parse(&create),
        Ok(Request::WorkspaceCreation { .. })
    ));
    create.push("--format");
    create.push("json");
    assert!(matches!(
        parse(&create),
        Ok(Request::WorkspaceCreation {
            format: OutputFormat::Json,
            ..
        })
    ));
}

#[test]
fn mutation_values_are_bounded() {
    // Labels: empty and overlong are usage; the protocol maximum passes.
    let overlong = "x".repeat(513);
    for arguments in [
        &["project", "create", "--label", ""][..],
        &["project", "create", "--label", overlong.as_str()][..],
        &["workspace", "rename", "w", "--label", ""][..],
        &["workspace", "rename", "w", "--label", overlong.as_str()][..],
        &["workspace", "create", "--project", "p", "--label", ""][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    let maximum = "x".repeat(512);
    assert_eq!(
        parse(&["workspace", "rename", "w", "--label", maximum.as_str()]),
        Ok(mutation(
            MutationCommand::WorkspaceRename {
                target: Target::Label("w".into()),
                label: label(maximum.as_str()),
            },
            OutputFormat::Text
        ))
    );
    // A dash-leading label is still a value, and equals spellings are rejected.
    assert_eq!(
        parse(&["workspace", "rename", "w", "--label", "--format"]),
        Ok(mutation(
            MutationCommand::WorkspaceRename {
                target: Target::Label("w".into()),
                label: label("--format"),
            },
            OutputFormat::Text
        ))
    );
    // The joined spelling is the same option: `--label=x` parses exactly as
    // `--label x`.
    assert_eq!(
        parse(&["workspace", "rename", "w", "--label=x"]),
        Ok(mutation(
            MutationCommand::WorkspaceRename {
                target: Target::Label("w".into()),
                label: label("x"),
            },
            OutputFormat::Text
        ))
    );

    // Geometry: u16 shape plus the protocol's product bounds.
    for (cols, rows, width, height) in [
        ("0", "24", "9", "18"),
        ("80", "0", "9", "18"),
        ("80", "24", "0", "18"),
        ("80", "24", "9", "0"),
        ("65536", "24", "9", "18"),
        ("80", "24", "9", "-1"),
        ("80", "24", "9", "wide"),
        // 4096*4096 grid cells and 4096*4096 pixel extents overflow.
        ("4096", "4096", "9", "18"),
        ("80", "24", "4096", "4096"),
        ("80", "24", "17", "4096"),
    ] {
        let arguments = [
            "tab",
            "create",
            "--workspace",
            "main",
            "--label",
            "t",
            "--cols",
            cols,
            "--rows",
            rows,
            "--cell-width",
            width,
            "--cell-height",
            height,
        ];
        assert_eq!(
            parse(&arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // The extreme legal geometry parses: 512*512 is the grid bound and both
    // pixel extents stay under the 65535 cap.
    let arguments = [
        "tab",
        "create",
        "--workspace",
        "main",
        "--label",
        "t",
        "--cols",
        "512",
        "--rows",
        "512",
        "--cell-width",
        "64",
        "--cell-height",
        "63",
    ];
    assert_eq!(
        parse(&arguments),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: WorkbenchTerminalGeometry::new(512, 512, 64, 63)
                    .expect("geometry inside bounds"),
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );
    // A paired-zero cell-pixel geometry is the explicit unknown-pixel form
    // and parses like any other legal geometry.
    let arguments = [
        "tab",
        "create",
        "--workspace",
        "main",
        "--label",
        "t",
        "--cols",
        "80",
        "--rows",
        "24",
        "--cell-width",
        "0",
        "--cell-height",
        "0",
    ];
    assert_eq!(
        parse(&arguments),
        Ok(mutation(
            MutationCommand::TabCreate {
                automatic_label: false,
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0)
                    .expect("paired-zero geometry is protocol-valid"),
                launch: None,
                profile: None,
                options: Default::default(),
            },
            OutputFormat::Text
        ))
    );
}

#[test]
fn accept_directory_path_is_a_literal_protocol_value() {
    // Relative, empty, NUL-bearing, and overlong values are usage failures
    // at the CLI boundary; '~'/'$' lead no special expansion.
    let overlong = format!("/{}", "x".repeat(4096));
    for arguments in [
        &["workspace", "accept-directory", "w", "--path", ""][..],
        &["workspace", "accept-directory", "w", "--path", "srv/repo"][..],
        &["workspace", "accept-directory", "w", "--path", "~/repo"][..],
        &["workspace", "accept-directory", "w", "--path", "$HOME/repo"][..],
        &["workspace", "accept-directory", "w", "--path", "/a\0b"][..],
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            overlong.as_str(),
        ][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // The protocol maximum (4,096 UTF-8 bytes including the leading slash)
    // parses, and a repeated or equals-spelled flag is usage.
    let maximum = format!("/{}", "x".repeat(4095));
    assert_eq!(
        parse(&[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            maximum.as_str()
        ]),
        Ok(mutation(
            MutationCommand::WorkspaceAcceptDirectory {
                target: Target::Label("w".into()),
                path: directory_path(maximum.as_str()),
            },
            OutputFormat::Text
        ))
    );
    // A literal path that merely contains '~' or '$' parses unchanged — the
    // CLI never expands anything inside the value.
    for literal in [
        "/~/repo",
        "/srv/$HOME/repo",
        "/srv/r$epo",
        r"C:\Users\工程 项目",
        "D:/repo",
    ] {
        assert_eq!(
            parse(&["workspace", "accept-directory", "w", "--path", literal]),
            Ok(mutation(
                MutationCommand::WorkspaceAcceptDirectory {
                    target: Target::Label("w".into()),
                    path: directory_path(literal),
                },
                OutputFormat::Text
            )),
            "{literal:?}"
        );
    }
    for arguments in [
        &[
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/a",
            "--path",
            "/b",
        ][..],
        &["workspace", "accept-directory", "w", "--path"][..],
        &["workspace", "accept-directory", "w", "--path", "--"][..],
        &["workspace", "accept-directory"][..],
        &[
            "workspace",
            "accept-directory",
            "w",
            "extra",
            "--path",
            "/a",
        ][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // The workspace target follows the ordinary rule: typed ID or exact
    // label; a zero ID shape is an identifier failure, and a valid
    // --format json selection parses.
    assert_eq!(
        parse(&["workspace", "accept-directory", ZERO, "--path", "/a"]),
        Err(identifier(Resource::Workspace, ZERO, FailureEnvelope::Text))
    );
    assert_eq!(
        parse(&[
            "--format=json",
            "workspace",
            "accept-directory",
            "w",
            "--path",
            "/a"
        ]),
        Ok(mutation(
            MutationCommand::WorkspaceAcceptDirectory {
                target: Target::Label("w".into()),
                path: directory_path("/a"),
            },
            OutputFormat::Json
        ))
    );
}

#[test]
fn mutation_targets_follow_each_resource_rule() {
    // Label-capable resources take ids or exact labels on their operand.
    assert_eq!(
        parse(&["workspace", "create", "--project", ZERO, "--label", "x"]),
        Err(identifier(Resource::Project, ZERO, FailureEnvelope::Text))
    );
    assert_eq!(
        parse(&["tab", "rename", ZERO, "--label", "x"]),
        Err(identifier(Resource::Tab, ZERO, FailureEnvelope::Text))
    );
    assert_eq!(
        parse(&["tab", "close", "not-a-panel", "--confirm"]),
        Ok(mutation(
            MutationCommand::TabClose {
                target: Target::Label("not-a-panel".into()),
                wait: CloseWait::Wait { timeout_ms: 30_000 },
            },
            OutputFormat::Text
        ))
    );
    // Panels accept only complete typed ids — operands and --target-panel alike.
    for (arguments, resource, value) in [
        (
            &[
                "panel",
                "split",
                "short",
                "--direction",
                "right",
                "--label",
                "p",
            ][..],
            Resource::Panel,
            "short",
        ),
        (
            &["panel", "close", ZERO, "--confirm"][..],
            Resource::Panel,
            ZERO,
        ),
        (
            &[
                "panel",
                "move",
                ID,
                "--target-panel",
                "by-label",
                "--into-tab",
                ID2,
                "--direction",
                "left",
            ][..],
            Resource::Panel,
            "by-label",
        ),
        (
            &[
                "panel",
                "move",
                ID,
                "--target-panel",
                ZERO,
                "--into-tab",
                ID2,
                "--direction",
                "left",
            ][..],
            Resource::Panel,
            ZERO,
        ),
    ] {
        assert_eq!(
            parse(arguments),
            Err(identifier(resource, value, FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // --project/--workspace on mutation commands are full targets too.
    assert_eq!(
        parse(&[
            "tab",
            "create",
            "--workspace",
            ZERO,
            "--label",
            "t",
            "--cols",
            "80",
            "--rows",
            "24",
            "--cell-width",
            "9",
            "--cell-height",
            "18",
        ]),
        Err(identifier(Resource::Workspace, ZERO, FailureEnvelope::Text))
    );
}

#[test]
fn mutation_help_paths_resolve_both_forms() {
    let topics: &[(&[&str], HelpTopic)] = &[
        (
            &["workspace", "create", "--help"],
            HelpTopic::Command(CommandPath::WorkspaceCreate),
        ),
        (
            &["workspace", "rename", "--help"],
            HelpTopic::Command(CommandPath::WorkspaceRename),
        ),
        (
            &["workspace", "accept-directory", "--help"],
            HelpTopic::Command(CommandPath::WorkspaceAcceptDirectory),
        ),
        (
            &["tab", "create", "--help"],
            HelpTopic::Command(CommandPath::TabCreate),
        ),
        (
            &["tab", "rename", "--help"],
            HelpTopic::Command(CommandPath::TabRename),
        ),
        (
            &["tab", "move", "--help"],
            HelpTopic::Command(CommandPath::TabMove),
        ),
        (
            &["tab", "close", "--help"],
            HelpTopic::Command(CommandPath::TabClose),
        ),
        (
            &["panel", "split", "--help"],
            HelpTopic::Command(CommandPath::PanelSplit),
        ),
        (
            &["panel", "close", "--help"],
            HelpTopic::Command(CommandPath::PanelClose),
        ),
        (
            &["panel", "move", "--help"],
            HelpTopic::Command(CommandPath::PanelMove),
        ),
        (
            &["help", "workspace", "create"],
            HelpTopic::Command(CommandPath::WorkspaceCreate),
        ),
        (
            &["help", "workspace", "rename"],
            HelpTopic::Command(CommandPath::WorkspaceRename),
        ),
        (
            &["help", "workspace", "accept-directory"],
            HelpTopic::Command(CommandPath::WorkspaceAcceptDirectory),
        ),
        (
            &["help", "tab", "create"],
            HelpTopic::Command(CommandPath::TabCreate),
        ),
        (
            &["help", "tab", "rename"],
            HelpTopic::Command(CommandPath::TabRename),
        ),
        (
            &["help", "tab", "move"],
            HelpTopic::Command(CommandPath::TabMove),
        ),
        (
            &["help", "tab", "close"],
            HelpTopic::Command(CommandPath::TabClose),
        ),
        (
            &["help", "panel", "split"],
            HelpTopic::Command(CommandPath::PanelSplit),
        ),
        (
            &["help", "panel", "close"],
            HelpTopic::Command(CommandPath::PanelClose),
        ),
        (
            &["help", "panel", "move"],
            HelpTopic::Command(CommandPath::PanelMove),
        ),
    ];
    for (arguments, topic) in topics {
        assert_eq!(parse(arguments), Ok(Request::Help(*topic)), "{arguments:?}");
    }
    // Help never executes mutations, even when destructive options are supplied.
    for (arguments, topic) in [
        (
            &["workspace", "create", "--project", "p", "--help"][..],
            CommandPath::WorkspaceCreate,
        ),
        (
            &["tab", "close", ID, "--confirm", "--help"][..],
            CommandPath::TabClose,
        ),
        (&["panel", "move", ID, "--help"][..], CommandPath::PanelMove),
    ] {
        assert_eq!(
            parse(arguments),
            Ok(Request::Help(HelpTopic::Command(topic))),
            "{arguments:?}"
        );
    }
}

#[test]
fn mutation_failures_follow_the_format_envelope() {
    // A valid json selection shapes mutation usage and identifier failures.
    assert_eq!(
        parse(&["tab", "close", ID, "--format", "json"]),
        Err(usage(FailureEnvelope::Json))
    );
    assert_eq!(
        parse(&["--format=json", "panel", "close", "nope", "--confirm"]),
        Err(identifier(Resource::Panel, "nope", FailureEnvelope::Json))
    );
    // Format anywhere legal around a complete mutation command selects json.
    assert_eq!(
        parse(&["--format=json", "tab", "close", ID, "--confirm",]),
        Ok(mutation(
            MutationCommand::TabClose {
                target: Target::Id(ID.into()),
                wait: CloseWait::Wait { timeout_ms: 30_000 },
            },
            OutputFormat::Json
        ))
    );
}

/// Every parse failure exit carries the concrete reason it reached; the table
/// pins one representative argv per classification so a regression in the
/// diagnostic wiring is caught before output.
#[test]
fn usage_failures_carry_their_concrete_reason() {
    let path = |spelling: &'static str| UsageContext::Path(spelling);
    let cases: &[(&[&str], UsageReason)] = &[
        // Argument-list and format-envelope failures (Silent envelope).
        (
            &["status", "--format"],
            UsageReason::MissingValue { option: "--format" },
        ),
        (
            &["status", "--format", "--"],
            UsageReason::MissingValue { option: "--format" },
        ),
        (
            &["status", "--format", "yaml"],
            UsageReason::InvalidValue {
                context: path("status"),
                option: "--format",
                value: "yaml".to_owned(),
                expected: ExpectedValue::Format,
            },
        ),
        (
            &["status", "--format="],
            UsageReason::InvalidValue {
                context: path("status"),
                option: "--format",
                value: String::new(),
                expected: ExpectedValue::Format,
            },
        ),
        (
            &["status", "--format", "json", "--format", "text"],
            UsageReason::DuplicateOption { option: "--format" },
        ),
        // Option-scan failures.
        (
            &["--bogus"],
            UsageReason::UnknownOption {
                context: UsageContext::Root,
                option: "--bogus".to_owned(),
            },
        ),
        (
            &["-x"],
            UsageReason::UnknownOption {
                context: UsageContext::Root,
                option: "-x".to_owned(),
            },
        ),
        (
            &["workspace", "list", "--project"],
            UsageReason::MissingValue {
                option: "--project",
            },
        ),
        (
            &["workspace", "list", "--project", "--"],
            UsageReason::MissingValue {
                option: "--project",
            },
        ),
        (
            &["workspace", "list", "--project", "a", "--project", "b"],
            UsageReason::DuplicateOption {
                option: "--project",
            },
        ),
        // Word-structure failures.
        (
            &["bogus"],
            UsageReason::UnknownCommand {
                context: UsageContext::Root,
                word: "bogus".to_owned(),
            },
        ),
        (
            &["--", "--help"],
            UsageReason::UnknownCommand {
                context: UsageContext::Root,
                word: "--help".to_owned(),
            },
        ),
        (
            &["terminal", "send-text"],
            UsageReason::UnknownCommand {
                context: path("terminal"),
                word: "send-text".to_owned(),
            },
        ),
        (
            &["project"],
            UsageReason::MissingCommand {
                context: path("project"),
            },
        ),
        (
            &["config"],
            UsageReason::MissingCommand {
                context: path("config"),
            },
        ),
        (
            &["--format=json"],
            UsageReason::MissingCommand {
                context: UsageContext::Root,
            },
        ),
        (
            &["status", "extra"],
            UsageReason::UnexpectedArgument {
                context: path("status"),
                value: "extra".to_owned(),
            },
        ),
        (
            &["status", "--", "--format", "json"],
            UsageReason::UnexpectedArgument {
                context: path("status"),
                value: "--format".to_owned(),
            },
        ),
        (
            &["project", "get"],
            UsageReason::MissingRequired {
                context: path("project get"),
                requirement: "<project>",
            },
        ),
        (
            &["terminal", "read"],
            UsageReason::MissingRequired {
                context: path("terminal read"),
                requirement: "<target> or --panel <panel>",
            },
        ),
        (
            &["completion"],
            UsageReason::MissingRequired {
                context: path("completion"),
                requirement: "<shell>",
            },
        ),
        (
            &["completion", "unknown-shell"],
            UsageReason::InvalidValue {
                context: path("completion"),
                option: "<shell>",
                value: "unknown-shell".to_owned(),
                expected: ExpectedValue::Shell,
            },
        ),
        (
            &["completion", "bash", "extra"],
            UsageReason::UnexpectedArgument {
                context: path("completion"),
                value: "extra".to_owned(),
            },
        ),
        // Help-path failures.
        (
            &["help", "bogus"],
            UsageReason::UnknownHelpTopic {
                path: "bogus".to_owned(),
            },
        ),
        (
            &["help", "status", "extra"],
            UsageReason::UnknownHelpTopic {
                path: "status extra".to_owned(),
            },
        ),
        (
            &["bogus", "--help"],
            UsageReason::UnknownHelpTopic {
                path: "bogus".to_owned(),
            },
        ),
        (
            &["help", "status", "--project", "x"],
            UsageReason::UnexpectedOption {
                context: path("status"),
                option: "--project",
            },
        ),
        (
            &["--version", "--help"],
            UsageReason::MisplacedFlag {
                context: UsageContext::Root,
                flag: "--version",
            },
        ),
        (
            &["--help", "--version"],
            UsageReason::MisplacedFlag {
                context: UsageContext::Root,
                flag: "--version",
            },
        ),
        (
            &["--help", "--help"],
            UsageReason::MisplacedFlag {
                context: UsageContext::Root,
                flag: "--help",
            },
        ),
        // Scope failures name the first out-of-scope flag in declaration order.
        (
            &["status", "--workspace", "x"],
            UsageReason::UnknownOption {
                context: path("status"),
                option: "--workspace".to_owned(),
            },
        ),
        (
            &["workspace", "list", "--workspace", "x"],
            UsageReason::UnknownOption {
                context: path("workspace list"),
                option: "--workspace".to_owned(),
            },
        ),
        (
            &["workspace", "list", "--tab", "x"],
            UsageReason::UnknownOption {
                context: path("workspace list"),
                option: "--tab".to_owned(),
            },
        ),
        (
            &["tab", "list", "--project", "x"],
            UsageReason::UnknownOption {
                context: path("tab list"),
                option: "--project".to_owned(),
            },
        ),
        (
            &["tui", "--format", "text"],
            UsageReason::UnexpectedOption {
                context: path("tui"),
                option: "--format",
            },
        ),
        (
            &["--version", "--format", "text"],
            UsageReason::UnexpectedOption {
                context: UsageContext::Literal("orchbee --version"),
                option: "--format",
            },
        ),
        // Required options, values, and geometry.
        (
            &["workspace", "rename", ID],
            UsageReason::MissingRequired {
                context: path("workspace rename"),
                requirement: "'--label'",
            },
        ),
        (
            &["workspace", "rename", ID, "--label", ""],
            UsageReason::InvalidValue {
                context: path("workspace rename"),
                option: "--label",
                value: String::new(),
                expected: ExpectedValue::Label,
            },
        ),
        (
            &["workspace", "accept-directory", ID],
            UsageReason::MissingRequired {
                context: path("workspace accept-directory"),
                requirement: "'--path'",
            },
        ),
        (
            &[
                "workspace",
                "accept-directory",
                ID,
                "--path",
                "relative/dir",
            ],
            UsageReason::InvalidValue {
                context: path("workspace accept-directory"),
                option: "--path",
                value: "relative/dir".to_owned(),
                expected: ExpectedValue::DirectoryPath,
            },
        ),
        (
            // A partial geometry set still refuses with the fixed
            // first-missing-flag diagnostic: `--cols` precedes `--rows`.
            &[
                "tab",
                "create",
                "--workspace",
                "main",
                "--label",
                "t",
                "--rows",
                "24",
            ],
            UsageReason::MissingRequired {
                context: path("tab create"),
                requirement: "'--cols'",
            },
        ),
        (
            &[
                "tab",
                "create",
                "--workspace",
                "main",
                "--label",
                "t",
                "--cols",
                "x",
            ],
            UsageReason::InvalidValue {
                context: path("tab create"),
                option: "--cols",
                value: "x".to_owned(),
                expected: ExpectedValue::Dimension,
            },
        ),
        (
            &[
                "tab",
                "create",
                "--workspace",
                "main",
                "--label",
                "t",
                "--cols",
                "80",
                "--rows",
                "24",
                "--cell-width",
                "8",
            ],
            UsageReason::MissingRequired {
                context: path("tab create"),
                requirement: "'--cell-height'",
            },
        ),
        (
            &[
                "tab",
                "create",
                "--workspace",
                "main",
                "--label",
                "t",
                "--cols",
                "80",
                "--rows",
                "24",
                "--cell-width",
                "0",
                "--cell-height",
                "16",
            ],
            UsageReason::InvalidGeometry {
                context: path("tab create"),
                cols: 80,
                rows: 24,
                cell_width: 0,
                cell_height: 16,
            },
        ),
        (
            &["tab", "close", ID],
            UsageReason::MissingRequired {
                context: path("tab close"),
                requirement: "'--confirm'",
            },
        ),
        (
            &["tab", "move", ID],
            UsageReason::MissingRequired {
                context: path("tab move"),
                requirement: "'--order' or '--insert-index'",
            },
        ),
        (
            &["tab", "move", ID, "--order", "left"],
            UsageReason::InvalidValue {
                context: path("tab move"),
                option: "--order",
                value: "left".to_owned(),
                expected: ExpectedValue::TabDirection,
            },
        ),
        (
            &[
                "panel",
                "move",
                ID,
                "--target-panel",
                ID,
                "--direction",
                "next",
                "--into-tab",
                ID2,
            ],
            UsageReason::InvalidValue {
                context: path("panel move"),
                option: "--direction",
                value: "next".to_owned(),
                expected: ExpectedValue::PanelDirection,
            },
        ),
        (
            &["panel", "move", ID, "--direction", "left"],
            UsageReason::MissingRequired {
                context: path("panel move"),
                requirement: "'--into-tab' or '--new-tab'",
            },
        ),
        (
            &["panel", "close", ID],
            UsageReason::MissingRequired {
                context: path("panel close"),
                requirement: "'--confirm'",
            },
        ),
    ];
    for (arguments, expected) in cases {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(failure.usage.as_ref(), Some(expected), "{arguments:?}");
    }
}

/// `--` ownership: a dash-leading token consumed as a value is never an
/// option, and a joined spelling whose name is not a declared value option
/// stays an unknown option.
#[test]
fn ownership_and_terminator_keep_their_reasons() {
    // A joined spelling on an undeclared name stays an unknown option —
    // `--bogus=x`, `--=x`, and a boolean `--standalone=x` all land here.
    for arguments in [
        &["workspace", "list", "--bogus=x"][..],
        &["workspace", "list", "--=x"][..],
        &["tab", "list", "--standalone=x"][..],
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::UnknownOption {
                context: UsageContext::Path(if arguments[0] == "tab" {
                    "tab list"
                } else {
                    "workspace list"
                }),
                option: arguments.last().expect("listed").to_string(),
            }),
            "{arguments:?}"
        );
    }
    // A consumed dash-leading value is never reclassified as an option.
    let Err(failure) = parse(&["status", "--bogus", "--project", "x"]) else {
        panic!("unknown option must fail");
    };
    assert_eq!(
        failure.usage.as_ref(),
        Some(&UsageReason::UnknownOption {
            context: UsageContext::Path("status"),
            option: "--bogus".to_owned(),
        })
    );
}

/// Current output sources share line bounds; geometry keeps its independent row dimension.
#[test]
fn terminal_read_source_limits() {
    // Geometry retains its independent row dimension.
    assert!(tab_create_with(&["--exec", "/bin/sh"]).is_ok());
    for (source, expected) in [
        ("visible", super::OutputSource::Visible),
        ("recent", super::OutputSource::Recent),
        ("recent-unwrapped", super::OutputSource::RecentUnwrapped),
        ("detection", super::OutputSource::Detection),
    ] {
        for (value, limit) in [
            ("0", 0),
            ("1", 1),
            ("1000", 1000),
            ("1001", 1000),
            ("4294967295", 1000),
        ] {
            assert!(
                matches!(parse(&["terminal", "read", ID, "--source", source, "--lines", value, "--format", "json"]),
                Ok(Request::Query { command: QueryCommand::TerminalOutput { selection, .. }, format: OutputFormat::Json })
                if selection == super::OutputSelection { source: expected, lines: Some(limit) })
            );
        }
    }
}

/// Output sources and line limits share one grammar.
#[test]
fn terminal_read_presentation_grammar() {
    for (args, unwrap, lines) in [
        (vec!["terminal", "read", ID], false, None),
        (
            vec!["terminal", "read", ID, "--source", "recent-unwrapped"],
            true,
            None,
        ),
        (vec!["terminal", "read", ID, "--lines", "5"], false, Some(5)),
        (vec!["terminal", "read", ID, "--lines", "0"], false, Some(0)),
        (
            vec!["terminal", "read", ID, "--lines", "65535"],
            false,
            Some(1000),
        ),
    ] {
        assert_eq!(
            parse(&args),
            Ok(query(
                expected_terminal_read(Target::Id(ID.into()), unwrap, lines),
                OutputFormat::Text
            )),
            "{args:?}"
        );
    }
    for value in ["4294967296", "abc", "-1", "+1", "5.0", "", " 5", "5 "] {
        let failure = parse(&["terminal", "read", ID, "--lines", value]).unwrap_err();
        assert_eq!(
            failure.usage,
            Some(UsageReason::InvalidValue {
                context: UsageContext::Path("terminal read"),
                option: "--lines",
                value: value.into(),
                expected: ExpectedValue::OutputLines,
            })
        );
    }
    for args in [
        vec!["terminal", "read", ID, "--source", "history"],
        vec![
            "terminal", "read", ID, "--source", "visible", "--source", "recent",
        ],
        vec!["terminal", "read", ID, "--lines", "1", "--lines", "2"],
        vec!["terminal", "list", "--lines", "3"],
        vec!["status", "--source", "recent"],
        vec!["terminal", "read", ID, "--plain", "--ansi"],
        vec!["terminal", "read", ID, "--plain", "--format", "json"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn terminal_wait_output_grammar() {
    let wait = |text: &str, rows: Option<u16>, timeout: Option<u32>| {
        query(
            expected_output_wait(text, rows, timeout),
            OutputFormat::Text,
        )
    }; // Required flag plus the two defaults; option order is free.
    for (arguments, expected) in [
        (
            &["terminal", "wait-output", ID, "--match", "ready"][..],
            wait("ready", None, None),
        ),
        (
            &[
                "terminal",
                "wait-output",
                ID,
                "--match",
                "ready",
                "--lines",
                "1",
            ][..],
            wait("ready", Some(1), None),
        ),
        (
            &[
                "terminal",
                "wait-output",
                ID,
                "--timeout",
                "3600000",
                "--lines",
                "1000",
                "--match",
                "ready",
            ][..],
            wait("ready", Some(1000), Some(3_600_000)),
        ),
        // `--match` owns its next word whole — flag-looking text is a value.
        (
            &["terminal", "wait-output", ID, "--match", "--lines"][..],
            wait("--lines", None, None),
        ),
    ] {
        assert_eq!(parse(arguments), Ok(expected), "{arguments:?}");
    }
    // Format selection composes.
    assert_eq!(
        parse(&[
            "terminal",
            "wait-output",
            ID,
            "--match",
            "ready",
            "--format",
            "json"
        ]),
        Ok(Request::Query {
            command: expected_output_wait("ready", None, None),
            format: OutputFormat::Json,
        })
    );
    // Exactly one search input is required — never a default, never both.
    for arguments in [
        &["terminal", "wait-output", ID][..],
        &["terminal", "wait-output", ID, "--lines", "5"][..],
        &["terminal", "wait-output", ID, "--format", "json"][..],
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::MissingRequired {
                context: UsageContext::Path("terminal wait-output"),
                requirement: "'--match' or '--regex'",
            }),
            "{arguments:?}"
        );
    }
    assert_eq!(
        parse(&[
            "terminal",
            "wait-output",
            ID,
            "--match",
            "a",
            "--regex",
            "b"
        ])
        .expect_err("both search inputs must conflict")
        .usage
        .as_ref(),
        Some(&UsageReason::ConflictingOptions {
            context: UsageContext::Path("terminal wait-output"),
            first: "--match",
            second: "--regex",
        })
    );
    // The regex arm parses and compiles to the compiled pattern.
    let compiled = regex::Regex::new("re[a-z]+dy").expect("regex");
    assert_eq!(
        parse(&[
            "terminal",
            "wait-output",
            ID,
            "--regex",
            "re[a-z]+dy",
            "--lines",
            "3"
        ]),
        Ok(query(
            QueryCommand::TerminalSourceWait {
                after: None,
                target: Target::TerminalOrPanelId(ID.into()),
                matcher: WaitPattern::Regex(compiled),
                selection: super::OutputSelection {
                    source: super::OutputSource::Recent,
                    lines: Some(3)
                },
                timeout_ms: Some(30_000),
            },
            OutputFormat::Text,
        ))
    );
    // `--regex` value shape: nonempty, ≤1024 bytes, and a program that
    // compiles inside the declared budgets.
    let over = "x".repeat(1025);
    for value in ["", over.as_str(), "[", "(", "a{999999999999}"] {
        let arguments = ["terminal", "wait-output", ID, "--regex", value];
        let Err(failure) = parse(&arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::InvalidValue {
                context: UsageContext::Path("terminal wait-output"),
                option: "--regex",
                value: value.to_owned(),
                expected: ExpectedValue::RegexPattern,
            }),
            "{arguments:?}"
        );
    }
    // `--regex` is out of scope everywhere else.
    for (arguments, context) in [
        (
            &["terminal", "read", ID, "--regex", "a"][..],
            "terminal read",
        ),
        (&["terminal", "list", "--regex", "a"][..], "terminal list"),
        (&["status", "--regex", "a"][..], "status"),
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::UnknownOption {
                context: UsageContext::Path(context),
                option: "--regex".to_owned(),
            }),
            "{arguments:?}"
        );
    }
    assert_eq!(
        parse(&[
            "terminal",
            "wait-output",
            ID,
            "--regex",
            "a",
            "--regex",
            "b"
        ])
        .expect_err("repeat must fail")
        .usage
        .as_ref(),
        Some(&UsageReason::DuplicateOption { option: "--regex" })
    );
    // `--match` value shape: non-empty, ≤4096 bytes (a 4097-byte value fails).
    let over = "x".repeat(4097);
    for value in ["", over.as_str()] {
        let arguments = ["terminal", "wait-output", ID, "--match", value];
        let Err(failure) = parse(&arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::InvalidValue {
                context: UsageContext::Path("terminal wait-output"),
                option: "--match",
                value: value.to_owned(),
                expected: ExpectedValue::MatchText,
            }),
            "{arguments:?}"
        );
    }
    // Output limits share the unsigned, clamped line-count grammar.
    for value in ["4294967296", "+1", "abc", "-1", "5.0"] {
        let arguments = [
            "terminal",
            "wait-output",
            ID,
            "--match",
            "m",
            "--lines",
            value,
        ];
        let Err(failure) = parse(&arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::InvalidValue {
                context: UsageContext::Path("terminal wait-output"),
                option: "--lines",
                value: value.to_owned(),
                expected: ExpectedValue::OutputLines,
            }),
            "{arguments:?}"
        );
    }
    // `--timeout` value shape: 1..=3600000 milliseconds.
    for value in ["0", "3600001", "abc", "-1", "1.5", ""] {
        let arguments = [
            "terminal",
            "wait-output",
            ID,
            "--match",
            "m",
            "--timeout",
            value,
        ];
        let Err(failure) = parse(&arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::InvalidValue {
                context: UsageContext::Path("terminal wait-output"),
                option: "--timeout",
                value: value.to_owned(),
                expected: ExpectedValue::WaitTimeout,
            }),
            "{arguments:?}"
        );
    }
    // All three flags are out of scope elsewhere, named in declaration order.
    for (arguments, context) in [
        (
            &["terminal", "read", ID, "--match", "m"][..],
            "terminal read",
        ),
        (
            &["terminal", "get", ID, "--timeout", "5"][..],
            "terminal get",
        ),
        (&["status", "--match", "m"][..], "status"),
        (&["panel", "list", "--timeout", "5"][..], "panel list"),
    ] {
        let option = if arguments.contains(&"--match") {
            "--match"
        } else {
            "--timeout"
        };
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::UnknownOption {
                context: UsageContext::Path(context),
                option: option.to_owned(),
            }),
            "{arguments:?}"
        );
    }
    // Repeated flags stay duplicates before resolution.
    for (arguments, expected) in [
        (
            &[
                "terminal",
                "wait-output",
                ID,
                "--match",
                "a",
                "--match",
                "b",
            ][..],
            UsageReason::DuplicateOption { option: "--match" },
        ),
        (
            &[
                "terminal",
                "wait-output",
                ID,
                "--match",
                "a",
                "--timeout",
                "5",
                "--timeout",
                "6",
            ][..],
            UsageReason::DuplicateOption {
                option: "--timeout",
            },
        ),
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(failure.usage.as_ref(), Some(&expected), "{arguments:?}");
    }
}

#[test]
fn terminal_send_grammar() {
    let send = |input| {
        mutation(
            MutationCommand::TerminalSend {
                target: Target::TerminalOrPanelId(ID.into()),
                input,
            },
            OutputFormat::Text,
        )
    };
    assert_eq!(
        parse(&["terminal", "send", ID, "--text", "ls -la"]),
        Ok(send(TerminalSendInput::Text {
            text: "ls -la".to_owned(),
            enter: false,
        }))
    );
    assert_eq!(
        parse(&["terminal", "send", ID, "--text", "make", "--enter"]),
        Ok(send(TerminalSendInput::Text {
            text: "make".to_owned(),
            enter: true,
        }))
    );
    assert_eq!(
        parse(&[
            "terminal", "send", ID, "--enter", "--text", "make", "--format", "json"
        ]),
        Ok(mutation(
            MutationCommand::TerminalSend {
                target: Target::TerminalOrPanelId(ID.into()),
                input: TerminalSendInput::Text {
                    text: "make".to_owned(),
                    enter: true,
                },
            },
            OutputFormat::Json,
        ))
    );
    // Every admitted key name resolves to its semantic key.
    for (name, key) in [
        ("enter", orchbee_client_server_protocol::SendKey::Enter),
        ("escape", orchbee_client_server_protocol::SendKey::Escape),
        ("tab", orchbee_client_server_protocol::SendKey::Tab),
        (
            "backspace",
            orchbee_client_server_protocol::SendKey::Backspace,
        ),
        ("delete", orchbee_client_server_protocol::SendKey::Delete),
        ("up", orchbee_client_server_protocol::SendKey::Up),
        ("down", orchbee_client_server_protocol::SendKey::Down),
        ("left", orchbee_client_server_protocol::SendKey::Left),
        ("right", orchbee_client_server_protocol::SendKey::Right),
        ("home", orchbee_client_server_protocol::SendKey::Home),
        ("end", orchbee_client_server_protocol::SendKey::End),
        ("page-up", orchbee_client_server_protocol::SendKey::PageUp),
        (
            "page-down",
            orchbee_client_server_protocol::SendKey::PageDown,
        ),
    ] {
        assert_eq!(
            parse(&["terminal", "send", ID, "--key", name]),
            Ok(send(TerminalSendInput::Keys(vec![
                orchbee_client_server_protocol::SendKeyStroke::new(
                    orchbee_client_server_protocol::SendStrokeKey::Named(key),
                    0
                )
                .unwrap()
            ]))),
            "{name}"
        );
    }
    // Exactly one source is required.
    let Err(failure) = parse(&["terminal", "send", ID]) else {
        panic!("missing payload must fail");
    };
    assert_eq!(
        failure.usage.as_ref(),
        Some(&UsageReason::MissingRequired {
            context: UsageContext::Path("terminal send"),
            requirement: "'--text', '--key', '--file', or '--stdin'",
        })
    );
    // The declared sources parse untouched into their own variants.
    assert_eq!(
        parse(&["terminal", "send", ID, "--file", "/tmp/in.txt"]),
        Ok(send(TerminalSendInput::File {
            path: "/tmp/in.txt".to_owned(),
            enter: false,
            paste: false,
        }))
    );
    assert_eq!(
        parse(&["terminal", "send", ID, "--file", "/tmp/in.txt", "--enter"]),
        Ok(send(TerminalSendInput::File {
            path: "/tmp/in.txt".to_owned(),
            enter: true,
            paste: false,
        }))
    );
    assert_eq!(
        parse(&["terminal", "send", ID, "--stdin"]),
        Ok(send(TerminalSendInput::Stdin {
            enter: false,
            paste: false
        }))
    );
    assert_eq!(
        parse(&["terminal", "send", ID, "--stdin", "--enter"]),
        Ok(send(TerminalSendInput::Stdin {
            enter: true,
            paste: false
        }))
    );
    // The forms exclude each other, named in the fixed order
    // --text, --key, --file, --stdin regardless of argv order.
    for (arguments, first, second) in [
        (
            &["terminal", "send", ID, "--text", "x", "--key", "enter"][..],
            "--text",
            "--key",
        ),
        (
            &["terminal", "send", ID, "--key", "enter", "--text", "x"][..],
            "--text",
            "--key",
        ),
        (
            &["terminal", "send", ID, "--stdin", "--text", "x"][..],
            "--text",
            "--stdin",
        ),
        (
            &["terminal", "send", ID, "--file", "/p", "--stdin"][..],
            "--file",
            "--stdin",
        ),
        (
            &["terminal", "send", ID, "--stdin", "--file", "/p"][..],
            "--file",
            "--stdin",
        ),
        (
            &["terminal", "send", ID, "--stdin", "--key", "enter"][..],
            "--key",
            "--stdin",
        ),
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("conflicting payload forms must fail: {arguments:?}");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::ConflictingOptions {
                context: UsageContext::Path("terminal send"),
                first,
                second,
            }),
            "{arguments:?}"
        );
    }
    // `--enter` is the text form's modifier only.
    for arguments in [
        &["terminal", "send", ID, "--enter"][..],
        &["terminal", "send", ID, "--key", "enter", "--enter"][..],
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::UnexpectedOption {
                context: UsageContext::Path("terminal send"),
                option: "--enter",
            }),
            "{arguments:?}"
        );
    }
    // Empty text and over-budget records are refused at the boundary; the
    // bound counts the folded CR, so 4096 literal bytes fit, 4095+CR is the
    // last legal record, and 4096+CR or 4097 literal are refused.
    let maximum = "x".repeat(4096);
    assert_eq!(
        parse(&["terminal", "send", ID, "--text", maximum.as_str()]),
        Ok(send(TerminalSendInput::Text {
            text: maximum.clone(),
            enter: false,
        }))
    );
    let edge = "x".repeat(4095);
    assert_eq!(
        parse(&["terminal", "send", ID, "--text", edge.as_str(), "--enter"]),
        Ok(send(TerminalSendInput::Text {
            text: edge,
            enter: true,
        }))
    );
    for (text, enter) in [
        (String::new(), false),
        ("x".repeat(4097), false),
        ("x".repeat(4096), true),
    ] {
        let mut arguments = vec!["terminal", "send", ID, "--text", text.as_str()];
        if enter {
            arguments.push("--enter");
        }
        let Err(failure) = parse(&arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::InvalidValue {
                context: UsageContext::Path("terminal send"),
                option: "--text",
                value: text.clone(),
                expected: ExpectedValue::SendText,
            }),
            "{arguments:?}"
        );
    }
    let Err(failure) = parse(&["terminal", "send", ID, "--key", "ctrl-x"]) else {
        panic!("unknown key must fail");
    };
    assert_eq!(
        failure.usage.as_ref(),
        Some(&UsageReason::InvalidValue {
            context: UsageContext::Path("terminal send"),
            option: "--key",
            value: "ctrl-x".to_owned(),
            expected: ExpectedValue::SendKey,
        })
    );
    // The target stays a typed terminal id — labels are not admitted.
    for (value, expect_identifier) in [("web", true), (ID, false), (ZERO, true)] {
        let outcome = parse(&["terminal", "send", value, "--key", "enter"]);
        if expect_identifier {
            assert_eq!(
                outcome.map_err(|failure| failure.kind),
                Err(ParseFailureKind::InvalidIdentifier {
                    resource: Resource::Terminal,
                    target: value.to_owned(),
                }),
                "{value}"
            );
        } else {
            assert!(outcome.is_ok(), "{value}");
        }
    }
    // Send flags stay inside their own scope.
    for (arguments, option) in [
        (&["terminal", "get", ID, "--text", "x"][..], "--text"),
        (&["terminal", "read", ID, "--key", "enter"][..], "--key"),
        (&["terminal", "list", "--enter"][..], "--enter"),
    ] {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.usage.as_ref(),
            Some(&UsageReason::UnknownOption {
                context: match arguments[1] {
                    "get" => UsageContext::Path("terminal get"),
                    "read" => UsageContext::Path("terminal read"),
                    _ => UsageContext::Path("terminal list"),
                },
                option: option.to_owned(),
            }),
            "{arguments:?}"
        );
    }
}

#[test]
fn worktree_list_grammar() {
    for (words, format) in [
        (
            vec!["worktree", "list", "--directory", ID2],
            OutputFormat::Text,
        ),
        (
            vec!["--format=json", "worktree", "list", "--directory", ID2],
            OutputFormat::Json,
        ),
    ] {
        assert_eq!(
            parse(&words),
            Ok(query(
                QueryCommand::WorktreeList {
                    directory: WorkbenchDirectoryId::parse(ID2).unwrap(),
                },
                format
            ))
        );
    }
    for value in [
        ZERO,
        "0123456789ABCDEF0123456789ABCDEF",
        "short",
        "/repo/main",
        "label",
        "",
    ] {
        let failure = parse(&["worktree", "list", "--directory", value]).unwrap_err();
        assert_eq!(
            failure.usage,
            Some(UsageReason::InvalidValue {
                context: UsageContext::Path("worktree list"),
                option: "--directory",
                value: value.into(),
                expected: ExpectedValue::DirectoryId,
            })
        );
    }
    for words in [
        vec!["worktree", "list", "--project", ID, "--directory", ID2],
        vec!["worktree", "list", "extra", "--directory", ID2],
        vec!["worktree", "list", "--workspace", ID, "--directory", ID2],
        vec!["worktree", "list", "--directory", ID2, "--directory", ID2],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn worktree_help_paths_resolve() {
    for (arguments, topic) in [
        (
            &["help", "worktree"][..],
            HelpTopic::Family(Family::Worktree),
        ),
        (
            &["help", "worktree", "list"][..],
            HelpTopic::Command(CommandPath::WorktreeList),
        ),
        (
            &["worktree", "--help"][..],
            HelpTopic::Family(Family::Worktree),
        ),
        (
            &["worktree", "list", "--help"][..],
            HelpTopic::Command(CommandPath::WorktreeList),
        ),
    ] {
        assert_eq!(parse(arguments), Ok(Request::Help(topic)), "{arguments:?}");
    }
}

const AGENT_PROTOCOL_CURSOR: &str = "v1.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456.abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";
#[cfg(not(windows))]
const AGENT_CURSOR: &str = AGENT_PROTOCOL_CURSOR;
#[cfg(windows)]
const AGENT_CURSOR: &str = "u1.eyJzaXRlIjoiMTExMTExMTExMTExMTExMTExMTExMTExMTExMTExMTEiLCJ3b3JrYmVuY2giOiIxIiwiY2Fub25pY2FsIjoxLCJyZXBvcnRzIjoiMSIsImluY2x1ZGVFbmRlZCI6ZmFsc2UsImNsaWVudCI6bnVsbCwib2Zmc2V0IjoxLCJmaW5nZXJwcmludCI6IjAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAifQ";

/// `agent list|get|explain`: the flat read grammar — defaults, option values,
/// and the opaque session identity operand.
#[test]
fn agent_read_grammar() {
    use orchbee_agent_observation_protocol::AgentSessionId;
    let session = || AgentSessionId::parse("sess-alpha").expect("valid session id");
    let cursor = || AGENT_CURSOR.to_owned();

    // Defaults: first page, live sessions only, page size 50.
    assert_eq!(
        parse(&["agent", "list"]),
        Ok(query(
            QueryCommand::AgentList {
                client: None,
                include_ended: false,
                limit: 50,
                cursor: None,
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&[
            "agent",
            "list",
            "--client",
            "00000000000000000000000000000007"
        ]),
        Ok(query(
            QueryCommand::AgentList {
                client: Some(
                    orchbee_workbench_protocol::WorkbenchClientId::parse(
                        "00000000000000000000000000000007"
                    )
                    .unwrap()
                ),
                include_ended: false,
                limit: 50,
                cursor: None,
            },
            OutputFormat::Text
        ))
    );
    // Every declared option in any order.
    assert_eq!(
        parse(&[
            "agent",
            "list",
            "--include-ended",
            "--limit",
            "25",
            "--cursor",
            AGENT_CURSOR,
        ]),
        Ok(query(
            QueryCommand::AgentList {
                client: None,
                include_ended: true,
                limit: 25,
                cursor: Some(cursor()),
            },
            OutputFormat::Text
        ))
    );
    // Page-size bounds are inclusive on both ends.
    for value in ["1", "100"] {
        assert_eq!(
            parse(&["agent", "list", "--limit", value]),
            Ok(query(
                QueryCommand::AgentList {
                    client: None,
                    include_ended: false,
                    limit: value.parse().expect("bound literal"),
                    cursor: None,
                },
                OutputFormat::Text
            )),
            "--limit {value}"
        );
    }
    // Session identity operands: opaque strings, never labels or typed ids.
    assert_eq!(
        parse(&["agent", "get", "sess-alpha"]),
        Ok(query(
            QueryCommand::AgentGet(AgentTarget::Auto { target: session() }),
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["agent", "explain", "sess-alpha"]),
        Ok(query(
            QueryCommand::AgentExplain(AgentTarget::Auto { target: session() }),
            OutputFormat::Text
        ))
    );
    // A 32-hex operand is still just a session identity — no typed-id split.
    assert_eq!(
        parse(&["agent", "get", ID]),
        Ok(query(
            QueryCommand::AgentGet(AgentTarget::Auto {
                target: AgentSessionId::parse(ID).expect("hex session id")
            }),
            OutputFormat::Text
        ))
    );
    // Format selection applies like every other query.
    assert_eq!(
        parse(&["agent", "list", "--format", "json"]),
        Ok(query(
            QueryCommand::AgentList {
                client: None,
                include_ended: false,
                limit: 50,
                cursor: None,
            },
            OutputFormat::Json
        ))
    );
}

/// `agent` grammar failures: wrong arity, out-of-scope options, and invalid
/// option values all stay usage failures — the wire never sees them.
#[test]
fn agent_read_usage_failures() {
    let usage_reason = |arguments: &[&str]| {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.kind,
            ParseFailureKind::InvalidUsage,
            "{arguments:?}"
        );
        failure.usage.expect("usage reason recorded")
    };
    let path = |spelling: &'static str| UsageContext::Path(spelling);

    // Missing command / missing operand / extra operands / unknown verb.
    assert_eq!(
        usage_reason(&["agent"]),
        UsageReason::MissingCommand {
            context: path("agent"),
        }
    );
    assert_eq!(
        usage_reason(&["agent", "list", "extra"]),
        UsageReason::UnexpectedArgument {
            context: path("agent list"),
            value: "extra".to_owned(),
        }
    );
    assert_eq!(
        usage_reason(&["agent", "get"]),
        UsageReason::MissingRequired {
            context: path("agent get"),
            requirement: "<target> or --agent-session <id> or --name <alias> or --panel <id|selector> or --terminal <id|selector>",
        }
    );
    assert_eq!(
        usage_reason(&["agent", "explain", "a", "b"]),
        UsageReason::UnexpectedArgument {
            context: path("agent explain"),
            value: "b".to_owned(),
        }
    );
    assert_eq!(
        usage_reason(&["agent", "frobnicate"]),
        UsageReason::UnknownCommand {
            context: path("agent"),
            word: "frobnicate".to_owned(),
        }
    );

    // Out-of-scope options on each path.
    for (arguments, context, option) in [
        (
            &["agent", "get", "sess", "--include-ended"][..],
            "agent get",
            "--include-ended",
        ),
        (
            &["agent", "explain", "sess", "--cursor", AGENT_CURSOR][..],
            "agent explain",
            "--cursor",
        ),
        (
            &["agent", "list", "--label", "x"][..],
            "agent list",
            "--label",
        ),
    ] {
        assert_eq!(
            usage_reason(arguments),
            UsageReason::UnknownOption {
                context: path(context),
                option: option.to_owned(),
            },
            "{arguments:?}"
        );
    }

    // Invalid option values and identities.
    for value in ["0", "101", "1.5", "abc", "-1"] {
        assert_eq!(
            usage_reason(&["agent", "list", "--limit", value]),
            UsageReason::InvalidValue {
                context: path("agent list"),
                option: "--limit",
                value: value.to_owned(),
                expected: ExpectedValue::AgentListLimit,
            },
            "{value}"
        );
    }
    // Cursor encoding belongs to the emitting backend; grammar preserves the opaque input.
    for value in ["nope", "v1.short.tag"] {
        assert!(
            matches!(parse(&["agent", "list", "--cursor", value]), Ok(Request::Query {
            command: QueryCommand::AgentList { cursor: Some(cursor), .. }, ..
        }) if cursor == value)
        );
    }
    for (verb, value) in [("get", "bad id!"), ("explain", ""), ("get", "non-ascii-会")] {
        assert_eq!(
            usage_reason(&["agent", verb, value]),
            UsageReason::InvalidValue {
                context: path(if verb == "get" {
                    "agent get"
                } else {
                    "agent explain"
                }),
                option: "<target>",
                value: value.to_owned(),
                expected: ExpectedValue::AgentTarget,
            },
            "{verb} {value}"
        );
    }
    assert_eq!(
        usage_reason(&["agent", "list", "--limit"]),
        UsageReason::MissingValue { option: "--limit" }
    );
    assert_eq!(
        usage_reason(&["agent", "list", "--include-ended", "--include-ended"]),
        UsageReason::DuplicateOption {
            option: "--include-ended",
        }
    );
}

/// `agent wait`: the held-session wait grammar — the canonical default
/// seven when `--until` is absent, the twelve-name vocabulary in requested
/// order, the safe-integer revision fence, and the 600000 ms default.
#[test]
fn agent_wait_grammar() {
    let explicit_client = "11111111111111111111111111111111";
    assert!(
        matches!(parse(&["agent", "wait", "sess-alpha", "--client", explicit_client]),
        Ok(Request::Query { command: QueryCommand::AgentWait { client: Some(client), .. }, .. })
        if client.as_str() == explicit_client)
    );

    use orchbee_agent_observation_protocol::AgentSessionId;
    let session = || AgentSessionId::parse("sess-alpha").expect("valid session id");
    let defaults: Option<Vec<crate::invocation::AgentWaitCondition>> = None;

    // Absence remains explicit until the selected backend applies its documented defaults.
    assert_eq!(
        parse(&["agent", "wait", "sess-alpha"]),
        Ok(query(
            QueryCommand::AgentWait {
                client: None,
                session: AgentTarget::Auto { target: session() },
                until: defaults.clone(),
                after_revision: None,
                timeout_ms: None,
            },
            OutputFormat::Text
        ))
    );
    // Every declared option in any order; `--until` repeats in requested
    // order verbatim.
    assert_eq!(
        parse(&[
            "agent",
            "wait",
            "sess-alpha",
            "--until",
            "stale",
            "--after-revision",
            "42",
            "--timeout",
            "1200",
            "--until",
            "failed",
            "--format",
            "json",
        ]),
        Ok(query(
            QueryCommand::AgentWait {
                client: None,
                session: AgentTarget::Auto { target: session() },
                until: Some(vec![
                    crate::invocation::AgentWaitCondition::Stale,
                    crate::invocation::AgentWaitCondition::Failed,
                ]),
                after_revision: Some(orchbee_agent_observation_protocol::AgentSafeInteger::new(
                    42
                )),
                timeout_ms: Some(crate::invocation::AgentWaitTimeout::Milliseconds(1_200)),
            },
            OutputFormat::Json
        ))
    );
    // The whole twelve-name condition vocabulary parses, one per flag.
    for (name, condition) in [
        (
            "needs-attention",
            crate::invocation::AgentWaitCondition::NeedsAttention,
        ),
        ("failed", crate::invocation::AgentWaitCondition::Failed),
        (
            "succeeded",
            crate::invocation::AgentWaitCondition::Succeeded,
        ),
        ("canceled", crate::invocation::AgentWaitCondition::Canceled),
        (
            "interrupted",
            crate::invocation::AgentWaitCondition::Interrupted,
        ),
        ("working", crate::invocation::AgentWaitCondition::Working),
        ("idle", crate::invocation::AgentWaitCondition::Idle),
        ("unknown", crate::invocation::AgentWaitCondition::Unknown),
        ("ended", crate::invocation::AgentWaitCondition::Ended),
        ("fresh", crate::invocation::AgentWaitCondition::Fresh),
        ("stale", crate::invocation::AgentWaitCondition::Stale),
        (
            "disconnected",
            crate::invocation::AgentWaitCondition::Disconnected,
        ),
    ] {
        let Ok(request) = parse(&["agent", "wait", "sess-alpha", "--until", name]) else {
            panic!("--until {name} must parse");
        };
        let Request::Query { command, .. } = request else {
            panic!("--until {name} resolves to a query");
        };
        assert_eq!(
            command,
            QueryCommand::AgentWait {
                client: None,
                session: AgentTarget::Auto { target: session() },
                until: Some(vec![condition]),
                after_revision: None,
                timeout_ms: None,
            },
            "--until {name}"
        );
    }
    // Fence and timeout bounds are inclusive on both ends.
    for value in ["0", "9007199254740991"] {
        assert_eq!(
            parse(&["agent", "wait", "sess-alpha", "--after-revision", value]),
            Ok(query(
                QueryCommand::AgentWait {
                    client: None,
                    session: AgentTarget::Auto { target: session() },
                    until: defaults.clone(),
                    after_revision: Some(
                        orchbee_agent_observation_protocol::AgentSafeInteger::new(
                            value.parse().expect("bound literal")
                        )
                    ),
                    timeout_ms: None,
                },
                OutputFormat::Text
            )),
            "--after-revision {value}"
        );
    }
    for value in ["1", "3600000"] {
        assert_eq!(
            parse(&["agent", "wait", "sess-alpha", "--timeout", value]),
            Ok(query(
                QueryCommand::AgentWait {
                    client: None,
                    session: AgentTarget::Auto { target: session() },
                    until: defaults.clone(),
                    after_revision: None,
                    timeout_ms: Some(crate::invocation::AgentWaitTimeout::Milliseconds(
                        value.parse().expect("bound literal")
                    )),
                },
                OutputFormat::Text
            )),
            "--timeout {value}"
        );
    }
}

/// `agent wait` usage failures: wrong arity, out-of-scope options, unknown
/// or duplicated conditions, and out-of-range values all stay usage
/// failures — the wire never sees them.
#[test]
fn agent_wait_usage_failures() {
    let usage_reason = |arguments: &[&str]| {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.kind,
            ParseFailureKind::InvalidUsage,
            "{arguments:?}"
        );
        failure.usage.expect("usage reason recorded")
    };
    let path = |spelling: &'static str| UsageContext::Path(spelling);

    assert_eq!(
        usage_reason(&["agent", "wait"]),
        UsageReason::MissingRequired {
            context: path("agent wait"),
            requirement: "<target> or --agent-session <id> or --name <alias> or --panel <id|selector> or --terminal <id|selector>",
        }
    );
    assert_eq!(
        usage_reason(&["agent", "wait", "a", "b"]),
        UsageReason::UnexpectedArgument {
            context: path("agent wait"),
            value: "b".to_owned(),
        }
    );
    // Out-of-scope options, including the wait-output peers `--timeout`
    // does not bring along.
    for (arguments, option) in [
        (&["agent", "wait", "sess", "--match", "x"][..], "--match"),
        (&["agent", "wait", "sess", "--rows", "5"][..], "--rows"),
        (
            &["agent", "wait", "sess", "--include-ended"][..],
            "--include-ended",
        ),
        (&["agent", "wait", "sess", "--limit", "5"][..], "--limit"),
        (
            &["agent", "wait", "sess", "--cursor", AGENT_CURSOR][..],
            "--cursor",
        ),
        (&["agent", "wait", "sess", "--label", "x"][..], "--label"),
    ] {
        assert_eq!(
            usage_reason(arguments),
            UsageReason::UnknownOption {
                context: path("agent wait"),
                option: option.to_owned(),
            },
            "{arguments:?}"
        );
    }
    // Unknown condition names name the closed vocabulary.
    for value in ["running", "finished", "Idle", ""] {
        assert_eq!(
            usage_reason(&["agent", "wait", "sess", "--until", value]),
            UsageReason::InvalidValue {
                context: path("agent wait"),
                option: "--until",
                value: value.to_owned(),
                expected: ExpectedValue::AgentWaitUntil,
            },
            "--until {value}"
        );
    }
    // A repeated condition is a duplicate option, distinct from a repeat
    // of the flag itself carrying a different value.
    assert_eq!(
        usage_reason(&[
            "agent", "wait", "sess", "--until", "idle", "--until", "idle"
        ]),
        UsageReason::DuplicateOption { option: "--until" }
    );
    // Fence values outside the safe-integer range fail; the flag itself is
    // single-use.
    for value in ["-1", "9007199254740992", "1.5", "abc"] {
        assert_eq!(
            usage_reason(&["agent", "wait", "sess", "--after-revision", value]),
            UsageReason::InvalidValue {
                context: path("agent wait"),
                option: "--after-revision",
                value: value.to_owned(),
                expected: ExpectedValue::AgentAfterRevision,
            },
            "--after-revision {value}"
        );
    }
    assert_eq!(
        usage_reason(&[
            "agent",
            "wait",
            "sess",
            "--after-revision",
            "1",
            "--after-revision",
            "2"
        ]),
        UsageReason::DuplicateOption {
            option: "--after-revision",
        }
    );
    // The shared `--timeout` flag under the wait meaning: the same
    // 1..=3600000 vocabulary `wait-output` uses.
    for value in ["0", "3600001", "1.5", "abc"] {
        assert_eq!(
            usage_reason(&["agent", "wait", "sess", "--timeout", value]),
            UsageReason::InvalidValue {
                context: path("agent wait"),
                option: "--timeout",
                value: value.to_owned(),
                expected: ExpectedValue::WaitTimeout,
            },
            "--timeout {value}"
        );
    }
    assert_eq!(
        usage_reason(&["agent", "wait", "sess", "--timeout", "5", "--timeout", "6"]),
        UsageReason::DuplicateOption {
            option: "--timeout"
        }
    );
    assert_eq!(
        usage_reason(&["agent", "wait", "sess", "--until"]),
        UsageReason::MissingValue { option: "--until" }
    );
    assert_eq!(
        usage_reason(&["agent", "wait", "bad id!"]),
        UsageReason::InvalidValue {
            context: path("agent wait"),
            option: "<target>",
            value: "bad id!".to_owned(),
            expected: ExpectedValue::AgentTarget,
        }
    );
}

/// Agent help resolves on both surfaces and lands on the pinned topics.
#[test]
fn agent_help_paths_resolve() {
    assert_eq!(
        parse(&["agent", "--help"]),
        Ok(Request::Help(HelpTopic::Family(Family::Agent)))
    );
    assert_eq!(
        parse(&["help", "agent"]),
        Ok(Request::Help(HelpTopic::Family(Family::Agent)))
    );
    for (verb, command) in [
        ("list", CommandPath::AgentList),
        ("get", CommandPath::AgentGet),
        ("explain", CommandPath::AgentExplain),
        ("wait", CommandPath::AgentWait),
    ] {
        assert_eq!(
            parse(&["agent", verb, "--help"]),
            Ok(Request::Help(HelpTopic::Command(command))),
            "agent {verb} --help"
        );
        assert_eq!(
            parse(&["help", "agent", verb]),
            Ok(Request::Help(HelpTopic::Command(command))),
            "help agent {verb}"
        );
    }
}

/// `agent doctor` grammar: provider and asset-root selectors are repeatable
/// and order-preserving, `--format` stays orthogonal, and no flag selects a
/// network path — the query variant is the whole result.
#[test]
fn agent_doctor_grammar() {
    // Defaults: all providers, no supplied roots, text output.
    assert_eq!(
        parse(&["agent", "doctor"]),
        Ok(query(
            QueryCommand::AgentDoctor {
                providers: Vec::new(),
                asset_roots: Vec::new(),
            },
            OutputFormat::Text
        ))
    );
    // Both selectors repeat and keep flag order; duplicates pass through the
    // grammar layer untouched (canonical-order dedup lives in the probe).
    assert_eq!(
        parse(&[
            "agent",
            "doctor",
            "--provider",
            "qwen-code",
            "--asset-root",
            &doctor_test_root("/opt/capsule-a"),
            "--provider",
            "codex",
            "--provider",
            "codex",
            "--asset-root",
            &doctor_test_root("/opt/capsule-b"),
        ]),
        Ok(query(
            QueryCommand::AgentDoctor {
                providers: vec![
                    "qwen-code".to_owned(),
                    "codex".to_owned(),
                    "codex".to_owned(),
                ],
                asset_roots: vec![
                    doctor_test_root("/opt/capsule-a"),
                    doctor_test_root("/opt/capsule-b")
                ],
            },
            OutputFormat::Text
        ))
    );
    // Every canonical provider id admits.
    for id in [
        "claude-code",
        "codex",
        "gemini",
        "github-copilot",
        "grok",
        "cline",
        "opencode",
        "pi",
        "omp",
        "qwen-code",
    ] {
        assert_eq!(
            parse(&["agent", "doctor", "--provider", id]),
            Ok(query(
                QueryCommand::AgentDoctor {
                    providers: vec![id.to_owned()],
                    asset_roots: Vec::new(),
                },
                OutputFormat::Text
            )),
            "--provider {id}"
        );
    }
    // Exactly four supplied roots is the admitted ceiling.
    assert_eq!(
        parse(&[
            "agent",
            "doctor",
            "--asset-root",
            &doctor_test_root("/a"),
            "--asset-root",
            &doctor_test_root("/b"),
            "--asset-root",
            &doctor_test_root("/c"),
            "--asset-root",
            &doctor_test_root("/d"),
        ]),
        Ok(query(
            QueryCommand::AgentDoctor {
                providers: Vec::new(),
                asset_roots: vec![
                    doctor_test_root("/a"),
                    doctor_test_root("/b"),
                    doctor_test_root("/c"),
                    doctor_test_root("/d")
                ],
            },
            OutputFormat::Text
        ))
    );
    // Format selection applies like every other query.
    assert_eq!(
        parse(&["agent", "doctor", "--format", "json"]),
        Ok(query(
            QueryCommand::AgentDoctor {
                providers: Vec::new(),
                asset_roots: Vec::new(),
            },
            OutputFormat::Json
        ))
    );
}

/// `agent doctor` usage failures: unknown provider ids, non-absolute or
/// over-limit asset roots, missing option values, and out-of-scope options
/// are all typed usage failures — nothing reaches the probe layer.
#[test]
fn agent_doctor_usage_failures() {
    let usage_reason = |arguments: &[&str]| {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.kind,
            ParseFailureKind::InvalidUsage,
            "{arguments:?}"
        );
        failure.usage.expect("usage reason recorded")
    };
    let path = |spelling: &'static str| UsageContext::Path(spelling);

    for value in ["claude", "CODEX", "codex ", "", "gpt-5"] {
        assert_eq!(
            usage_reason(&["agent", "doctor", "--provider", value]),
            UsageReason::InvalidValue {
                context: path("agent doctor"),
                option: "--provider",
                value: value.to_owned(),
                expected: ExpectedValue::ProviderId,
            },
            "--provider {value:?}"
        );
    }
    // Relative spellings, empties, and ~/$ forms are usage failures at the
    // grammar boundary — never expanded and never probed as guesses.
    for value in ["", "rel/path", "./x", "../x", "~/x", "$HOME/x", "x/"] {
        assert_eq!(
            usage_reason(&["agent", "doctor", "--asset-root", value]),
            UsageReason::InvalidValue {
                context: path("agent doctor"),
                option: "--asset-root",
                value: value.to_owned(),
                expected: ExpectedValue::AssetRootPath,
            },
            "--asset-root {value:?}"
        );
    }
    // A fifth supplied root overruns the declared ceiling.
    assert_eq!(
        usage_reason(&[
            "agent",
            "doctor",
            "--asset-root",
            &doctor_test_root("/a"),
            "--asset-root",
            &doctor_test_root("/b"),
            "--asset-root",
            &doctor_test_root("/c"),
            "--asset-root",
            &doctor_test_root("/d"),
            "--asset-root",
            &doctor_test_root("/e"),
        ]),
        UsageReason::InvalidValue {
            context: path("agent doctor"),
            option: "--asset-root",
            value: ["/a", "/b", "/c", "/d", "/e"]
                .map(doctor_test_root)
                .join(" "),
            expected: ExpectedValue::AssetRootPath,
        }
    );
    for option in ["--provider", "--asset-root"] {
        assert_eq!(
            usage_reason(&["agent", "doctor", option]),
            UsageReason::MissingValue { option },
            "{option} alone"
        );
    }
    // Out-of-scope options and stray operands stay usage failures.
    for (arguments, reason) in [
        (
            &["agent", "doctor", "--include-ended"][..],
            UsageReason::UnknownOption {
                context: path("agent doctor"),
                option: "--include-ended".to_owned(),
            },
        ),
        (
            &["agent", "doctor", "--cursor", AGENT_CURSOR][..],
            UsageReason::UnknownOption {
                context: path("agent doctor"),
                option: "--cursor".to_owned(),
            },
        ),
        (
            &["agent", "doctor", "--label", "x"][..],
            UsageReason::UnknownOption {
                context: path("agent doctor"),
                option: "--label".to_owned(),
            },
        ),
        (
            &["agent", "doctor", "extra"][..],
            UsageReason::UnexpectedArgument {
                context: path("agent doctor"),
                value: "extra".to_owned(),
            },
        ),
    ] {
        assert_eq!(usage_reason(arguments), reason, "{arguments:?}");
    }
}

/// `agent doctor` help resolves on both surfaces to the pinned command topic.
#[test]
fn agent_doctor_help_paths_resolve() {
    assert_eq!(
        parse(&["agent", "doctor", "--help"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::AgentDoctor)))
    );
    assert_eq!(
        parse(&["help", "agent", "doctor"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::AgentDoctor)))
    );
}

/// `agent integration` parses the four local provider-onboarding commands;
/// `explain`/`enable`/`disable` owe `--provider`, `list` accepts it as a
/// filter, and `--adapter-source` must be a literal absolute path.
#[test]
fn agent_integration_commands_parse() {
    use crate::agent_integrations::IntegrationId;
    use crate::invocation::AgentIntegrationCommand;
    let integration = |command: AgentIntegrationCommand| Request::AgentIntegration {
        command,
        format: OutputFormat::Text,
    };
    assert_eq!(
        parse(&["agent", "integration", "list"]),
        Ok(integration(AgentIntegrationCommand::List {
            outdated_only: false,
            provider: None,
            adapter_source: None,
        }))
    );
    assert_eq!(
        parse(&[
            "agent",
            "integration",
            "list",
            "--provider",
            "codex",
            "--format",
            "json",
        ]),
        Ok(Request::AgentIntegration {
            command: AgentIntegrationCommand::List {
                outdated_only: false,
                provider: Some(IntegrationId::Codex),
                adapter_source: None,
            },
            format: OutputFormat::Json,
        })
    );
    assert_eq!(
        parse(&["agent", "integration", "list", "--outdated-only"]),
        Ok(integration(AgentIntegrationCommand::List {
            outdated_only: true,
            provider: None,
            adapter_source: None
        }))
    );
    for args in [
        vec![
            "agent",
            "integration",
            "enable",
            "--provider",
            "codex",
            "--outdated-only",
        ],
        vec!["agent", "list", "--outdated-only"],
        vec![
            "agent",
            "integration",
            "list",
            "--outdated-only",
            "--outdated-only",
        ],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    for (verb, provider) in [
        ("explain", "claude-code"),
        ("enable", "github-copilot"),
        ("disable", "qwen-code"),
    ] {
        let parsed = parse(&["agent", "integration", verb, "--provider", provider]);
        let Request::AgentIntegration { command, .. } = parsed.expect(verb) else {
            panic!("{verb} must resolve to an integration command")
        };
        let (id, ..) = match command {
            AgentIntegrationCommand::Explain {
                provider,
                adapter_source,
            }
            | AgentIntegrationCommand::Enable {
                provider,
                adapter_source,
            }
            | AgentIntegrationCommand::Disable {
                provider,
                adapter_source,
            } => (provider, adapter_source),
            other => panic!("{verb} resolved to {other:?}"),
        };
        assert_eq!(id.as_str(), provider, "{verb} --provider");
    }
    let source = if cfg!(windows) {
        r"C:\adapters"
    } else {
        "/tmp/adapters"
    };
    assert_eq!(
        parse(&[
            "agent",
            "integration",
            "explain",
            "--provider",
            "omp",
            "--adapter-source",
            source,
        ]),
        Ok(integration(AgentIntegrationCommand::Explain {
            provider: IntegrationId::Omp,
            adapter_source: Some(std::path::PathBuf::from(source)),
        }))
    );
}

/// The integration grammar rejects everything outside its declared shape:
/// missing verbs, extra positionals, out-of-scope flags, an absent or
/// invalid `--provider`, and a relative `--adapter-source`.
#[test]
fn agent_integration_commands_fail_closed() {
    for arguments in [
        // Bare sub-family owes a verb.
        &["agent", "integration"][..],
        // Unknown verbs and extra positionals.
        &["agent", "integration", "bogus"][..],
        &["agent", "integration", "list", "extra"][..],
        // `explain`/`enable`/`disable` owe `--provider`.
        &["agent", "integration", "explain"][..],
        &["agent", "integration", "enable"][..],
        &["agent", "integration", "disable"][..],
        // An unknown provider id is a missing/invalid value, not a topic.
        &["agent", "integration", "enable", "--provider", "bogus"][..],
        // `--provider` is single-use and owes its value.
        &["agent", "integration", "list", "--provider"][..],
        &[
            "agent",
            "integration",
            "list",
            "--provider",
            "codex",
            "--provider",
            "pi",
        ][..],
        // `--adapter-source` must be absolute and is single-use.
        &[
            "agent",
            "integration",
            "list",
            "--adapter-source",
            "relative/dir",
        ][..],
        &[
            "agent",
            "integration",
            "list",
            "--adapter-source",
            "/a",
            "--adapter-source",
            "/b",
        ][..],
        // Out-of-scope business flags.
        &["agent", "integration", "list", "--confirm"][..],
        &["agent", "integration", "list", "--project", "x"][..],
        &[
            "agent",
            "integration",
            "enable",
            "--provider",
            "pi",
            "--label",
            "x",
        ][..],
        &["agent", "integration", "list", "--include-ended"][..],
        // A joined spelling on an undeclared name is not a known option.
        &["agent", "integration", "list", "--bogus=codex"][..],
        // `--mouse` belongs to `tui` alone.
        &["agent", "integration", "list", "--mouse"][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // `--format` still applies on the mutation surface.
    assert_eq!(
        parse(&[
            "--format",
            "json",
            "agent",
            "integration",
            "enable",
            "--provider",
            "pi",
        ]),
        Ok(Request::AgentIntegration {
            command: crate::invocation::AgentIntegrationCommand::Enable {
                provider: crate::agent_integrations::IntegrationId::Pi,
                adapter_source: None,
            },
            format: OutputFormat::Json,
        })
    );
}

/// `agent integration` help resolves on both surfaces for the sub-family
/// and each of its four commands.
#[test]
fn agent_integration_help_paths_resolve() {
    assert_eq!(
        parse(&["agent", "integration", "--help"]),
        Ok(Request::Help(HelpTopic::Family(Family::AgentIntegration)))
    );
    assert_eq!(
        parse(&["help", "agent", "integration"]),
        Ok(Request::Help(HelpTopic::Family(Family::AgentIntegration)))
    );
    for (verb, command) in [
        ("list", CommandPath::AgentIntegrationList),
        ("explain", CommandPath::AgentIntegrationExplain),
        ("enable", CommandPath::AgentIntegrationEnable),
        ("disable", CommandPath::AgentIntegrationDisable),
    ] {
        assert_eq!(
            parse(&["agent", "integration", verb, "--help"]),
            Ok(Request::Help(HelpTopic::Command(command))),
            "agent integration {verb} --help"
        );
        assert_eq!(
            parse(&["help", "agent", "integration", verb]),
            Ok(Request::Help(HelpTopic::Command(command))),
            "help agent integration {verb}"
        );
    }
}

/// `workspace close` parses every target shape and always owes `--confirm`.
#[test]
fn workspace_close_parses_targets_and_requires_confirm() {
    for (target, expected) in [
        (ID, Target::Id(ID.into())),
        ("main", Target::Label("main".to_owned())),
        (
            "@workspace:0xab",
            Target::Selector("@workspace:0xab".to_owned()),
        ),
    ] {
        assert_eq!(
            parse(&["workspace", "close", target, "--confirm"]),
            Ok(mutation(
                MutationCommand::WorkspaceClose {
                    target: expected,
                    wait: CloseWait::Wait { timeout_ms: 30_000 },
                },
                OutputFormat::Text
            )),
            "target {target}"
        );
    }
    // `--confirm` is mandatory for the destructive verb, a missing operand
    // names its own requirement, and a stray extra is an unexpected
    // argument — all stay usage failures with their concrete reason.
    let usage_reason = |arguments: &[&str]| {
        let Err(failure) = parse(arguments) else {
            panic!("{arguments:?} must fail");
        };
        assert_eq!(
            failure.kind,
            ParseFailureKind::InvalidUsage,
            "{arguments:?}"
        );
        failure.usage.expect("usage reason recorded")
    };
    assert_eq!(
        usage_reason(&["workspace", "close", ID]),
        UsageReason::MissingRequired {
            context: UsageContext::Path("workspace close"),
            requirement: "'--confirm'",
        }
    );
    assert_eq!(
        usage_reason(&["workspace", "close"]),
        UsageReason::MissingRequired {
            context: UsageContext::Path("workspace close"),
            requirement: "<workspace>",
        }
    );
    assert_eq!(
        usage_reason(&["workspace", "close", ID, "--confirm", "extra"]),
        UsageReason::UnexpectedArgument {
            context: UsageContext::Path("workspace close"),
            value: "extra".to_owned(),
        }
    );
}

/// `workspace close-status` parses only strict stable IDs — the retained
/// record outlives the Workspace's active-projection membership, so labels
/// and selectors never reach the wire.
#[test]
fn workspace_close_status_requires_strict_stable_ids() {
    for (words, format) in [
        (
            vec!["workspace", "close-status", "--workspace", ID2],
            OutputFormat::Text,
        ),
        (
            vec![
                "workspace",
                "close-status",
                "--workspace",
                ID2,
                "--format=json",
            ],
            OutputFormat::Json,
        ),
    ] {
        assert_eq!(
            parse(&words),
            Ok(query(
                QueryCommand::WorkspaceCloseStatus {
                    workspace: WorkbenchWorkspaceId::parse(ID2).unwrap(),
                },
                format
            ))
        );
    }
    for value in [ZERO, "label", "@workspace:0xab", ""] {
        let failure = parse(&["workspace", "close-status", "--workspace", value]).unwrap_err();
        assert_eq!(
            failure.usage,
            Some(UsageReason::InvalidValue {
                context: UsageContext::Path("workspace close-status"),
                option: "--workspace",
                value: value.into(),
                expected: ExpectedValue::WorkspaceId,
            })
        );
    }
    for words in [
        vec!["workspace", "close-status"],
        vec![
            "workspace",
            "close-status",
            "--workspace",
            ID2,
            "--project",
            ID,
        ],
        vec!["workspace", "close-status", "--workspace", ID2, "--confirm"],
        vec!["workspace", "close-status", "extra"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn workspace_close_resume_parses_strict_selectors_and_fences() {
    for generation in ["1", "7", "18446744073709551615"] {
        assert_eq!(
            parse(&[
                "workspace",
                "close-resume",
                "--workspace",
                ID2,
                "--request",
                ID,
                "--generation",
                generation,
                "--confirm"
            ]),
            Ok(mutation(
                MutationCommand::WorkspaceCloseResume {
                    workspace: WorkbenchWorkspaceId::parse(ID2).unwrap(),
                    request: WorkbenchRequestId::parse(ID).unwrap(),
                    generation: WorkbenchTabCloseAttemptGeneration::parse(generation).unwrap(),
                },
                OutputFormat::Text
            ))
        );
    }
    let valid = [
        "workspace",
        "close-resume",
        "--workspace",
        ID2,
        "--request",
        ID,
        "--generation",
        "1",
        "--confirm",
    ];
    for index in [2, 4, 6, 8] {
        let mut words = valid.to_vec();
        words.drain(index..if index == 8 { 9 } else { index + 2 });
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for value in ["0", "abc", "-1", "1.5", ""] {
        let mut words = valid;
        words[7] = value;
        assert_eq!(
            parse(&words).unwrap_err().usage,
            Some(UsageReason::InvalidValue {
                context: UsageContext::Path("workspace close-resume"),
                option: "--generation",
                value: value.into(),
                expected: ExpectedValue::CloseGeneration,
            })
        );
    }
    for (index, option, expected) in [
        (3, "--workspace", ExpectedValue::WorkspaceId),
        (5, "--request", ExpectedValue::CloseRequestId),
    ] {
        let mut words = valid;
        words[index] = "label";
        assert_eq!(
            parse(&words).unwrap_err().usage,
            Some(UsageReason::InvalidValue {
                context: UsageContext::Path("workspace close-resume"),
                option,
                value: "label".into(),
                expected,
            })
        );
    }
    let mut words = valid.to_vec();
    words.extend(["--project", ID]);
    assert!(parse(&words).is_err());
}

#[test]
fn workspace_close_help_paths_resolve() {
    for (verb, command) in [
        ("close", CommandPath::WorkspaceClose),
        ("close-status", CommandPath::WorkspaceCloseStatus),
        ("close-resume", CommandPath::WorkspaceCloseResume),
    ] {
        assert_eq!(
            parse(&["workspace", verb, "--help"]),
            Ok(Request::Help(HelpTopic::Command(command))),
            "workspace {verb} --help"
        );
        assert_eq!(
            parse(&["help", "workspace", verb]),
            Ok(Request::Help(HelpTopic::Command(command))),
            "help workspace {verb}"
        );
    }
}

#[test]
fn per_launch_options_support_default_explicit_and_profile_launches() {
    for selection in [
        vec![],
        vec!["--exec", "C:\\tools\\agent.exe", "--arg", "--literal"],
        vec!["--profile", "dev"],
    ] {
        for panel in [false, true] {
            let mut words = if panel {
                vec!["panel", "split", ID, "--direction", "right", "--label", "p"]
            } else {
                vec!["tab", "create", "--workspace", "main", "--label", "t"]
            };
            words.extend(selection.iter().copied());
            words.extend([
                "--cwd",
                "C:\\代码 space",
                "--env",
                "KEY=literal=值 $()",
                "--env=EMPTY=",
            ]);
            let Request::Creation { command, .. } = parse(&words).unwrap() else {
                panic!("creation");
            };
            let CreationCommand {
                options,
                launch,
                profile,
                ..
            } = command;
            assert_eq!(options.cwd.unwrap().as_str(), "C:\\代码 space");
            assert_eq!(
                options.environment.entries(),
                &[
                    ("KEY".into(), "literal=值 $()".into()),
                    ("EMPTY".into(), "".into())
                ]
            );
            assert_eq!(launch.is_some(), selection.first() == Some(&"--exec"));
            assert_eq!(profile.is_some(), selection.first() == Some(&"--profile"));
        }
    }
}

#[test]
fn per_launch_options_reject_bad_values_and_scope_without_echoing_secrets() {
    for extras in [
        vec!["--env", "TOKEN_SECRET"],
        vec!["--env", "TERM=TOKEN_SECRET"],
        vec!["--env", "Path=TOKEN_SECRET", "--env=PATH=other"],
    ] {
        let mut words = vec!["tab", "create", "--workspace", "main", "--label", "t"];
        words.extend(extras);
        let failure = parse(&words).unwrap_err();
        assert!(!format!("{failure:?}").contains("TOKEN_SECRET"));
        assert!(matches!(
            failure.usage,
            Some(UsageReason::InvalidValue {
                expected: ExpectedValue::LaunchEnvironment,
                ..
            })
        ));
    }
    for words in [
        vec![
            "tab",
            "create",
            "--workspace",
            "main",
            "--label",
            "t",
            "--cwd",
            "relative",
        ],
        vec![
            "tab",
            "create",
            "--workspace",
            "main",
            "--label",
            "t",
            "--cwd",
            "C:\\a",
            "--cwd=C:\\b",
        ],
        vec!["status", "--env", "A=b"],
        vec!["--skill", "--cwd", "C:\\a"],
        vec![
            "launch-profile",
            "create",
            "--name",
            "dev",
            "--exec",
            "C:\\agent.exe",
            "--env",
            "A=b",
        ],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn current_panel_grammar_has_explicit_or_caller_modes() {
    for words in [
        vec!["panel", "current"],
        vec!["panel", "current", "--current"],
    ] {
        assert_eq!(
            parse(&words),
            Ok(Request::Query {
                command: QueryCommand::PanelCurrent(crate::invocation::PanelQueryTarget::Current),
                format: OutputFormat::Text
            })
        );
    }
    for words in [
        vec!["panel", "current", "@panel:0xabc"],
        vec!["panel", "current", "--", "@panel:0xabc"],
    ] {
        assert_eq!(
            parse(&words),
            Ok(Request::Query {
                command: QueryCommand::PanelCurrent(PanelQueryTarget::Panel(Target::Selector(
                    "@panel:0xabc".into()
                ))),
                format: OutputFormat::Text
            })
        );
    }
    for words in [
        vec!["panel", "current", "--panel", "@panel:0xabc", "--current"],
        vec!["panel", "current", "--current", "--current"],
        vec![
            "panel",
            "current",
            "--panel",
            "@panel:0xabc",
            "--panel",
            "@panel:0xabc",
        ],
        vec!["panel", "current", "--panel"],
        vec!["panel", "current", "--panel", "label"],
        vec!["panel", "current", "--panel", "--"],
        vec!["panel", "current", "--current=true"],
        vec!["panel", "current", "--workspace", "ws"],
        vec!["panel", "current", "@panel:0xabc", "@panel:0xdef"],
        vec!["panel", "current", "--", "--current"],
        vec!["panel", "list", "--current"],
        vec!["status", "--panel", "@panel:0xabc"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    assert_eq!(
        parse(&["panel", "current", "--help"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::PanelCurrent)))
    );
    assert_eq!(
        parse(&["help", "panel", "current"]),
        Ok(Request::Help(HelpTopic::Command(CommandPath::PanelCurrent)))
    );
}

#[test]
fn panel_process_info_grammar_rejects_conflicting_targets_before_effects() {
    for words in [
        vec!["panel", "process-info"],
        vec!["panel", "process-info", "--current"],
    ] {
        assert_eq!(
            parse(&words),
            Ok(Request::Query {
                command: QueryCommand::PanelProcessInfo(
                    crate::invocation::PanelQueryTarget::Current
                ),
                format: OutputFormat::Text
            })
        );
    }
    for words in [
        vec!["panel", "process-info", "@panel:0xabc"],
        vec!["panel", "process-info", "--", "@panel:0xabc"],
    ] {
        assert_eq!(
            parse(&words),
            Ok(Request::Query {
                command: QueryCommand::PanelProcessInfo(PanelQueryTarget::Panel(Target::Selector(
                    "@panel:0xabc".into()
                ))),
                format: OutputFormat::Text
            })
        );
    }
    for words in [
        vec!["panel", "process-info", "@panel:0xabc", "--current"],
        vec![
            "panel",
            "process-info",
            "@panel:0xabc",
            "--panel",
            "@panel:0xabc",
        ],
        vec![
            "panel",
            "process-info",
            "--current",
            "--panel",
            "@panel:0xabc",
        ],
        vec!["panel", "process-info", "--current", "--current"],
        vec!["panel", "process-info", "--current=true"],
        vec!["panel", "process-info", "@panel:0xabc", "@panel:0xdef"],
        vec!["panel", "process-info", "--panel"],
        vec!["panel", "process-info", "--panel", "label"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    assert_eq!(
        parse(&["panel", "process-info", "--help"]),
        Ok(Request::Help(HelpTopic::Command(
            CommandPath::PanelProcessInfo
        )))
    );
}

#[test]
fn workspace_launch_preserves_launch_values_in_the_platform_workflow() {
    let request = parse(&[
        "workspace",
        "create",
        "--project",
        "P",
        "--label",
        "开发",
        "--cwd",
        r"C:\project space",
        "--env",
        "DEMO=a=b $literal",
        "--exec",
        r"C:\Windows\System32\cmd.exe",
        "--arg",
        "/K",
    ])
    .unwrap();
    let Request::WorkspaceCreation { command, .. } = request else {
        panic!("directory Workspace launch expected");
    };
    assert!(
        matches!(&command.directory, super::workspace_creation::ParsedWorkspaceCreationDirectory::Explicit(path) if path.as_str() == r"C:\project space")
    );
    assert!(command.options.cwd.is_none());
    assert_eq!(
        command.options.environment.entries(),
        &[("DEMO".to_owned(), "a=b $literal".to_owned())]
    );
    assert!(
        matches!(command.launch, Some(super::program::ParsedLaunch::Absolute(launch)) if launch.arguments() == ["/K"])
    );
    for tail in [
        vec!["--cols", "80"],
        vec![
            "--cwd",
            r"C:\project",
            "--profile",
            "P",
            "--exec",
            r"C:\cmd.exe",
        ],
    ] {
        let mut words = vec!["workspace", "create", "--project", "P", "--label", "W"];
        words.extend(tail);
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

fn doctor_test_root(posix: &str) -> String {
    if cfg!(windows) {
        format!("C:{posix}")
    } else {
        posix.to_owned()
    }
}

#[test]
fn agent_read_targets_accept_exactly_one_auto_session_or_name_selector() {
    use orchbee_agent_observation_protocol::{AgentName, AgentSessionId};
    for verb in ["get", "explain", "wait"] {
        for (args, expected) in [
            (
                vec!["alpha"],
                AgentTarget::Auto {
                    target: AgentSessionId::parse("alpha").unwrap(),
                },
            ),
            (
                vec!["--agent-session", "alpha"],
                AgentTarget::Session {
                    session_id: AgentSessionId::parse("alpha").unwrap(),
                },
            ),
            (
                vec!["--name=alpha"],
                AgentTarget::Name {
                    name: AgentName::parse("alpha").unwrap(),
                },
            ),
        ] {
            let parsed = parse(&[vec!["agent", verb], args].concat()).unwrap();
            let Request::Query { command, .. } = parsed else {
                panic!("query");
            };
            let target = match command {
                QueryCommand::AgentGet(target)
                | QueryCommand::AgentExplain(target)
                | QueryCommand::AgentWait {
                    session: target, ..
                } => target,
                _ => panic!("target query"),
            };
            assert_eq!(target, expected);
        }
        for args in [
            vec!["alpha", "--name", "beta"],
            vec!["--name", "alpha", "--agent-session", "beta"],
            vec!["--name", "Upper"],
            vec!["--agent-session", "bad id"],
            vec!["--name", "alpha", "extra"],
        ] {
            assert!(parse(&[vec!["agent", verb], args].concat()).is_err());
        }
    }
}

#[cfg(windows)]
#[test]
fn projection_selectors_share_one_grammar_across_agent_operations() {
    use super::{
        AgentSelector,
        client_control::{ClientCommand, ClientNavigationTarget},
    };
    for (token, expected) in [
        ("@agent:codex", AgentSelector::Kind("codex".into())),
        (
            "@agent:grok-build",
            AgentSelector::Kind("grok-build".into()),
        ),
        ("@agent:1", AgentSelector::Ordinal(1)),
        ("@agent:4096", AgentSelector::Ordinal(4096)),
    ] {
        for (verb, tail) in [
            ("get", None),
            ("explain", None),
            ("read", None),
            ("wait", None),
            ("focus", None),
            ("prompt", Some("literal prompt")),
            ("send-keys", Some("enter")),
        ] {
            let mut words = vec!["agent", verb, token];
            words.extend(tail);
            let parsed = parse(&words).unwrap();
            let target = match parsed {
                Request::Query {
                    command:
                        QueryCommand::AgentGet(target)
                        | QueryCommand::AgentExplain(target)
                        | QueryCommand::TerminalOutput {
                            target: super::OutputTarget::Agent(target),
                            ..
                        }
                        | QueryCommand::AgentWait {
                            session: target, ..
                        },
                    ..
                }
                | Request::Mutation {
                    command:
                        MutationCommand::AgentPrompt { target, .. }
                        | MutationCommand::AgentSendKeys { target, .. },
                    ..
                }
                | Request::Client {
                    command:
                        ClientCommand::Navigate {
                            target: ClientNavigationTarget::Agent(target),
                            ..
                        },
                    ..
                } => target,
                _ => panic!("Agent operation must preserve its selector: {words:?}"),
            };
            assert_eq!(
                target,
                AgentTarget::Projection {
                    selector: expected.clone()
                },
                "{words:?}"
            );
        }
    }
    for token in [
        "@agent:",
        "@agent:0",
        "@agent:01",
        "@agent:4097",
        "@agent:Codex",
        "@agent:bad/name",
    ] {
        assert!(parse(&["agent", "get", token]).is_err(), "{token}");
    }
    assert!(parse(&["agent", "prompt", "@agent:1", "text", "--name", "alias"]).is_err());
}

#[test]
fn agent_terminal_grammar_validates_all_keys_and_exclusive_targets_before_effects() {
    let command = parse(&[
        "agent",
        "send-keys",
        "--name",
        "alpha",
        "enter",
        "ctrl+shift+a",
    ]);
    assert!(matches!(command, Ok(Request::Mutation {
        command: MutationCommand::AgentSendKeys { target: AgentTarget::Name { .. }, keys }, ..
    }) if keys.len() == 2));
    assert!(matches!(
        parse(&[
            "agent",
            "read",
            "alpha",
            "--source",
            "recent-unwrapped",
            "--lines",
            "2"
        ]),
        Ok(Request::Query {
            command: QueryCommand::TerminalOutput {
                target: super::OutputTarget::Agent(_),
                selection: super::OutputSelection {
                    source: super::OutputSource::RecentUnwrapped,
                    lines: Some(2)
                },
                ..
            },
            ..
        })
    ));
    for words in [
        vec!["agent", "send-keys", "alpha"],
        vec!["agent", "send-keys", "alpha", "enter", "not-a-key"],
        vec![
            "agent",
            "send-keys",
            "--name",
            "alpha",
            "--agent-session",
            "s1",
            "enter",
        ],
        vec!["agent", "read", "alpha", "--name", "beta"],
        vec!["agent", "read", "alpha", "--rows", "20"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    let mut words = vec!["agent", "send-keys", "alpha"];
    words.extend(std::iter::repeat_n("enter", 64));
    assert!(parse(&words).is_ok());
    words.push("enter");
    assert!(parse(&words).is_err());
}

#[test]
fn terminal_and_agent_send_share_extended_keys_and_reject_partial_invalid_sequences() {
    for prefix in [
        vec!["terminal", "send", "00000000000000000000000000000001"],
        vec!["agent", "send-keys", "alpha"],
    ] {
        let terminal = prefix[0] == "terminal";
        let mut args = prefix.clone();
        for key in ["f1", "space", "super+c", "中", "shift+?", "plus"] {
            if terminal {
                args.push("--key");
            }
            args.push(key);
        }
        assert!(parse(&args).is_ok(), "{args:?}");
        if terminal {
            args.push("--key");
        }
        args.push("super+ctrl+control+a");
        assert!(parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn prompt_grammar_keeps_text_literal_and_wait_options_explicit() {
    assert!(
        matches!(parse(&["agent", "prompt", "--name", "alpha", "line one\nline two", "--wait", "--until", "succeeded", "--timeout", "1234"]),
        Ok(Request::Mutation { command: MutationCommand::AgentPrompt { text, wait: Some(wait), .. }, .. }) if text == "line one\nline two" && wait.timeout_ms == Some(crate::invocation::AgentWaitTimeout::Milliseconds(1234)))
    );
    for words in [
        vec!["agent", "prompt", "alpha"],
        vec!["agent", "prompt", "alpha", ""],
        vec!["agent", "prompt", "alpha", "one", "two"],
        vec!["agent", "prompt", "alpha", "text", "--until", "succeeded"],
        vec!["agent", "prompt", "alpha", "text", "--timeout", "1234"],
        vec!["agent", "prompt", "alpha", "text", "--wait", "--wait"],
        vec!["agent", "get", "alpha", "--wait"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    assert!(parse(&["agent", "prompt", "alpha", "--", "--literal"]).is_ok());
}

#[test]
fn infinite_agent_wait_is_explicit_and_rejects_conflicting_or_unscoped_flags() {
    use crate::invocation::{AgentPromptWait, AgentWaitCondition};
    assert!(
        matches!(parse(&["agent", "wait", "alpha", "--forever", "--until", "done", "--until", "blocked"]),
        Ok(Request::Query { command: QueryCommand::AgentWait { timeout_ms: Some(crate::invocation::AgentWaitTimeout::Forever), until, .. }, .. }) if until == Some(vec![AgentWaitCondition::Done, AgentWaitCondition::Blocked]))
    );
    assert!(matches!(
        parse(&["agent", "prompt", "alpha", "task", "--wait", "--forever"]),
        Ok(Request::Mutation {
            command: MutationCommand::AgentPrompt {
                wait: Some(AgentPromptWait {
                    timeout_ms: Some(crate::invocation::AgentWaitTimeout::Forever),
                    ..
                }),
                ..
            },
            ..
        })
    ));
    for words in [
        vec!["agent", "wait", "alpha", "--forever", "--timeout", "100"],
        vec!["agent", "wait", "alpha", "--forever", "--forever"],
        vec!["agent", "prompt", "alpha", "task", "--forever"],
        vec![
            "agent",
            "prompt",
            "alpha",
            "task",
            "--wait",
            "--forever",
            "--timeout",
            "100",
        ],
        vec!["agent", "get", "alpha", "--forever"],
        vec![
            "terminal",
            "wait-output",
            "alpha",
            "--match",
            "ready",
            "--forever",
        ],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn agent_panel_selectors_are_exclusive_and_preserve_following_operands() {
    let panel = "22222222222222222222222222222222";
    for verb in ["get", "explain", "wait", "read", "resolve"] {
        let parsed = parse_request(
            ["agent", verb, "--panel", panel]
                .into_iter()
                .map(OsString::from),
        )
        .unwrap();
        assert!(!matches!(parsed, Request::Help { .. }));
        for extra in [
            ["--agent-session", "sess"],
            ["--name", "alpha"],
            [panel, "extra"],
        ] {
            assert!(
                parse_request(
                    ["agent", verb, "--panel", panel, extra[0], extra[1]]
                        .into_iter()
                        .map(OsString::from)
                )
                .is_err()
            );
        }
        assert!(
            parse_request(
                ["agent", verb, "--panel", "label"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_err()
        );
        assert!(
            parse_request(
                ["agent", verb, "@panel:0x222"]
                    .into_iter()
                    .map(OsString::from)
            )
            .is_ok()
        );
    }
    for words in [
        vec!["agent", "prompt", "--panel", panel, "literal"],
        vec!["agent", "send-keys", "--panel", panel, "enter"],
        vec!["agent", "rename", "--panel", panel, "alpha"],
        vec!["agent", "rename", "--panel", panel, "--clear"],
    ] {
        assert!(parse_request(words.into_iter().map(OsString::from)).is_ok());
    }
}

#[test]
fn panel_rename_admits_exact_targets_and_normalizes_only_its_label() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    for (flags, expected) in [
        (vec!["--label", "  标题\u{85}"], Some("标题")),
        (vec!["--label="], None),
        (vec!["--clear"], None),
    ] {
        let mut words = vec!["panel", "rename", "@panel:0x1234"];
        words.extend(flags);
        let Request::Mutation {
            command: MutationCommand::PanelRename { target, label },
            ..
        } = parse(&words).unwrap()
        else {
            panic!("panel rename")
        };
        assert_eq!(target.selector_identity(), Some(format!("{:032x}", 0x1234)));
        assert_eq!(label.custom_label().map(|value| value.as_str()), expected);
    }
    for words in [
        vec!["panel", "rename", "@panel:0x1234"],
        vec!["panel", "rename", "label", "--clear"],
        vec!["panel", "rename", "--clear"],
        vec!["panel", "rename", "@panel:0x1234", "--clear", "--label=x"],
        vec!["panel", "rename", "@panel:0x1234", "--current", "--clear"],
        vec!["panel", "rename", "@panel:0x1234", "extra", "--clear"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    assert!(
        parse(&[
            "panel",
            "rename",
            "@panel:0x1234",
            "--label",
            &"🐝".repeat(513)
        ])
        .is_err()
    );
}

#[test]
fn creation_optional_targets_focus_and_ratios_are_validated_before_admission() {
    for args in [
        vec!["tab", "create"],
        vec!["panel", "split", "--direction=left"],
        vec!["panel", "split", "--current", "--direction=up"],
    ] {
        let Request::Creation { command, .. } = parse(&args).unwrap() else {
            panic!("creation");
        };
        assert!(command.label.is_none());
        assert!(!command.focus);
        assert_eq!(command.timeout_ms, 30_000);
    }
    for (input, expected) in [("-10", 0.1), ("0.7", 0.7), ("5", 0.9)] {
        let Request::Creation { command, .. } = parse(&[
            "panel",
            "split",
            ID,
            "--direction=down",
            "--ratio",
            input,
            "--focus",
            "--client",
            ID2,
            "--timeout=1",
        ])
        .unwrap() else {
            panic!("creation");
        };
        assert!(command.focus);
        assert_eq!(command.client.unwrap().as_str(), ID2);
        assert_eq!(command.timeout_ms, 1);
        let CreationKind::Split { target, ratio, .. } = command.kind else {
            panic!("split");
        };
        assert_eq!(target, Some(Target::Id(ID.into())));
        assert_eq!(serde_json::json!(ratio), expected);
    }
    for extra in [
        vec!["--focus", "--no-focus"],
        vec!["--focus=true"],
        vec!["--ratio=NaN"],
        vec!["--ratio=inf"],
        vec!["--ratio=1e999"],
        vec!["--timeout=0"],
        vec!["--timeout=3600001"],
        vec!["--current", "--panel", ID],
        vec![ID, "--current"],
        vec![ID, "--panel", ID],
        vec!["--detach"],
        vec!["--request-id", ID],
    ] {
        let mut args = vec!["panel", "split", "--direction=right"];
        args.extend(extra);
        assert!(parse(&args).is_err(), "{args:?}");
    }
    assert!(parse(&["tab", "create", "--ratio=0.5"]).is_err());
}

#[test]
fn panel_input_requires_one_selector_and_closed_policy_without_effect_options() {
    use orchbee_workbench_protocol::PanelRightClickTarget;
    for (words, explicit, policy) in [
        (
            vec!["panel", "input", "@panel:0x6", "--right-click=panel"],
            true,
            PanelRightClickTarget::Panel,
        ),
        (
            vec!["panel", "input", "@panel:0x6", "--right-click", "orchbee"],
            true,
            PanelRightClickTarget::Orchbee,
        ),
        (
            vec!["--right-click=panel", "panel", "input", "--current"],
            false,
            PanelRightClickTarget::Panel,
        ),
    ] {
        let Request::Mutation {
            command:
                MutationCommand::PanelInput {
                    target,
                    right_click,
                },
            ..
        } = parse(&words).unwrap()
        else {
            panic!("input")
        };
        assert_eq!(target.is_some(), explicit);
        assert_eq!(right_click, policy);
    }
    for words in [
        vec!["panel", "input", "--right-click=panel"],
        vec!["panel", "input", "--current"],
        vec![
            "panel",
            "input",
            "@panel:0x6",
            "--current",
            "--right-click=panel",
        ],
        vec![
            "panel",
            "input",
            "--panel=@panel:0x6",
            "--current",
            "--right-click=panel",
        ],
        vec![
            "panel",
            "input",
            "@panel:0x6",
            "@panel:0x7",
            "--right-click=panel",
        ],
        vec!["panel", "input", "--current", "--right-click=pane"],
        vec![
            "panel",
            "input",
            "--current",
            "--right-click=invalid-target",
        ],
        vec!["panel", "input", "--current", "--right-click="],
        vec!["panel", "input", "--current", "--right-click=Panel"],
        vec![
            "panel",
            "input",
            "--current",
            "--right-click=panel",
            "--right-click=orchbee",
        ],
        vec![
            "panel",
            "input",
            "--current",
            "--right-click=panel",
            "--focus",
        ],
        vec![
            "panel",
            "input",
            "--current",
            "--right-click=panel",
            "--client=00000000000000000000000000000001",
        ],
        vec!["panel", "get", "@panel:0x6", "--right-click=panel"],
        vec!["tab", "create", "--right-click=panel"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    for (value, expected) in [
        ("orchbee", PanelRightClickTarget::Orchbee),
        ("panel", PanelRightClickTarget::Panel),
    ] {
        let Request::Creation { command, .. } = parse(&[
            "panel",
            "split",
            "@panel:0x6",
            "--direction=right",
            "--right-click",
            value,
        ])
        .unwrap() else {
            panic!("creation")
        };
        assert!(
            matches!(command.kind, CreationKind::Split { right_click, .. } if right_click == expected)
        );
    }
}

#[test]
fn tab_index_grammar_is_bounded_exclusive_and_order_independent() {
    for (word, expected) in [("0", 0), ("2", 2), ("256", 256), ("002", 2)] {
        for args in [
            vec!["tab", "move", ID, "--insert-index", word],
            vec!["--insert-index", word, "tab", "move", ID],
        ] {
            assert_eq!(
                parse(&args),
                Ok(mutation(
                    MutationCommand::TabMoveIndex {
                        target: Target::Id(ID.into()),
                        insert_index: orchbee_workbench_protocol::WorkbenchTabInsertIndex::new(
                            expected
                        )
                        .unwrap(),
                    },
                    OutputFormat::Text
                ))
            );
        }
    }
    for word in [
        "",
        "-1",
        "+1",
        "257",
        "1.5",
        "1e1",
        "NaN",
        " 1",
        "9999999999999999999999999",
    ] {
        assert!(
            parse(&["tab", "move", ID, "--insert-index", word]).is_err(),
            "{word}"
        );
    }
    for args in [
        vec!["tab", "move", ID],
        vec![
            "tab",
            "move",
            ID,
            "--insert-index",
            "1",
            "--direction",
            "first",
        ],
        vec!["--direction=first", "tab", "--insert-index=1", "move", ID],
        vec!["tab", "move", ID, "--insert-index=1", "--insert-index=2"],
        vec!["tab", "rename", ID, "--insert-index=1", "--label=x"],
        vec!["panel", "move", ID, "--insert-index=1"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn creation_directory_policies_are_scoped_and_preserve_absolute_overrides() {
    for (value, expected) in [
        ("follow", CreationDirectory::Follow),
        ("home", CreationDirectory::Home),
        ("current", CreationDirectory::Current),
        ("workspace", CreationDirectory::Workspace),
        (r"C:\项目 dir", CreationDirectory::Explicit),
    ] {
        for prefix in [
            vec!["tab", "create"],
            vec!["panel", "split", "--direction", "right"],
        ] {
            let mut args = prefix;
            args.extend(["--cwd", value]);
            let Request::Creation { command, .. } = parse(&args).unwrap() else {
                panic!("creation");
            };
            assert_eq!(command.directory, expected);
            assert_eq!(
                command
                    .options
                    .cwd
                    .as_ref()
                    .map(WorkbenchDirectoryPath::as_str),
                if expected == CreationDirectory::Explicit {
                    Some(value)
                } else {
                    None
                }
            );
        }
    }
    for args in [
        vec!["tab", "create", "--cwd", "Follow"],
        vec!["tab", "create", "--cwd", "relative"],
        vec!["tab", "create", "--cwd", "home", "--cwd", "current"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
}

#[test]
fn current_panel_queries_accept_explicit_clients_and_refuse_ignored_selectors() {
    let client = "12345678901234567890123456789012";
    for verb in ["current", "process-info"] {
        for words in [
            vec!["panel", verb, "--client", client],
            vec!["--client", client, "panel", verb, "--current"],
        ] {
            let Ok(Request::Query { command, .. }) = parse(&words) else {
                panic!("{words:?}")
            };
            let (QueryCommand::PanelCurrent(target) | QueryCommand::PanelProcessInfo(target)) =
                command
            else {
                panic!()
            };
            assert_eq!(
                target,
                PanelQueryTarget::Client(super::WorkbenchClientId::parse(client).unwrap())
            );
        }
        for words in [
            vec!["panel", verb, "--panel", "@panel:0xabc", "--client", client],
            vec!["panel", verb, "--client", client, "--client", client],
            vec!["panel", verb, "--client", "bad"],
            vec!["panel", verb, "--client"],
            vec!["panel", verb, "--client", client, "--timeout", "20"],
            vec!["panel", verb, "--client", client, "--detach"],
        ] {
            assert!(parse(&words).is_err(), "{words:?}");
        }
    }
    assert!(parse(&["panel", "process-info", "@panel:0xabc", "--client", client]).is_err());
    for topic in [CommandPath::PanelCurrent, CommandPath::PanelProcessInfo] {
        for locale in [
            crate::localization::Locale::EnUs,
            crate::localization::Locale::ZhCn,
        ] {
            assert!(
                String::from_utf8(crate::cli::help(HelpTopic::Command(topic), locale).stdout)
                    .unwrap()
                    .contains("--client")
            );
        }
    }
}

#[test]
fn agent_changes_grammar_requires_bounded_explicit_continuation() {
    #[cfg(windows)]
    assert!(parse(&["agent", "changes", "--cursor", AGENT_CURSOR]).is_err());
    assert_eq!(
        parse(&["agent", "changes"]),
        Ok(query(
            QueryCommand::AgentChanges {
                limit: 50,
                cursor: None
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&[
            "agent",
            "changes",
            "--cursor",
            AGENT_PROTOCOL_CURSOR,
            "--limit=100",
            "--format=json"
        ]),
        Ok(query(
            QueryCommand::AgentChanges {
                limit: 100,
                cursor: Some(
                    orchbee_agent_observation_protocol::AgentChangesCursor::parse(
                        AGENT_PROTOCOL_CURSOR
                    )
                    .unwrap()
                )
            },
            OutputFormat::Json
        ))
    );
    for arguments in [
        vec!["agent", "changes", "extra"],
        vec!["agent", "changes", "--include-ended"],
        vec!["agent", "changes", "--limit", "0"],
        vec!["agent", "changes", "--limit", "101"],
        vec!["agent", "changes", "--limit", "2", "--limit", "2"],
        vec!["agent", "changes", "--cursor", "bad"],
        vec!["agent", "changes", "--cursor"],
        vec!["agent", "changes", "--timeout", "100"],
    ] {
        assert!(parse(&arguments).is_err(), "{arguments:?}");
    }
}

#[test]
fn prompt_client_selection_requires_wait_and_one_exact_client() {
    let id = "11111111111111111111111111111111";
    assert!(
        matches!(parse(&["agent","prompt","alpha","task","--wait","--client",id]),
        Ok(Request::Mutation {command:MutationCommand::AgentPrompt {wait:Some(wait), ..}, ..}) if wait.client.as_ref().is_some_and(|client| client.as_str()==id))
    );
    for tail in [
        vec!["--client", id],
        vec!["--wait", "--client", "bad"],
        vec!["--wait", "--client", id, "--client", id],
    ] {
        let mut args = vec!["agent", "prompt", "alpha", "task"];
        args.extend(tail);
        assert!(parse(&args).is_err());
    }
}
#[cfg(windows)]
#[test]
fn named_session_selection_preserves_complete_grammar_and_value_ownership() {
    use super::parse_selected_request;
    let parse = |words: &[&str]| parse_selected_request(words.iter().map(OsString::from));
    for words in [
        vec!["--session", "Work", "status"],
        vec!["status", "--session=Work"],
    ] {
        let request = parse(&words).unwrap();
        assert_eq!(request.session.unwrap().as_str(), "Work");
        assert_eq!(
            request.request,
            parse_request(["status"].into_iter().map(OsString::from)).unwrap()
        );
    }
    for words in [
        vec!["--session", "..", "status"],
        vec!["status", "--session="],
        vec!["--session", "a", "--session=b", "status"],
        vec!["--session", "--"],
        vec!["status", "--", "--session", "a"],
    ] {
        assert!(parse(&words).is_err(), "{words:?}");
    }
    let failure = parse(&["--session=..", "status", "--format=json"]).unwrap_err();
    assert_eq!(failure.envelope, super::FailureEnvelope::Json);
    let offline = parse(&["--session", "Work", "--help"]).unwrap();
    assert_eq!(offline.request, super::Request::RootHelp);
    let launch = parse(&[
        "tab",
        "create",
        "--workspace",
        "main",
        "--exec",
        r"C:\Windows\System32\cmd.exe",
        "--arg=--session",
        "--session",
        "work",
    ])
    .unwrap();
    assert_eq!(launch.session.unwrap().as_str(), "work");
    let agent = parse(&[
        "--session",
        "work",
        "agent",
        "get",
        "--agent-session",
        "00000000000000000000000000000001",
    ]);
    let agent = agent.unwrap();
    assert_eq!(agent.session.unwrap().as_str(), "work");
    assert!(matches!(
        agent.request,
        Request::Query {
            command: QueryCommand::AgentGet(AgentTarget::Session { session_id }),
            ..
        } if session_id.as_str() == "00000000000000000000000000000001"
    ));
    // A runtime selector alone must not become an Agent target.
    assert!(parse(&["--session", "work", "agent", "get"]).is_err());
    // The global selector applies to Workbench queries; it is not the retired status filter.
    let status = parse(&["--session", "work", "status"]).unwrap();
    assert_eq!(status.session.unwrap().as_str(), "work");
    assert!(parse(&["session", "start"]).is_err(), "retired spelling");
    // `session` now names the admitted named-Session family, so it has a help topic.
    assert!(parse(&["help", "session"]).is_ok());
}

#[cfg(windows)]
#[test]
fn workspace_creation_defaults_are_accepted_with_and_without_session_selection() {
    for words in [
        vec!["workspace", "create", "--label", "w"],
        vec!["workspace", "create", "--project", "p"],
        vec![
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "w",
            "--env",
            "A=B",
        ],
        vec![
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "w",
            "--exec",
            r"C:\cmd.exe",
        ],
        vec![
            "workspace",
            "create",
            "--project",
            "p",
            "--label",
            "w",
            "--cwd",
            "relative",
        ],
        vec!["workspace", "create", "--cwd", "home"],
    ] {
        let ordinary = parse_request(words.iter().map(OsString::from)).unwrap();
        assert!(
            matches!(ordinary, Request::WorkspaceCreation { .. }),
            "{words:?}"
        );
        let selected = super::parse_selected_request(
            ["--session", "work"]
                .iter()
                .chain(words.iter())
                .map(OsString::from),
        )
        .unwrap();
        assert_eq!(selected.request, ordinary);
        assert_eq!(selected.session.unwrap().as_str(), "work");
    }
}

#[test]
fn workspace_move_grammar_covers_boundaries_blocks_and_scope() {
    for value in ["0", "1", "1024", "002"] {
        assert_eq!(
            parse(&["workspace", "move", ID, "--insert-index", value]),
            Ok(mutation(
                MutationCommand::WorkspaceMove {
                    target: Target::Id(ID.into()),
                    insert_index: orchbee_workbench_protocol::WorkbenchWorkspaceInsertIndex::new(
                        value.parse().unwrap()
                    )
                    .unwrap()
                },
                OutputFormat::Text
            ))
        );
    }
    assert_eq!(
        parse(&["--before=third", "workspace", "move-block", "second", ID]),
        Ok(mutation(
            MutationCommand::WorkspaceMoveBlock {
                targets: vec![Target::Label("second".into()), Target::Id(ID.into())],
                before: Some(Target::Label("third".into()))
            },
            OutputFormat::Text
        ))
    );
    for value in ["-1", "1025", "1.5", "+1", "1e1", ""] {
        assert!(
            parse(&["workspace", "move", ID, "--insert-index", value]).is_err(),
            "{value}"
        );
    }
    for args in [
        vec!["workspace", "move", ID],
        vec![
            "workspace",
            "move",
            ID,
            "--insert-index=1",
            "--before=other",
        ],
        vec!["workspace", "move-block"],
        vec!["workspace", "move-block", ID, "--insert-index=1"],
        vec!["workspace", "move-block", ID, "--before=a", "--before=b"],
        vec!["workspace", "list", "--before=a"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    let mut maximum = vec!["workspace", "move-block"];
    maximum.extend(std::iter::repeat_n(ID, 1024));
    assert!(parse(&maximum).is_ok());
    maximum.push(ID);
    assert!(parse(&maximum).is_err());
}

#[cfg(windows)]
#[test]
fn reusable_layout_export_has_closed_targets_options_and_total_deadline() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    for words in [
        vec!["layout", "export"],
        vec!["layout", "export", "--current"],
        vec!["layout", "export", "--tab", "named", "--timeout", "3600000"],
        vec![
            "--format=json",
            "layout",
            "export",
            "--panel",
            "@panel:0xabcd",
            "--client",
            "11111111111111111111111111111111",
        ],
    ] {
        let Request::LayoutExport { command, .. } = parse(&words).unwrap() else {
            panic!("layout export expected")
        };
        assert!((1..=3_600_000).contains(&command.timeout_ms));
    }
    for suffix in [
        vec!["extra"],
        vec!["--tab", "a", "--panel", "@panel:0xabcd"],
        vec!["--tab", "a", "--current"],
        vec!["--panel", "@panel:0xabcd", "--current"],
        vec!["--workspace", "a"],
        vec!["--file", "a"],
        vec!["--confirm"],
        vec!["--focus"],
        vec!["--client", "bad"],
        vec!["--timeout", "0"],
        vec!["--timeout", "3600001"],
    ] {
        let mut words = vec!["layout", "export"];
        words.extend(suffix);
        assert!(parse(&words).is_err(), "{words:?}");
    }
}
#[cfg(windows)]
#[test]
fn reusable_layout_apply_requires_a_source_and_confirmed_exclusive_destination() {
    let parse = |words: &[&str]| parse_request(words.iter().map(OsString::from));
    for words in [
        vec!["layout", "apply", "--stdin"],
        vec![
            "layout",
            "apply",
            "--file",
            "中文 file.json",
            "--workspace",
            "main",
            "--focus",
        ],
        vec![
            "layout",
            "apply",
            "--stdin",
            "--tab",
            "old",
            "--confirm",
            "--no-focus",
        ],
        vec![
            "layout",
            "apply",
            "--stdin",
            "--workspace",
            "work",
            "--label",
            "new",
            "--timeout=1",
        ],
    ] {
        let Request::LayoutApply { command, .. } = parse(&words).expect("admitted apply") else {
            panic!("apply expected");
        };
        assert!((1..=3_600_000).contains(&command.timeout_ms));
    }
    for suffix in [
        vec![],
        vec!["--tab", "old"],
        vec!["--confirm"],
        vec!["--file", "x", "--stdin"],
        vec!["--tab", "old", "--confirm", "--workspace", "work"],
        vec!["--workspace", "work", "--standalone"],
        vec!["--focus", "--no-focus"],
        vec!["extra"],
        vec!["--timeout=0"],
        vec!["--client=bad"],
        vec!["--panel=x"],
    ] {
        let mut words = vec!["layout", "apply"];
        if !suffix.is_empty() {
            words.push("--stdin");
        }
        words.extend(suffix);
        assert!(parse(&words).is_err(), "{words:?}");
    }
}

#[test]
fn manifest_commands_accept_only_their_owned_options() {
    assert!(matches!(
        parse(&["server", "agent-manifests"]),
        Ok(Request::AgentManifests {
            command: ManifestCommand::Current,
            ..
        })
    ));
    assert!(
        parse(&[
            "server",
            "reload-agent-manifests",
            "--request-id",
            "11111111111111111111111111111111"
        ])
        .is_ok()
    );
    assert!(
        parse(&[
            "server",
            "agent-manifests-status",
            "11111111111111111111111111111111"
        ])
        .is_ok()
    );
    for args in [
        vec!["server", "agent-manifests-status"],
        vec!["server", "reload-agent-manifests", "--request-id", "bad"],
        vec![
            "server",
            "agent-manifests",
            "--request-id",
            "11111111111111111111111111111111",
        ],
        vec!["server", "reload-agent-manifests", "--confirm"],
    ] {
        assert!(parse(&args).is_err());
    }
}

#[test]
#[cfg(windows)]
fn config_reset_keys_grammar_is_offline_and_rejects_effect_options() {
    assert_eq!(
        parse(&["config", "reset-keys"]),
        Ok(Request::ConfigResetKeys {
            format: OutputFormat::Text
        })
    );
    assert_eq!(
        parse(&["--format=json", "config", "reset-keys"]),
        Ok(Request::ConfigResetKeys {
            format: OutputFormat::Json
        })
    );
    for args in [
        vec!["config", "reset-keys", "extra"],
        vec!["config", "reset-keys", "--force"],
        vec!["config", "reset-keys", "--current"],
        vec!["config", "reset-keys", "--timeout=20"],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    for args in [
        ["config", "reset-keys", "--help"],
        ["help", "config", "reset-keys"],
    ] {
        assert_eq!(
            parse(&args),
            Ok(Request::Help(HelpTopic::Command(
                CommandPath::ConfigResetKeys
            )))
        );
    }
}

#[test]
fn notification_defaults_and_joined_flags_normalize_before_any_effect() {
    use orchbee_workbench_protocol::{NotificationPosition, NotificationSound};
    let Request::Notification { message, format } =
        parse(&["notification", "show", "  build  ready  "]).unwrap()
    else {
        panic!()
    };
    assert_eq!(message.title, "build ready");
    assert_eq!(message.body, None);
    assert_eq!(message.position, None);
    assert_eq!(message.sound, NotificationSound::None);
    assert_eq!(format, OutputFormat::Text);
    let Request::Notification { message, format } = parse(&[
        "--format=json",
        "notification",
        "show",
        "done",
        "--position=top-left",
        "--sound=request",
        "--body=  ",
    ])
    .unwrap() else {
        panic!()
    };
    assert_eq!(message.body, None);
    assert_eq!(message.position, Some(NotificationPosition::TopLeft));
    assert_eq!(message.sound, NotificationSound::Request);
    assert_eq!(format, OutputFormat::Json);
}

#[test]
fn closes_wait_by_default_and_admit_detach_timeout_and_dry_run() {
    let close = |args: &[&str]| match parse(args) {
        Ok(Request::Mutation {
            command:
                MutationCommand::TabClose { wait, .. } | MutationCommand::WorkspaceClose { wait, .. },
            ..
        }) => Ok(wait),
        Ok(other) => panic!("{other:?}"),
        Err(failure) => Err(failure),
    };
    for family in ["tab", "workspace"] {
        assert_eq!(
            close(&[family, "close", "x", "--confirm"]),
            Ok(CloseWait::Wait { timeout_ms: 30_000 })
        );
        assert_eq!(
            close(&[family, "close", "x", "--confirm", "--timeout", "500"]),
            Ok(CloseWait::Wait { timeout_ms: 500 })
        );
        assert_eq!(
            close(&[family, "close", "x", "--confirm", "--detach"]),
            Ok(CloseWait::Detach)
        );
        // A preview discards nothing, so it needs no confirmation.
        assert_eq!(
            close(&[family, "close", "x", "--dry-run"]),
            Ok(CloseWait::DryRun)
        );
        assert!(close(&[family, "close", "x"]).is_err());
        assert!(
            close(&[
                family,
                "close",
                "x",
                "--confirm",
                "--detach",
                "--timeout",
                "5"
            ])
            .is_err()
        );
        assert!(close(&[family, "close", "x", "--dry-run", "--detach"]).is_err());
    }
}

#[test]
fn format_and_agent_target_diagnostics_point_at_the_invoked_command() {
    let reason = |args: &[&str]| parse(args).unwrap_err().usage.unwrap();
    assert_eq!(
        reason(&["tab", "list", "--format", "yaml"]),
        UsageReason::InvalidValue {
            context: UsageContext::Path("tab list"),
            option: "--format",
            value: "yaml".to_owned(),
            expected: ExpectedValue::Format,
        }
    );
    // Option values are not mistaken for command words.
    assert_eq!(
        reason(&["--format", "yaml", "tab", "rename", "x", "--label", "list"]),
        UsageReason::InvalidValue {
            context: UsageContext::Path("tab rename"),
            option: "--format",
            value: "yaml".to_owned(),
            expected: ExpectedValue::Format,
        }
    );
    assert_eq!(
        reason(&["agent", "explain", "@workspace:0x91"]),
        UsageReason::InvalidValue {
            context: UsageContext::Path("agent explain"),
            option: "<target>",
            value: "@workspace:0x91".to_owned(),
            expected: ExpectedValue::AgentTarget,
        }
    );
}

#[test]
fn layout_reads_reject_mutation_and_detached_options() {
    for verb in ["layout", "neighbor", "edges"] {
        for flag in [
            vec!["--detach"],
            vec!["--request-id", "00000000000000000000000000000001"],
        ] {
            let mut args = vec!["panel", verb, "00000000000000000000000000000006"];
            if verb == "neighbor" {
                args.extend(["--direction", "left"]);
            }
            args.extend(flag);
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
    assert!(
        parse(&[
            "layout",
            "set-split-ratio",
            "--tab",
            "00000000000000000000000000000005",
            "--path",
            "root",
            "--ratio",
            "0.6",
            "--detach"
        ])
        .is_err()
    );
}

#[test]
fn stable_compact_selectors_are_exact_nonzero_identities() {
    for kind in ["project", "workspace", "tab", "panel", "terminal"] {
        for hex in ["1", "91", "ffffffffffffffffffffffffffffffff"] {
            let handle = format!("@{kind}:0x{hex}");
            let target = Target::Selector(handle.clone());
            assert_eq!(target.text(), handle);
            assert_eq!(target.selector_identity(), Some(format!("{hex:0>32}")));
            assert!(parse(&[kind, "get", &handle]).is_ok());
        }
        for hex in ["", "0", "01", "A", "g", "fffffffffffffffffffffffffffffffff"] {
            assert!(parse(&[kind, "get", &format!("@{kind}:0x{hex}")]).is_err());
        }
    }
    assert!(parse(&["panel", "get", "@terminal:0x1"]).is_err());
}
