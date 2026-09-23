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
        usage: None,
    };
    let output = super::parse_failure(usage_failure, Locale::EnUs);
    assert_eq!(output.status, 2);
    assert_eq!(envelope(&output.stdout)["failure"]["kind"], "invalid-usage");

    let silent = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Silent,
        usage: None,
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
        Family::Agent,
        Family::AgentIntegration,
        Family::Api,
        Family::Completion,
        Family::Config,
        Family::LaunchProfile,
        Family::Panel,
        Family::Project,
        Family::Site,
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
        CommandPath::AgentDoctor,
        CommandPath::AgentExplain,
        CommandPath::AgentGet,
        CommandPath::AgentRead,
        CommandPath::AgentPrompt,
        CommandPath::AgentStart,
        CommandPath::AgentSendKeys,
        CommandPath::AgentList,
        CommandPath::AgentNames,
        CommandPath::AgentRename,
        CommandPath::AgentResolve,
        CommandPath::AgentIntegrationDisable,
        CommandPath::AgentIntegrationEnable,
        CommandPath::AgentIntegrationExplain,
        CommandPath::AgentIntegrationList,
        CommandPath::AgentWait,
        CommandPath::ApiSchema,
        CommandPath::ApiSnapshot,
        CommandPath::ConfigCheck,
        CommandPath::LaunchProfileCreate,
        CommandPath::LaunchProfileDelete,
        CommandPath::LaunchProfileGet,
        CommandPath::LaunchProfileList,
        CommandPath::PanelClose,
        CommandPath::PanelCurrent,
        CommandPath::PanelProcessInfo,
        CommandPath::PanelGet,
        CommandPath::PanelList,
        CommandPath::PanelMove,
        CommandPath::PanelSplit,
        CommandPath::ProjectCreate,
        CommandPath::ProjectGet,
        CommandPath::ProjectList,
        CommandPath::ProjectRename,
        CommandPath::SiteDiagnose,
        CommandPath::SiteStart,
        CommandPath::SiteStop,
        CommandPath::SiteStopStatus,
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
        CommandPath::TerminalSend,
        CommandPath::TerminalWaitOutput,
        CommandPath::Tui,
        CommandPath::WorkspaceAcceptDirectory,
        CommandPath::WorkspaceClose,
        CommandPath::WorkspaceCloseResume,
        CommandPath::WorkspaceCloseStatus,
        CommandPath::WorkspaceCreate,
        CommandPath::WorkspaceGet,
        CommandPath::WorkspaceList,
        CommandPath::WorkspaceRename,
        CommandPath::WorktreeList,
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
        "agent",
        "api",
        "completion",
        "config",
        "help",
        "launch-profile",
        "panel",
        "project",
        "site",
        "status",
        "tab",
        "terminal",
        "tui",
        "workspace",
        "worktree",
    ];
    let verbs = [
        "accept-directory",
        "check",
        "close",
        "close-resume",
        "close-status",
        "create",
        "current",
        "delete",
        "diagnose",
        "doctor",
        "explain",
        "get",
        "integration",
        "list",
        "move",
        "names",
        "process-info",
        "read",
        "prompt",
        "rename",
        "resolve",
        "schema",
        "send",
        "send-keys",
        "snapshot",
        "split",
        "start",
        "stop",
        "stop-status",
        "wait",
        "wait-output",
    ];
    // `agent integration` nests one level deeper — the pin table must cover
    // its commands through both legal surfaces as well.
    let integration_verbs = ["disable", "enable", "explain", "list"];
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
    for verb in integration_verbs {
        collect(&["agent", "integration", verb, "--help"]);
        collect(&["help", "agent", "integration", verb]);
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
        // Heuristic guardrail, not a protocol contract: the real emission
        // bound is `output::LIMIT` (8 MiB). 128 KiB keeps fixed bodies ~64x
        // under it while leaving honest headroom for grammar growth — the
        // completion walker emits per-command forbidden-bit arms, so each new
        // family adds a proportional slice (the launch-profile family moved
        // the largest script from ~62 KiB to ~70 KiB).
        assert!(
            body.len() < 128 * 1024,
            "fixed output {} bytes must stay far under the bound",
            body.len()
        );
    }
    // The terminal family help must name the accepted optional history
    // spelling and explain it separately — the bound alone would let the
    // new grammar drift out of family help silently.
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let body = super::text::help(HelpTopic::Family(Family::Terminal), locale);
        assert!(
            body.contains("[--history --rows <n>]"),
            "{locale:?} terminal family help lost the history spelling"
        );
        assert!(
            body.contains(match locale {
                Locale::EnUs => "retained history",
                Locale::ZhCn => "保留历史",
            }),
            "{locale:?} terminal family help lost the history explanation"
        );
    }
    // The geometry-bearing command help must state the all-or-none group
    // and the exact default so a partial-set refusal reads as intended.
    for command in [CommandPath::TabCreate, CommandPath::PanelSplit] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let body = super::text::help(HelpTopic::Command(command), locale);
            assert!(
                body.contains("[--cols <n> --rows <n> --cell-width <px> --cell-height <px>]"),
                "{locale:?} {command:?} help lost the optional geometry group"
            );
            for needle in match locale {
                Locale::EnUs => ["all-or-none", "80x24"],
                Locale::ZhCn => ["全有或全无", "80x24"],
            } {
                assert!(
                    body.contains(needle),
                    "{locale:?} {command:?} help lost '{needle}'"
                );
            }
        }
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

/// The usage reason is a stderr-only diagnostic: the JSON failure document is
/// byte-identical whether or not a reason rides along.
#[test]
fn usage_reason_never_reaches_the_machine_envelope() {
    let reasoned = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Json,
        usage: Some(crate::invocation::UsageReason::UnknownOption {
            option: "--bogus".to_owned(),
        }),
    };
    let plain = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Json,
        usage: None,
    };
    let with_reason = super::parse_failure(reasoned, Locale::EnUs);
    let without = super::parse_failure(plain, Locale::EnUs);
    assert_eq!(with_reason.status, without.status);
    assert_eq!(with_reason.stdout, without.stdout);
    assert_eq!(
        envelope(&with_reason.stdout)["failure"]["kind"],
        "invalid-usage"
    );
    assert!(with_reason.stdout.iter().all(|byte| byte.is_ascii()));
    assert_eq!(
        with_reason.stderr,
        "Unknown option \"--bogus\". Run 'orchbee --help'.\n"
    );
    assert_ne!(with_reason.stderr, without.stderr);
}

/// `terminal wait-output` reserves two failure kinds: timeout exits 7 and a
/// signal-cancelled wait exits 130, each serializing its own `kind` string.
/// The whole matrix stays pinned so no existing kind can drift.
#[test]
fn wait_failure_kinds_map_to_their_exit_codes() {
    use super::FailureKind;
    let cases: &[(FailureKind, &str, u8)] = &[
        (FailureKind::Ambiguous, "ambiguous", 3),
        (FailureKind::Cancelled, "cancelled", 130),
        (FailureKind::CheckFailed, "check-failed", 1),
        (FailureKind::Denied, "denied", 3),
        (FailureKind::Incompatible, "incompatible", 6),
        (FailureKind::InternalFailure, "internal-failure", 1),
        (FailureKind::InvalidIdentifier, "invalid-identifier", 2),
        (FailureKind::InvalidUsage, "invalid-usage", 2),
        (FailureKind::NotFound, "not-found", 3),
        (FailureKind::OutcomeUnknown, "outcome-unknown", 8),
        (FailureKind::OutputTooLarge, "output-too-large", 1),
        (FailureKind::ProtocolInvalid, "protocol-invalid", 5),
        (FailureKind::Rejected, "rejected", 3),
        (FailureKind::ServiceUnavailable, "service-unavailable", 4),
        (FailureKind::SessionCapacity, "session-capacity", 4),
        (FailureKind::Timeout, "timeout", 7),
        (FailureKind::TransportLoss, "transport-loss", 5),
    ];
    for (kind, name, code) in cases {
        assert_eq!(kind.as_str(), *name, "{kind:?}");
        assert_eq!(kind.exit_code(), *code, "{kind:?}");
    }
    // Both new kinds emit their typed JSON envelope and localized stderr.
    for (kind, name, code) in [
        (FailureKind::Timeout, "timeout", 7_u8),
        (FailureKind::Cancelled, "cancelled", 130_u8),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output =
                super::failure_output(super::Failure::new(kind), Some(OutputFormat::Json), locale);
            assert_eq!(output.status, code, "{kind:?} {locale:?}");
            assert_eq!(envelope(&output.stdout)["failure"]["kind"], name);
            assert!(!output.stderr.is_empty(), "localized diagnostic");
        }
    }
    // The wait result schema is registered under its published id.
    let document = super::schema::answer(
        Some("orchbee.terminal.wait-output"),
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(document.status, 0);
    let body = envelope(&document.stdout);
    assert_eq!(
        body["result"]["schema"]["schemaId"],
        "orchbee.terminal.wait-output"
    );
    assert_eq!(
        body["result"]["schema"]["document"]["$id"],
        "orchbee.terminal.wait-output"
    );
}

/// A `terminal send` source-stage timeout or cancellation diagnoses the
/// input read — nothing was awaited and nothing was sent. The wait stage
/// keeps its own wording. Both locales and both routing paths are pinned.
#[cfg(target_os = "linux")]
#[test]
fn source_stage_failures_diagnose_the_input_read() {
    use super::{Failure, FailureKind, source_failure_output};
    let input_details = || {
        serde_json::Map::from_iter([
            ("stage".to_owned(), json!("input")),
            ("inputSource".to_owned(), json!("stdin")),
        ])
    };
    for kind in [FailureKind::Timeout, FailureKind::Cancelled] {
        for (locale, expected, forbidden) in [
            (
                Locale::EnUs,
                match kind {
                    FailureKind::Timeout => "could not be read within the time limit",
                    _ => "Reading the declared input was cancelled",
                },
                match kind {
                    FailureKind::Timeout => "wait deadline elapsed",
                    _ => "wait was cancelled",
                },
            ),
            (
                Locale::ZhCn,
                match kind {
                    FailureKind::Timeout => "未能在时限内读取",
                    _ => "读取已被取消",
                },
                match kind {
                    FailureKind::Timeout => "未出现目标文本",
                    _ => "未报告任何匹配",
                },
            ),
        ] {
            let failure = Failure::new(kind).details(input_details());
            let stderr = locale.cli_failure(&failure);
            assert!(
                stderr.contains(expected),
                "{kind:?} {locale:?} must diagnose the input read, got {stderr:?}"
            );
            assert!(
                !stderr.contains(forbidden),
                "{kind:?} {locale:?} must not claim a wait ran, got {stderr:?}"
            );
        }
    }
    // Both routing paths carry the same source-stage wording.
    for (format, expect_stdout) in [(OutputFormat::Text, false), (OutputFormat::Json, true)] {
        let failure = Failure::new(FailureKind::Timeout).details(input_details());
        let output = source_failure_output(failure, 7, format, Locale::EnUs);
        assert_eq!(output.stdout.is_empty(), !expect_stdout);
        assert!(
            output.stderr.contains("could not be read"),
            "stderr diagnoses the input read on the {format:?} path"
        );
        if expect_stdout {
            let body = envelope(&output.stdout);
            assert_eq!(body["schema"], "orchbee.cli.failure.source");
            assert_eq!(body["failure"]["kind"], "timeout");
        }
    }
}

/// The wait stage and the detail-free fallback keep the established wait
/// wording — the source-stage carve-out never leaks into other producers.
#[test]
fn wait_stage_failures_keep_the_wait_wording() {
    use super::{Failure, FailureKind};
    for kind in [FailureKind::Timeout, FailureKind::Cancelled] {
        for details in [
            Some(serde_json::Map::from_iter([(
                "stage".to_owned(),
                json!("wait"),
            )])),
            None,
        ] {
            let mut failure = Failure::new(kind);
            if let Some(details) = details {
                failure = failure.details(details);
            }
            let expected = match (kind, &failure.details) {
                (FailureKind::Timeout, _) => "wait deadline elapsed",
                _ => "wait was cancelled",
            };
            assert!(
                Locale::EnUs.cli_failure(&failure).contains(expected),
                "{kind:?} without stage:\"input\" keeps the wait wording"
            );
        }
    }
}

/// A captured signal on an Agent read answers `orchbee.cli.failure.agent`
/// with the cancelled kind at 130 and the noncloneable finalization riding
/// the common emit path — a cancelled read carries no correlated source.
#[cfg(target_os = "linux")]
#[test]
fn agent_cancellation_emits_agent_envelope_with_finalization() {
    use super::{Failure, FailureKind, SourceFinalization, query};
    let command = QueryCommand::AgentList {
        include_ended: false,
        limit: 50,
        cursor: None,
    };
    let mut details = serde_json::Map::new();
    details.insert("stage".to_owned(), json!("agent-read"));
    details.insert("operation".to_owned(), json!("list"));
    let output = super::agent_output(
        Err(query::AgentQueryError::Cancelled(Box::new(
            query::AgentCancelled {
                failure: Failure::new(FailureKind::Cancelled).details(details),
                finalization: SourceFinalization::failing(),
            },
        ))),
        &command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 130);
    assert!(
        output.finalization.is_some(),
        "the owed finalization reaches the common emit"
    );
    let body = envelope(&output.stdout);
    assert_eq!(body["schema"], "orchbee.cli.failure.agent");
    assert_eq!(body["failure"]["kind"], "cancelled");
    assert_eq!(body["failure"]["details"]["stage"], "agent-read");
    assert_eq!(body["failure"]["details"]["operation"], "list");
    assert!(
        body.get("source").is_none() || body["source"].is_null(),
        "a cancelled read publishes no correlated source"
    );
}

/// A failed termination-signal registration refuses an integration
/// invocation before any provider context, inspection, or effect — the
/// armed seam makes `install` fail once, and the refusal carries the
/// existing `internal-failure` kind at exit 1. A second install succeeds,
/// proving the refusal came from the injected fault, not ambient state.
#[cfg(unix)]
#[test]
fn integration_signal_install_failure_refuses_before_effects() {
    use super::{FailureKind, integration};
    use crate::agent_integrations::IntegrationId;
    use crate::invocation::AgentIntegrationCommand;

    crate::termination_signal::arm_install_fault();
    let command = AgentIntegrationCommand::Enable {
        provider: IntegrationId::ClaudeCode,
        adapter_source: None,
    };
    let failure = match integration::run_local(&command) {
        Err(failure) => failure,
        Ok(_) => panic!("armed install fault must refuse the invocation"),
    };
    assert_eq!(failure.kind, FailureKind::InternalFailure);
    assert_eq!(failure.kind.exit_code(), 1);
    assert!(
        crate::termination_signal::TerminationSignal::install().is_ok(),
        "the armed fault is one-shot; install itself stays healthy"
    );
}

/// A cancelled integration mutation projects `outcome:"cancelled"` — the
/// schema must accept the real emitted document and the exit stays 130.
/// The report is an entry-gate cancel: no attempted step, no observation.
#[cfg(unix)]
#[test]
fn integration_cancelled_mutation_document_validates_against_schema() {
    use super::integration::Prepared;
    use crate::agent_integrations::model::Provenance;
    use crate::agent_integrations::{Direction, IntegrationId, MutationReport, Verdict};

    let report = MutationReport {
        integration_id: IntegrationId::ClaudeCode,
        direction: Direction::Enable,
        attempted: vec![],
        residuals: vec![],
        observed: None,
        provenance: Provenance::PreEffect,
        effect_begun: false,
        verdict: Verdict::Cancelled,
    };
    let output = Prepared::Mutation { report }.render(OutputFormat::Json, Locale::EnUs);
    assert_eq!(output.status, 130, "cancelled keeps exit 130");
    let document: Value = serde_json::from_slice(&output.stdout).expect("mutation JSON");
    assert_eq!(document["result"]["outcome"], "cancelled");

    let schema: Value = serde_json::from_str(include_str!(
        "schemas/orchbee.agent.integration.mutation.json"
    ))
    .expect("mutation schema parses");
    let validator = jsonschema::draft202012::new(&schema).expect("schema compiles");
    assert!(
        validator.is_valid(&document),
        "real cancelled projection must validate: {document}"
    );

    // The vocabulary stays closed — an unknown outcome word is rejected.
    let mut bad = document.clone();
    bad["result"]["outcome"] = json!("aborted");
    assert!(
        !validator.is_valid(&bad),
        "a non-vocabulary outcome must fail the schema"
    );
}
