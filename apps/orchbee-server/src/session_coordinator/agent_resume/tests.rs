use super::{CommandEvidence, RunAnchor, RunTableEntry};
use crate::agent_service::AgentResumeCandidate;
use orchbee_agent_observation_protocol::{
    AgentIdentityGrade, AgentIdentityUnknownReason, AgentIdentityUnsupportedReason,
    AgentProcessIdentity, AgentReportProvider, AgentRunToken,
};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};
use orchbee_workbench_protocol::TerminalPresentationId;

const CLAUDE: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const CODEX: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const STALE: &str = "a3bb189e-8bf9-3888-9912-ace4e6543002";
const RUN: u128 = 0x1111_2222_3333_4444_5555_6666_7777_8888;
const OTHER_RUN: u128 = 0x9999_aaaa_bbbb_cccc_dddd_eeee_ffff_0001;

fn candidate(
    provider: AgentResumeProvider,
    key: &str,
    token: Option<u128>,
) -> AgentResumeCandidate {
    AgentResumeCandidate {
        presentation: TerminalPresentationId::parse(&format!("{:032x}", 11)).unwrap(),
        provider,
        native_key: key.to_owned(),
        run_token: token.map(|t| AgentRunToken::parse(&format!("{t:032x}")).unwrap()),
    }
}

fn claude(token: Option<u128>) -> AgentResumeCandidate {
    candidate(AgentResumeProvider::ClaudeCode, CLAUDE, token)
}

fn codex(token: Option<u128>) -> AgentResumeCandidate {
    candidate(AgentResumeProvider::Codex, CODEX, token)
}

fn reference(provider: AgentResumeProvider, key: &str) -> Option<RetainedAgentSession> {
    RetainedAgentSession::new(provider, key)
}

fn agent(provider: AgentReportProvider) -> AgentProcessIdentity {
    AgentProcessIdentity::Agent {
        provider,
        grade: AgentIdentityGrade::Recognized,
    }
}

const OPEN: CommandEvidence = CommandEvidence::Open(RUN.to_be_bytes());
const UNKNOWN: AgentProcessIdentity =
    AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::Settling);

/// The hook rule alone: a generation with no published run, run table entry or command line.
fn select(
    candidates: &[AgentResumeCandidate],
    evidence: CommandEvidence,
    identity: &AgentProcessIdentity,
) -> Option<RetainedAgentSession> {
    super::select(candidates, evidence, identity, None, &[], None)
}

#[test]
fn command_evidence_keeps_the_open_run_else_tokenless_and_refutes_ended_runs() {
    let claude_ref = reference(AgentResumeProvider::ClaudeCode, CLAUDE);
    let codex_ref = reference(AgentResumeProvider::Codex, CODEX);
    // The open command's token names its run over a tokenless one and over other tokens.
    assert_eq!(
        select(&[claude(Some(RUN)), codex(None)], OPEN, &UNKNOWN),
        claude_ref
    );
    assert_eq!(
        select(&[claude(Some(OTHER_RUN)), codex(Some(RUN))], OPEN, &UNKNOWN),
        codex_ref
    );
    // No token matches: only tokenless candidates remain.
    assert_eq!(
        select(&[claude(Some(OTHER_RUN)), codex(None)], OPEN, &UNKNOWN),
        codex_ref
    );
    assert_eq!(select(&[claude(Some(OTHER_RUN))], OPEN, &UNKNOWN), None);
    // A completed command ends every tokened run.
    assert_eq!(
        select(
            &[claude(Some(RUN)), codex(None)],
            CommandEvidence::Closed,
            &UNKNOWN
        ),
        codex_ref
    );
    assert_eq!(
        select(&[claude(Some(RUN))], CommandEvidence::Closed, &UNKNOWN),
        None
    );
    // An unmarked shell refutes nothing.
    assert_eq!(
        select(&[claude(Some(RUN))], CommandEvidence::Unmarked, &UNKNOWN),
        claude_ref
    );
    assert_eq!(select(&[], CommandEvidence::Unmarked, &UNKNOWN), None);
}

#[test]
fn process_identity_keeps_its_provider_and_none_drops_every_candidate() {
    let both = [claude(None), codex(None)];
    let unmarked = CommandEvidence::Unmarked;
    assert_eq!(
        select(&both, unmarked, &agent(AgentReportProvider::Claude)),
        reference(AgentResumeProvider::ClaudeCode, CLAUDE)
    );
    assert_eq!(
        select(&both, unmarked, &agent(AgentReportProvider::Codex)),
        reference(AgentResumeProvider::Codex, CODEX)
    );
    assert_eq!(
        select(
            &[claude(None)],
            unmarked,
            &agent(AgentReportProvider::Codex)
        ),
        None
    );
    assert_eq!(
        select(
            &[claude(None)],
            unmarked,
            &agent(AgentReportProvider::Gemini)
        ),
        None
    );
    assert_eq!(
        select(&[claude(None)], unmarked, &AgentProcessIdentity::None),
        None
    );
    let unsupported = AgentProcessIdentity::Unsupported(AgentIdentityUnsupportedReason::Platform);
    assert_eq!(
        select(&[claude(None)], unmarked, &unsupported),
        reference(AgentResumeProvider::ClaudeCode, CLAUDE)
    );
    // Neither refutation leaves both: ambiguity gives no reference.
    assert_eq!(select(&both, unmarked, &UNKNOWN), None);
}

#[test]
fn only_one_distinct_valid_reference_is_admitted() {
    let unmarked = CommandEvidence::Unmarked;
    // One key seen twice is still one reference.
    assert_eq!(
        select(&[claude(None), claude(Some(RUN))], unmarked, &UNKNOWN),
        reference(AgentResumeProvider::ClaudeCode, CLAUDE)
    );
    // Two keys of one provider, or one key under two providers, are ambiguous.
    let stale = candidate(AgentResumeProvider::ClaudeCode, STALE, None);
    assert_eq!(select(&[claude(None), stale], unmarked, &UNKNOWN), None);
    let shared = candidate(AgentResumeProvider::Codex, CLAUDE, None);
    assert_eq!(select(&[claude(None), shared], unmarked, &UNKNOWN), None);
    // A sole key that is not a hyphenated UUID gives none, as does an option-like one.
    for key in ["session-1", "-f8fad5b-d9cb-469f-a165-70867728950e", ""] {
        let invalid = candidate(AgentResumeProvider::ClaudeCode, key, None);
        assert_eq!(select(&[invalid], unmarked, &UNKNOWN), None, "{key}");
    }
    // An invalid key still makes a valid one ambiguous.
    let invalid = candidate(AgentResumeProvider::ClaudeCode, "session-1", None);
    assert_eq!(select(&[claude(None), invalid], unmarked, &UNKNOWN), None);
}

#[test]
fn a_run_table_entry_answers_only_for_the_published_agent_run_and_only_without_a_hook_session() {
    const ENTRY_ID: &str = "4fd1c9a2-2b5e-4c1e-9f0a-6d3b8e2a7c51";
    let run = RunAnchor {
        pid: 40,
        created: 1_000,
    };
    let entry = RunTableEntry {
        anchor: run,
        session: reference(AgentResumeProvider::ClaudeCode, ENTRY_ID).unwrap(),
    };
    let from_entry = reference(AgentResumeProvider::ClaudeCode, ENTRY_ID);
    let from_hook = reference(AgentResumeProvider::ClaudeCode, CLAUDE);
    let as_claude = agent(AgentReportProvider::Claude);
    let unmarked = CommandEvidence::Unmarked;
    let stale = candidate(AgentResumeProvider::ClaudeCode, STALE, None);
    let invalid = candidate(AgentResumeProvider::ClaudeCode, "session-1", None);
    // A later process that reuses the pid, and another process.
    let reused = RunAnchor {
        pid: 40,
        created: 2_000,
    };
    let other = RunAnchor {
        pid: 41,
        created: 1_000,
    };
    let unsupported = AgentProcessIdentity::Unsupported(AgentIdentityUnsupportedReason::Platform);
    // One capture of a generation whose published run has `entry` in its run table.
    let decide = |candidates: &[AgentResumeCandidate],
                  evidence: CommandEvidence,
                  identity: AgentProcessIdentity,
                  published: Option<RunAnchor>| {
        super::select(
            candidates,
            evidence,
            &identity,
            published,
            std::slice::from_ref(&entry),
            None,
        )
    };
    let sole = Some(run);
    assert_eq!(
        decide(&[], unmarked, as_claude, sole),
        from_entry,
        "no hook session: the published run's own entry"
    );
    assert_eq!(
        decide(&[claude(None)], unmarked, as_claude, sole),
        from_hook,
        "a hook session of the run wins"
    );
    assert_eq!(
        decide(&[claude(None), stale], unmarked, as_claude, sole),
        None,
        "hook ambiguity stays none"
    );
    assert_eq!(
        decide(&[invalid], unmarked, as_claude, sole),
        None,
        "a sole invalid hook key stays none"
    );
    assert_eq!(
        decide(&[codex(None)], unmarked, as_claude, sole),
        from_entry,
        "another provider's candidate is refuted, leaving the entry"
    );
    assert_eq!(
        decide(
            &[claude(Some(RUN))],
            CommandEvidence::Closed,
            as_claude,
            sole
        ),
        from_entry,
        "an ended command's candidate is refuted, leaving the entry"
    );
    assert_eq!(
        decide(&[], unmarked, as_claude, Some(reused)),
        None,
        "an entry of an earlier process with the reused pid"
    );
    assert_eq!(
        decide(&[], unmarked, as_claude, Some(other)),
        None,
        "an entry of another process"
    );
    assert_eq!(
        decide(&[], unmarked, as_claude, None),
        None,
        "no published run"
    );
    assert_eq!(
        decide(&[], unmarked, UNKNOWN, sole),
        None,
        "an identity still settling"
    );
    assert_eq!(
        decide(&[], unmarked, agent(AgentReportProvider::Codex), sole),
        None,
        "an identity of another provider"
    );
    assert_eq!(
        decide(&[], unmarked, AgentProcessIdentity::None, sole),
        None,
        "no Agent"
    );
    assert_eq!(
        decide(&[], unmarked, unsupported, sole),
        None,
        "an unsupported identity"
    );
    // Without an entry, a run with no hook session has no reference.
    assert_eq!(
        super::select(&[], unmarked, &as_claude, Some(run), &[], None),
        None
    );
}

#[test]
fn a_command_line_id_answers_only_for_the_published_agent_run_after_hooks_and_the_run_table() {
    const TABLE_ID: &str = "4fd1c9a2-2b5e-4c1e-9f0a-6d3b8e2a7c51";
    const LINE_ID: &str = "9b2f6c1e-5d3a-4e8b-a7c4-1f0e2d3c4b5a";
    const OTHER_ID: &str = "c56a4180-65aa-42ec-a945-5fd21dec0538";
    let run = RunAnchor {
        pid: 40,
        created: 1_000,
    };
    let reused = RunAnchor {
        pid: 40,
        created: 2_000,
    };
    let table = [RunTableEntry {
        anchor: run,
        session: reference(AgentResumeProvider::ClaudeCode, TABLE_ID).unwrap(),
    }];
    let args = |line: &str| -> Vec<String> {
        line.split(' ')
            .filter(|token| !token.is_empty())
            .map(|token| {
                token
                    .replace("{line}", LINE_ID)
                    .replace("{other}", OTHER_ID)
            })
            .collect()
    };
    let grok = agent(AgentReportProvider::Grok);
    let claude_agent = agent(AgentReportProvider::Claude);
    let hook = candidate(AgentResumeProvider::Grok, CLAUDE, None);
    let claude_hook = claude(None);
    let none: &[AgentResumeCandidate] = &[];
    let from = |provider, id| reference(provider, id);
    // (row, hook candidates, identity, run table, sampled run and its command line, expected)
    type Row<'a> = (
        &'a str,
        &'a [AgentResumeCandidate],
        &'a AgentProcessIdentity,
        &'a [RunTableEntry],
        Option<(RunAnchor, &'a str)>,
        Option<RetainedAgentSession>,
    );
    let rows: &[Row<'_>] = &[
        (
            "a command-line id alone",
            none,
            &grok,
            &[],
            Some((run, "--session-id {line}")),
            from(AgentResumeProvider::Grok, LINE_ID),
        ),
        (
            "a hand-typed resume of the same provider",
            none,
            &grok,
            &[],
            Some((run, "--resume={line}")),
            from(AgentResumeProvider::Grok, LINE_ID),
        ),
        (
            "Claude Code without a run table entry reads its command line",
            none,
            &claude_agent,
            &[],
            Some((run, "--resume {line}")),
            from(AgentResumeProvider::ClaudeCode, LINE_ID),
        ),
        (
            "the run table beats the command line for Claude Code",
            none,
            &claude_agent,
            &table,
            Some((run, "--resume {line}")),
            from(AgentResumeProvider::ClaudeCode, TABLE_ID),
        ),
        (
            "a hook candidate beats the command line",
            std::slice::from_ref(&hook),
            &grok,
            &[],
            Some((run, "--session-id {line}")),
            from(AgentResumeProvider::Grok, CLAUDE),
        ),
        (
            "a hook candidate beats the run table and the command line",
            std::slice::from_ref(&claude_hook),
            &claude_agent,
            &table,
            Some((run, "--resume {line}")),
            from(AgentResumeProvider::ClaudeCode, CLAUDE),
        ),
        (
            "two bound ids give none",
            none,
            &grok,
            &[],
            Some((run, "--session-id {line} --resume={other}")),
            None,
        ),
        (
            "one id bound twice is still two bindings",
            none,
            &grok,
            &[],
            Some((run, "-s {line} --session-id {line}")),
            None,
        ),
        (
            "a continue flag gives none",
            none,
            &grok,
            &[],
            Some((run, "--continue --session-id {line}")),
            None,
        ),
        (
            "another provider's id format gives none",
            none,
            &grok,
            &[],
            Some((run, "--resume=ses_0123456789abCdEfGhIjKlMnOp")),
            None,
        ),
        (
            "a command line of another process with the reused pid gives none",
            none,
            &grok,
            &[],
            Some((reused, "--session-id {line}")),
            None,
        ),
        (
            "no sampled command line gives none",
            none,
            &grok,
            &[],
            None,
            None,
        ),
        (
            "an identity of another provider reads by its own table",
            none,
            &agent(AgentReportProvider::Codex),
            &[],
            Some((run, "--session-id {line}")),
            None,
        ),
        (
            "an identity still settling gives none",
            none,
            &UNKNOWN,
            &[],
            Some((run, "--session-id {line}")),
            None,
        ),
    ];
    for (row, candidates, identity, entries, sampled, want) in rows {
        let sampled = sampled.map(|(anchor, line)| (anchor, args(line)));
        let want = want.clone();
        let got = super::select(
            candidates,
            CommandEvidence::Unmarked,
            identity,
            Some(run),
            entries,
            sampled
                .as_ref()
                .map(|(anchor, args)| (*anchor, args.as_slice())),
        );
        assert_eq!(got, want, "{row}");
    }
}

#[test]
fn a_carried_reference_binds_only_its_own_run_and_is_written_only_while_that_run_lives() {
    use super::{Carried, CarriedMoment as M, CarriedOutcome as O, PublishedRun, RunAnchor};
    let typed = vec![format!("--resume={CLAUDE}")];
    let unbound = Carried {
        session: reference(AgentResumeProvider::ClaudeCode, CLAUDE).unwrap(),
        arguments: typed.clone(),
        anchor: None,
    };
    let anchor = RunAnchor {
        pid: 40,
        created: 1_000,
    };
    let bound = Carried {
        anchor: Some(anchor),
        ..unbound.clone()
    };
    let resumed: Vec<String> = ["node.exe", r"C:\npm\cli.js"]
        .into_iter()
        .map(str::to_owned)
        .chain(typed)
        .collect();
    let manual = vec!["claude.exe".to_owned()];
    let claude = agent(AgentReportProvider::Claude);
    let codex = agent(AgentReportProvider::Codex);
    let unclassified =
        AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::UnclassifiedArguments);
    let ambiguous = AgentProcessIdentity::Unknown(AgentIdentityUnknownReason::AmbiguousCandidates);
    fn run(
        anchor: RunAnchor,
        identity: AgentProcessIdentity,
        argv: Option<&[String]>,
    ) -> Option<PublishedRun<'_>> {
        Some(PublishedRun {
            anchor,
            identity,
            argv,
        })
    }
    let (resumed, manual) = (Some(resumed.as_slice()), Some(manual.as_slice()));
    let reused = RunAnchor {
        pid: 40,
        created: 2_000,
    };
    let rows: [(&str, &Carried, M<'_>, O); 16] = [
        // A sample commit: the first published run decides an unbound reference.
        ("no run yet", &unbound, M::Sample(None), O::Keep),
        (
            "its own run",
            &unbound,
            M::Sample(run(anchor, claude, resumed)),
            O::Bind(anchor),
        ),
        (
            "another provider",
            &unbound,
            M::Sample(run(anchor, codex, resumed)),
            O::Drop,
        ),
        (
            "a manual start",
            &unbound,
            M::Sample(run(anchor, claude, manual)),
            O::Drop,
        ),
        (
            "unreadable arguments",
            &unbound,
            M::Sample(run(anchor, claude, None)),
            O::Drop,
        ),
        (
            "not yet an Agent",
            &unbound,
            M::Sample(run(anchor, unclassified, resumed)),
            O::Drop,
        ),
        (
            "already bound",
            &bound,
            M::Sample(run(reused, claude, manual)),
            O::Keep,
        ),
        // A capture: a candidate hands over to the live rule; only a living bound run is written.
        (
            "its run lives",
            &bound,
            M::Capture {
                candidate: false,
                run: run(anchor, claude, None),
            },
            O::Write,
        ),
        (
            "a candidate",
            &bound,
            M::Capture {
                candidate: true,
                run: run(anchor, claude, None),
            },
            O::Drop,
        ),
        (
            "a candidate before binding",
            &unbound,
            M::Capture {
                candidate: true,
                run: None,
            },
            O::Drop,
        ),
        (
            "the anchor is gone",
            &bound,
            M::Capture {
                candidate: false,
                run: None,
            },
            O::Drop,
        ),
        (
            "a reused pid",
            &bound,
            M::Capture {
                candidate: false,
                run: run(reused, claude, None),
            },
            O::Drop,
        ),
        (
            "the run is no longer its provider",
            &bound,
            M::Capture {
                candidate: false,
                run: run(anchor, codex, None),
            },
            O::Drop,
        ),
        (
            "the run is ambiguous",
            &bound,
            M::Capture {
                candidate: false,
                run: run(anchor, ambiguous, None),
            },
            O::Drop,
        ),
        (
            "still unbound",
            &unbound,
            M::Capture {
                candidate: false,
                run: None,
            },
            O::Keep,
        ),
        (
            "still unbound beside a run",
            &unbound,
            M::Capture {
                candidate: false,
                run: run(anchor, claude, None),
            },
            O::Keep,
        ),
    ];
    for (case, carried, moment, expected) in rows {
        assert_eq!(super::carried_outcome(carried, moment), expected, "{case}");
    }
}
