//! Output-boundary and failure-format witnesses: oversized projections are
//! injected at the result layer so every format-sensitive path is covered
//! without an invalid wire-sized snapshot.

use serde_json::{Value, json};

use super::results::{ContextRef, Projection, TabDoc, TabRow, TerminalRow};
use super::{OutputFormat, output};
use crate::invocation::{ParseFailure, ParseFailureKind, QueryCommand, Target};
use crate::localization::Locale;

const ID: &str = "0123456789abcdef0123456789abcdef";

fn oversized_terminals() -> Projection {
    // ~42k rows × ~200 rendered bytes exceeds the 8 MiB preparation bound in
    // every format without any Site or wire involvement.
    let row = || TerminalRow {
        terminal_id: ID.to_owned(),
        worker_id: "6f7263686265652d6c6f63616c2d7631".to_owned(),
        presentation_id: ID.to_owned(),
        panel_id: ID.to_owned(),
        tab_id: ID.to_owned(),
        terminal_context_id: ID.to_owned(),
    };
    Projection::TerminalList((0..42_000).map(|_| row()).collect())
}

fn oversized_snapshot() -> Projection {
    // A syntactically valid machine payload beyond the preparation bound.
    Projection::ApiSnapshot(json!({"blob": "x".repeat(output::LIMIT)}))
}

fn envelope(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("failure envelope is JSON")
}

#[test]
fn oversized_text_result_stays_silent_and_reports_too_large() {
    let output = super::success_output(
        oversized_terminals(),
        json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"}),
        &QueryCommand::TerminalList,
        OutputFormat::Text,
        Locale::EnUs,
    );
    assert_eq!(output.status, 1);
    assert!(
        output.stdout.is_empty(),
        "preparation overflow writes no result prefix"
    );
    assert!(!output.stderr.is_empty(), "localized diagnostic on stderr");
}

#[test]
fn oversized_json_result_emits_the_selected_envelope() {
    let output = super::success_output(
        oversized_terminals(),
        json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"}),
        &QueryCommand::TerminalList,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 1);
    let failure = envelope(&output.stdout);
    assert_eq!(failure["schema"], "orchbee.cli.failure");
    assert_eq!(failure["failure"]["kind"], "output-too-large");
    assert_eq!(
        failure["failure"]["details"]["limitBytes"],
        output::LIMIT as u64
    );
    assert_eq!(failure["source"]["kind"], "workbench-snapshot");
    assert!(!output.stderr.is_empty());
}

#[test]
fn api_snapshot_overflow_obeys_the_caller_format() {
    // `api snapshot` success is always a machine document, but its
    // preparation overflow answers in the caller's selected format.
    let source = json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"});
    let text = super::success_output(
        oversized_snapshot(),
        source.clone(),
        &QueryCommand::ApiSnapshot,
        OutputFormat::Text,
        Locale::EnUs,
    );
    assert_eq!(text.status, 1);
    assert!(
        text.stdout.is_empty(),
        "text caller gets no failure document"
    );
    assert!(!text.stderr.is_empty());

    let json_output = super::success_output(
        oversized_snapshot(),
        source,
        &QueryCommand::ApiSnapshot,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(json_output.status, 1);
    assert_eq!(
        envelope(&json_output.stdout)["failure"]["kind"],
        "output-too-large"
    );
}

#[test]
fn api_schema_failures_follow_the_caller_format() {
    // Text/default: the unknown schema id keeps stdout empty.
    let text = super::schema::answer(Some("orchbee.nope"), OutputFormat::Text, Locale::EnUs);
    assert_eq!(text.status, 3);
    assert!(text.stdout.is_empty());
    assert!(!text.stderr.is_empty());

    // Explicit json: the same failure is a typed envelope with embedded source.
    let json_output = super::schema::answer(Some("orchbee.nope"), OutputFormat::Json, Locale::EnUs);
    assert_eq!(json_output.status, 3);
    let failure = envelope(&json_output.stdout);
    assert_eq!(failure["failure"]["kind"], "not-found");
    assert_eq!(failure["failure"]["details"]["resource"], "schema");
    assert_eq!(failure["failure"]["details"]["target"], "orchbee.nope");
    assert_eq!(failure["source"]["kind"], "embedded");

    // Success is a machine document in both formats.
    for format in [OutputFormat::Text, OutputFormat::Json] {
        let output = super::schema::answer(Some("orchbee.status"), format, Locale::EnUs);
        assert_eq!(output.status, 0);
        let document = envelope(&output.stdout);
        assert_eq!(document["schema"], "orchbee.api.schema");
        assert_eq!(document["source"]["kind"], "embedded");
        assert_eq!(document["result"]["schema"]["schemaId"], "orchbee.status");
    }
}

#[test]
fn parse_failure_envelopes_follow_the_recorded_selection() {
    let usage_failure = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Json,
    };
    let output = super::parse_failure(usage_failure, Locale::EnUs);
    assert_eq!(output.status, 2);
    assert_eq!(envelope(&output.stdout)["failure"]["kind"], "invalid-usage");

    let silent = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Silent,
    };
    let output = super::parse_failure(silent, Locale::EnUs);
    assert_eq!(output.status, 2);
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn renderer_path_lands_exactly_on_and_just_over_the_bound() {
    // The real `tab get` renderer streams into the bounded sink: an exact
    // 8 MiB document succeeds and one byte over is a preparation failure
    // with no result prefix on stdout.
    let doc = |blob: usize| {
        Projection::TabGet(TabDoc {
            row: TabRow {
                tab_id: ID.to_owned(),
                label: "t".to_owned(),
                pinned: false,
                group_id: None,
                context: ContextRef {
                    standalone: true,
                    terminal_context_id: ID.to_owned(),
                    workspace_id: None,
                },
            },
            root: json!({"blob": "x".repeat(blob)}),
        })
    };
    let command = QueryCommand::TabGet(Target::Id(ID.to_owned()));
    let baseline =
        output::render_text(|out| super::present::render(out, &doc(0), &command, Locale::EnUs))
            .expect("small render");
    // Growing `blob` grows the rendered body by exactly that many bytes.
    let fill = output::LIMIT - baseline.len();
    let rendered =
        output::render_text(|out| super::present::render(out, &doc(fill), &command, Locale::EnUs))
            .expect("render exactly at the bound");
    assert_eq!(rendered.len(), output::LIMIT);

    let output = super::success_output(
        doc(fill + 1),
        json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"}),
        &command,
        OutputFormat::Text,
        Locale::EnUs,
    );
    assert_eq!(output.status, 1);
    assert!(output.stdout.is_empty(), "no result prefix over the bound");
    assert!(!output.stderr.is_empty());
}

#[test]
fn fixed_outputs_are_far_below_the_bound() {
    use crate::invocation::{
        CommandPath, CompletionShell, Family, HelpTopic, Request, parse_request,
    };
    use std::ffi::OsString;
    let mut bodies = vec![
        super::completion::render(CompletionShell::Bash),
        super::completion::render(CompletionShell::Zsh),
        super::completion::render(CompletionShell::Fish),
    ];
    for family in [
        Family::Api,
        Family::Completion,
        Family::Config,
        Family::Panel,
        Family::Project,
        Family::Tab,
        Family::Terminal,
        Family::Workspace,
    ] {
        bodies.push(super::text::help(HelpTopic::Family(family), Locale::EnUs).to_owned());
        bodies.push(super::text::help(HelpTopic::Family(family), Locale::ZhCn).to_owned());
    }
    // Every `CommandPath` variant, in both locales — the bound must cover the
    // mutation helps as well as the queries.
    let commands = [
        CommandPath::ApiSchema,
        CommandPath::ApiSnapshot,
        CommandPath::ConfigCheck,
        CommandPath::PanelClose,
        CommandPath::PanelGet,
        CommandPath::PanelList,
        CommandPath::PanelMove,
        CommandPath::PanelSplit,
        CommandPath::ProjectGet,
        CommandPath::ProjectList,
        CommandPath::Status,
        CommandPath::TabClose,
        CommandPath::TabCreate,
        CommandPath::TabGet,
        CommandPath::TabList,
        CommandPath::TabMove,
        CommandPath::TabRename,
        CommandPath::TerminalGet,
        CommandPath::TerminalList,
        CommandPath::TerminalRead,
        CommandPath::Tui,
        CommandPath::WorkspaceCreate,
        CommandPath::WorkspaceGet,
        CommandPath::WorkspaceList,
        CommandPath::WorkspaceRename,
    ];
    for command in &commands {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            bodies.push(super::text::help(HelpTopic::Command(*command), locale).to_owned());
        }
    }
    // Coverage contract — manual vocabulary, verified both directions:
    // `roots`/`verbs` below are the *current* routing words maintained by
    // hand. The walk proves the pinned `commands` table and the grammar agree
    // on every command reachable through these surfaces; it cannot discover a
    // root or verb that the grammar later admits but nobody lists here. When
    // the grammar grows a new root or verb, extend these lists in the same
    // change — the two assertions then keep the pin table honest for
    // everything the walk does enumerate. Both legal surfaces are walked:
    // trailing `--help` and the `help` prefix.
    let roots = [
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
    let verbs = [
        "check", "close", "create", "get", "list", "move", "read", "rename", "schema", "snapshot",
        "split",
    ];
    let mut topics: Vec<CommandPath> = Vec::new();
    let mut collect = |arguments: &[&str]| {
        if let Ok(Request::Help(HelpTopic::Command(command))) =
            parse_request(arguments.iter().map(|word| OsString::from(*word)))
            && !topics.contains(&command)
        {
            topics.push(command);
        }
    };
    for root in roots {
        collect(&[root, "--help"]);
        collect(&["help", root]);
        for verb in verbs {
            collect(&[root, verb, "--help"]);
            collect(&["help", root, verb]);
        }
    }
    for command in &topics {
        assert!(
            commands.contains(command),
            "command path {command:?} resolved by the grammar is not pinned in the bound test"
        );
    }
    assert_eq!(
        topics.len(),
        commands.len(),
        "the pinned table must cover every command path the grammar admits"
    );
    // Negative control on the mechanism over the current vocabulary: the walk
    // must actually resolve known mutation topics, and a pin table missing
    // one would trip both assertions above — proving the check is not vacuous.
    // This says nothing about roots/verbs admitted later but never listed.
    assert!(
        topics.contains(&CommandPath::PanelSplit) && topics.contains(&CommandPath::WorkspaceCreate),
        "the grammar walk must resolve real mutation help topics"
    );
    for body in &bodies {
        assert!(
            body.len() < 64 * 1024,
            "fixed output {} bytes must stay far under the bound",
            body.len()
        );
    }
}

#[test]
fn failure_documents_stay_well_below_the_bound() {
    // A maximally populated failure envelope must never overflow preparation.
    let failure =
        super::Failure::new(super::FailureKind::Ambiguous).details(serde_json::Map::from_iter([
            ("resource".to_owned(), json!("tab")),
            ("target".to_owned(), json!("x".repeat(512))),
            ("candidates".to_owned(), json!(vec!["y".repeat(32); 8])),
            ("totalCount".to_owned(), json!(u64::MAX)),
        ]));
    let output = super::failure_output(failure, Some(OutputFormat::Json), Locale::ZhCn);
    assert!(!output.stdout.is_empty(), "json failure envelope emitted");
    assert!(output.stdout.len() < 64 * 1024);
}
