use std::ffi::OsString;

use super::{
    CommandPath, CompletionShell, FailureEnvelope, Family, HelpTopic, OutputFormat, ParseFailure,
    ParseFailureKind, QueryCommand, Request, Resource, Target, parse_request,
};

fn parse(arguments: &[&str]) -> Result<Request, ParseFailure> {
    parse_request(arguments.iter().map(OsString::from))
}

fn usage(envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope,
    }
}

fn identifier(resource: Resource, target: &str, envelope: FailureEnvelope) -> ParseFailure {
    ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier {
            resource,
            target: target.to_owned(),
        },
        envelope,
    }
}

fn query(command: QueryCommand, format: OutputFormat) -> Request {
    Request::Query { command, format }
}

const ID: &str = "0123456789abcdef0123456789abcdef";
const ZERO: &str = "00000000000000000000000000000000";

#[test]
fn accepts_legacy_root_forms() {
    assert_eq!(parse(&[]), Ok(Request::Tui));
    assert_eq!(parse(&["tui"]), Ok(Request::Tui));
    assert_eq!(parse(&["--"]), Ok(Request::Tui));
    assert_eq!(parse(&["--", "tui"]), Ok(Request::Tui));
    assert_eq!(parse(&["--help"]), Ok(Request::RootHelp));
    assert_eq!(parse(&["help"]), Ok(Request::RootHelp));
    assert_eq!(parse(&["--version"]), Ok(Request::Version));
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
            QueryCommand::WorkspaceList { project: None },
        ),
        (
            &["workspace", "get", "main"],
            QueryCommand::WorkspaceGet(Target::Label("main".into())),
        ),
        (
            &["tab", "list"],
            QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
        ),
        (
            &["tab", "get", ID],
            QueryCommand::TabGet(Target::Id(ID.into())),
        ),
        (&["panel", "list"], QueryCommand::PanelList { tab: None }),
        (
            &["panel", "get", ID],
            QueryCommand::PanelGet(Target::Id(ID.into())),
        ),
        (&["terminal", "list"], QueryCommand::TerminalList),
        (
            &["terminal", "get", ID],
            QueryCommand::TerminalGet(Target::Id(ID.into())),
        ),
        (
            &["terminal", "read", ID],
            QueryCommand::TerminalRead(Target::Id(ID.into())),
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
        &["-h"],
        &["--unknown"],
        &["status", "--unknown"],
        &["--help", "status"],
        &["status", "--help", "extra"],
        &["help", "--help"],
        &["help", "help"],
        &["help", "version"],
        &["help", "--version"],
        &["status", "--version"],
        &["--version", "status"],
        &["--version", "--help"],
        &["tui", "--format", "text"],
        &["completion"],
        &["completion", "powershell"],
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
fn filters_have_declared_scope_single_use_and_values() {
    // Correct scopes.
    assert_eq!(
        parse(&["workspace", "list", "--project", "web"]),
        Ok(query(
            QueryCommand::WorkspaceList {
                project: Some(Target::Label("web".into())),
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["tab", "list", "--workspace", ID]),
        Ok(query(
            QueryCommand::TabList {
                workspace: Some(Target::Id(ID.into())),
                standalone: false,
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["tab", "list", "--standalone"]),
        Ok(query(
            QueryCommand::TabList {
                workspace: None,
                standalone: true,
            },
            OutputFormat::Text
        ))
    );
    assert_eq!(
        parse(&["panel", "list", "--tab", ID]),
        Ok(query(
            QueryCommand::PanelList {
                tab: Some(Target::Id(ID.into())),
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
        // `tab list` declares only --workspace/--standalone.
        &["tab", "list", "--project", "x"],
        &["tab", "list", "--tab", "x"],
        // `panel list` declares only --tab.
        &["panel", "list", "--project", "x"],
        &["panel", "list", "--workspace", "x"],
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
        // Filters never use the equals spelling.
        &["workspace", "list", "--project=x"],
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
fn suffix_help_requires_an_exact_complete_command_path() {
    let topics: &[(&[&str], HelpTopic)] = &[
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
        (
            &["completion", "--help"],
            HelpTopic::Command(CommandPath::Completion),
        ),
    ];
    for (arguments, topic) in topics {
        assert_eq!(parse(arguments), Ok(Request::Help(*topic)), "{arguments:?}");
    }

    let cases: &[&[&str]] = &[
        // A family name is not a complete command path for the suffix form.
        &["project", "--help"],
        &["api", "--help"],
        &["config", "--help"],
        // Business arguments with suffix help are usage.
        &["project", "get", "x", "--help"],
        &["api", "schema", "orchbee.status", "--help"],
        &["completion", "bash", "--help"],
        &["workspace", "list", "--project", "x", "--help"],
        &["tab", "list", "--standalone", "--help"],
        // Format on a help request is a syntax failure.
        &["status", "--help", "--format", "text"],
        &["--format", "text", "status", "--help"],
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    assert_eq!(
        parse(&["status", "--help", "--format", "json"]),
        Err(usage(FailureEnvelope::Json))
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
        })
    );
}
