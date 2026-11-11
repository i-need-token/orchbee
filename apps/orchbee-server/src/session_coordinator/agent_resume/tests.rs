use super::CommandEvidence;
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

/// The hook rule alone: a generation with no published run and no run table entry.
fn select(
    candidates: &[AgentResumeCandidate],
    evidence: CommandEvidence,
    identity: &AgentProcessIdentity,
) -> Option<RetainedAgentSession> {
    super::select(candidates, evidence, identity, None, &[])
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
