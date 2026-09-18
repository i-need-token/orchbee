use std::ffi::OsString;

use orchbee_workbench_protocol::{
    WorkbenchLabel, WorkbenchPanelSplitDirection, WorkbenchTabMoveDirection,
    WorkbenchTerminalGeometry,
};

use super::{
    CommandPath, CompletionShell, FailureEnvelope, Family, HelpTopic, MutationCommand,
    OutputFormat, ParseFailure, ParseFailureKind, QueryCommand, Request, Resource, TabContext,
    Target, parse_request,
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

// ---- mutation commands ------------------------------------------------------

fn mutation(command: MutationCommand, format: OutputFormat) -> Request {
    Request::Mutation { command, format }
}

fn label(value: &str) -> WorkbenchLabel {
    WorkbenchLabel::parse(value).expect("test label is protocol-valid")
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
            &["workspace", "create", "--project", "web", "--label", "api"],
            MutationCommand::WorkspaceCreate {
                project: Target::Label("web".into()),
                label: label("api"),
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
            &["tab", "rename", ID, "--label", "renamed"],
            MutationCommand::TabRename {
                target: Target::Id(ID.into()),
                label: label("renamed"),
            },
        ),
        (
            &["tab", "move", "logs", "--direction", "first"],
            MutationCommand::TabMove {
                target: Target::Label("logs".into()),
                direction: WorkbenchTabMoveDirection::First,
            },
        ),
        (
            &["tab", "close", ID, "--confirm"],
            MutationCommand::TabClose {
                target: Target::Id(ID.into()),
            },
        ),
        (
            &["panel", "close", ID, "--confirm"],
            MutationCommand::PanelClose {
                target: Target::Id(ID.into()),
            },
        ),
        (
            &[
                "panel",
                "move",
                ID,
                "--target-panel",
                ID2,
                "--direction",
                "left",
            ],
            MutationCommand::PanelMove {
                target: Target::Id(ID.into()),
                into: Target::Id(ID2.into()),
                direction: WorkbenchPanelSplitDirection::Left,
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
    let mut arguments = vec!["tab", "create", "--standalone", "--label", "t"];
    arguments.extend(GEOMETRY_ARGS);
    assert_eq!(
        parse(&arguments),
        Ok(mutation(
            MutationCommand::TabCreate {
                context: TabContext::Standalone,
                label: label("t"),
                geometry: geometry(),
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
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                geometry: geometry(),
            },
            OutputFormat::Text
        ))
    );

    // Any missing dimension rejects the whole command.
    for missing in ["--cols", "--rows", "--cell-width", "--cell-height"] {
        let mut arguments = vec!["tab", "create", "--standalone", "--label", "t"];
        for pair in GEOMETRY_ARGS.chunks(2) {
            if pair[0] != missing {
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

#[test]
fn tab_create_admits_exactly_one_context() {
    let mut workspace = tab_create_args("main");
    workspace.extend(["--label", "t"]);
    workspace.extend(GEOMETRY_ARGS);
    assert_eq!(
        parse(&workspace),
        Ok(mutation(
            MutationCommand::TabCreate {
                context: TabContext::Workspace(Target::Label("main".into())),
                label: label("t"),
                geometry: geometry(),
            },
            OutputFormat::Text
        ))
    );

    // Neither context nor both contexts are legal.
    let mut neither = vec!["tab", "create", "--label", "t"];
    neither.extend(GEOMETRY_ARGS);
    assert_eq!(parse(&neither), Err(usage(FailureEnvelope::Text)));

    let mut both = tab_create_args("main");
    both.extend(["--standalone", "--label", "t"]);
    both.extend(GEOMETRY_ARGS);
    assert_eq!(parse(&both), Err(usage(FailureEnvelope::Text)));

    let mut swapped = vec!["tab", "create", "--standalone", "--workspace", "main"];
    swapped.extend(["--label", "t"]);
    swapped.extend(GEOMETRY_ARGS);
    assert_eq!(parse(&swapped), Err(usage(FailureEnvelope::Text)));
}

#[test]
fn close_commands_require_explicit_confirm() {
    for arguments in [
        &["tab", "close", ID][..],
        &["panel", "close", ID][..],
        // --confirm is scoped to the close commands only.
        &["tab", "rename", ID, "--label", "x", "--confirm"][..],
        &["workspace", "rename", "w", "--label", "x", "--confirm"][..],
        &["tab", "move", ID, "--direction", "next", "--confirm"][..],
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
        let arguments = ["tab", "move", ID, "--direction", word];
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
        ("above", WorkbenchPanelSplitDirection::Above),
        ("below", WorkbenchPanelSplitDirection::Below),
        ("left", WorkbenchPanelSplitDirection::Left),
        ("right", WorkbenchPanelSplitDirection::Right),
    ] {
        let arguments = [
            "panel",
            "move",
            ID,
            "--target-panel",
            ID2,
            "--direction",
            word,
        ];
        assert_eq!(
            parse(&arguments),
            Ok(mutation(
                MutationCommand::PanelMove {
                    target: Target::Id(ID.into()),
                    into: Target::Id(ID2.into()),
                    direction,
                },
                OutputFormat::Text
            )),
            "{arguments:?}"
        );
    }
    // Cross-family direction words are usage, not a nearest match.
    for arguments in [
        &["tab", "move", ID, "--direction", "above"][..],
        &["tab", "move", ID, "--direction", "right"][..],
        &[
            "panel",
            "move",
            ID,
            "--target-panel",
            ID2,
            "--direction",
            "next",
        ][..],
        &[
            "panel",
            "move",
            ID,
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
        &["workspace", "create", "--label", "x"],
        &["workspace", "create", "--project", "p"],
        &["workspace", "rename", "w"],
        &["tab", "rename", "t"],
        &["tab", "move", "t"],
        &["panel", "split", ID, "--label", "p"],
        &["panel", "split", ID, "--direction", "right"],
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
                target: Target::Id(ID.into()),
                direction: WorkbenchPanelSplitDirection::Right,
                label: label("p"),
                geometry: geometry(),
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
        &["tab", "rename", "t", "--label", "x", "--workspace", "w"],
        &["tab", "move", "t", "--direction", "next", "--label", "x"],
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
    ];
    for arguments in cases {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
            "{arguments:?}"
        );
    }
    // The legal form still parses.
    create.push("--format");
    create.push("json");
    assert_eq!(
        parse(&create),
        Ok(mutation(
            MutationCommand::WorkspaceCreate {
                project: Target::Label("p".into()),
                label: label("x"),
            },
            OutputFormat::Json
        ))
    );
}

#[test]
fn mutation_values_are_bounded() {
    // Labels: empty and overlong are usage; the protocol maximum passes.
    let overlong = "x".repeat(513);
    for arguments in [
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
    assert_eq!(
        parse(&["workspace", "rename", "w", "--label=x"]),
        Err(usage(FailureEnvelope::Text))
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
            "--standalone",
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
        "--standalone",
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
                context: TabContext::Standalone,
                label: label("t"),
                geometry: WorkbenchTerminalGeometry::new(512, 512, 64, 63)
                    .expect("geometry inside bounds"),
            },
            OutputFormat::Text
        ))
    );
    // A paired-zero cell-pixel geometry is the explicit unknown-pixel form
    // and parses like any other legal geometry.
    let arguments = [
        "tab",
        "create",
        "--standalone",
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
                context: TabContext::Standalone,
                label: label("t"),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 0, 0)
                    .expect("paired-zero geometry is protocol-valid"),
            },
            OutputFormat::Text
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
    // Business arguments keep suffix help a usage failure.
    for arguments in [
        &["workspace", "create", "--project", "p", "--help"][..],
        &["tab", "close", ID, "--confirm", "--help"][..],
        &["panel", "move", ID, "--help"][..],
    ] {
        assert_eq!(
            parse(arguments),
            Err(usage(FailureEnvelope::Text)),
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
            },
            OutputFormat::Json
        ))
    );
}
