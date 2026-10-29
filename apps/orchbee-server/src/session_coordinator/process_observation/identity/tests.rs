//! Deterministic traces for the identity state: settle, identifier reuse, restart, hand-off,
//! ambiguity, failure runs, detached consoles, and the published-run compatibility rule.
use super::*;
use AgentReportProvider as P;
use orchbee_agent_observation_protocol::{
    AgentIdentityGrade, AgentInstallShape, AgentProcessRecognition, AgentUnresolvedProcess,
    evaluate_agent_sample,
};

const SHELL: AgentProcessRow = AgentProcessRow {
    pid: 10,
    parent_pid: None,
    creation_time: 100,
    opaque: false,
    detached: false,
    recognition: AgentProcessRecognition::NotAgent,
};

fn agent(pid: u32, parent: u32, created: u64, provider: P) -> AgentProcessRow {
    AgentProcessRow {
        pid,
        parent_pid: Some(parent),
        creation_time: created,
        recognition: AgentProcessRecognition::Interactive {
            provider,
            shape: AgentInstallShape::NativeImage,
        },
        ..SHELL
    }
}

struct Trace {
    state: IdentityState,
    origin: Instant,
}

impl Trace {
    fn new() -> Self {
        Self {
            state: IdentityState::default(),
            origin: Instant::now(),
        }
    }
    fn at(&self, ms: u64) -> Instant {
        self.origin + Duration::from_millis(ms)
    }
    /// Applies a successful sample and returns the ended run's sequence.
    fn sample(&mut self, ms: u64, rows: &[AgentProcessRow]) -> Option<u32> {
        let verdict = evaluate_agent_sample(rows, false);
        self.state
            .observe(self.at(ms), rows, verdict)
            .map(|run| run.sequence)
    }
    fn fail(&mut self, ms: u64) {
        self.state.fail(self.at(ms));
    }
    fn result(&self) -> Identity {
        self.state.result()
    }
    /// The published run's sequence and anchor identifier.
    fn published(&self) -> Option<(u32, u32)> {
        self.state.published().map(|run| (run.sequence, run.pid))
    }
}

fn recognized(provider: P) -> Identity {
    Identity::Agent {
        provider,
        grade: AgentIdentityGrade::Recognized,
    }
}

#[test]
fn settle_needs_the_same_anchor_in_two_samples_at_least_the_window_apart() {
    let mut t = Trace::new();
    let rows = [SHELL, agent(20, 10, 200, P::Claude)];
    assert_eq!(t.result(), Identity::Unknown(Reason::Settling));
    t.sample(0, &rows);
    t.sample(250, &rows);
    t.sample(499, &rows);
    assert_eq!(t.result(), Identity::Unknown(Reason::Settling));
    assert!(t.state.tracking() && t.published().is_none());
    t.sample(500, &rows);
    assert_eq!(t.result(), recognized(P::Claude));
    assert_eq!(t.published(), Some((1, 20)));
    // A different anchor while settling restarts the window.
    let mut t = Trace::new();
    t.sample(0, &rows);
    t.sample(400, &[SHELL, agent(21, 10, 201, P::Claude)]);
    t.sample(600, &[SHELL, agent(21, 10, 201, P::Claude)]);
    assert_eq!(t.result(), Identity::Unknown(Reason::Settling));
    t.sample(900, &[SHELL, agent(21, 10, 201, P::Claude)]);
    assert_eq!(t.published(), Some((1, 21)));
    // An idle shell is `none`; a remote client is unsupported.
    t.sample(1000, &[SHELL]);
    assert_eq!(t.result(), Identity::None);
    let ssh = AgentProcessRow {
        pid: 30,
        parent_pid: Some(10),
        creation_time: 300,
        recognition: AgentProcessRecognition::RemoteClient,
        ..SHELL
    };
    t.sample(1100, &[SHELL, ssh]);
    assert_eq!(
        t.result(),
        Identity::Unsupported(AgentIdentityUnsupportedReason::RemoteExecution)
    );
}

#[test]
fn identifier_reuse_ends_the_run_and_never_transfers_it() {
    let mut t = Trace::new();
    let rows = [SHELL, agent(20, 10, 200, P::Claude)];
    t.sample(0, &rows);
    t.sample(500, &rows);
    // The same identifier with a later creation time is another process.
    let reused = [SHELL, agent(20, 10, 201, P::Claude)];
    assert_eq!(t.sample(600, &reused), Some(1));
    assert_eq!(t.published(), None);
    assert_eq!(t.result(), Identity::Unknown(Reason::Settling));
    t.sample(1100, &reused);
    assert_eq!(t.published(), Some((2, 20)));
    // A child whose parent identifier was reused by a later process is no hand-off.
    let mut t = Trace::new();
    let launcher = agent(20, 10, 200, P::Claude);
    t.sample(0, &[SHELL, launcher]);
    t.sample(500, &[SHELL, launcher]);
    let impostor = AgentProcessRow {
        creation_time: 400,
        recognition: AgentProcessRecognition::NotAgent,
        ..launcher
    };
    let child = agent(30, 20, 300, P::Claude);
    assert_eq!(t.sample(600, &[SHELL, impostor, child]), Some(1));
    assert_eq!(t.published(), None);
}

#[test]
fn restart_is_a_new_run_and_hand_off_keeps_the_run() {
    let mut t = Trace::new();
    let first = [SHELL, agent(20, 10, 200, P::Claude)];
    t.sample(0, &first);
    t.sample(500, &first);
    // Exit, then an immediate restart of another provider.
    assert_eq!(
        t.sample(600, &[SHELL, agent(21, 10, 210, P::Codex)]),
        Some(1)
    );
    t.sample(1100, &[SHELL, agent(21, 10, 210, P::Codex)]);
    assert_eq!(t.published(), Some((2, 21)));
    assert_eq!(t.result(), recognized(P::Codex));
    // A launcher exits while its same-provider child continues: the same run hands off.
    let mut t = Trace::new();
    let launcher = agent(20, 10, 200, P::Claude);
    let child = agent(30, 20, 300, P::Claude);
    t.sample(0, &[SHELL, launcher]);
    t.sample(500, &[SHELL, launcher, child]);
    assert_eq!(t.sample(600, &[SHELL, child]), None);
    assert_eq!(t.published(), Some((1, 30)));
    // A departing anchor whose only descendant is another provider ends the run.
    let nested = agent(40, 30, 400, P::Codex);
    assert_eq!(t.sample(700, &[SHELL, child, nested]), None);
    assert_eq!(t.sample(800, &[SHELL, nested]), Some(1));
    assert_eq!(t.published(), None);
}

#[test]
fn ambiguity_keeps_a_live_run_and_blocks_a_new_one() {
    let mut t = Trace::new();
    let claude = agent(20, 10, 200, P::Claude);
    let sibling = agent(30, 10, 300, P::Codex);
    t.sample(0, &[SHELL, claude]);
    t.sample(500, &[SHELL, claude]);
    t.sample(600, &[SHELL, claude, sibling]);
    assert_eq!(t.result(), Identity::Unknown(Reason::AmbiguousCandidates));
    assert_eq!(t.published(), Some((1, 20)));
    // An opaque row outside the anchor keeps the run; inside it, nothing changes.
    let opaque = AgentProcessRow {
        pid: 40,
        parent_pid: Some(10),
        creation_time: 400,
        opaque: true,
        ..SHELL
    };
    t.sample(700, &[SHELL, claude, opaque]);
    assert_eq!(t.result(), Identity::Unknown(Reason::CommandLineUnreadable));
    assert_eq!(t.published(), Some((1, 20)));
    let inside = AgentProcessRow {
        parent_pid: Some(20),
        ..opaque
    };
    t.sample(800, &[SHELL, claude, inside]);
    assert_eq!(t.result(), recognized(P::Claude));
    // Siblings before settling never start a run.
    let mut t = Trace::new();
    for ms in [0, 500, 1000] {
        t.sample(ms, &[SHELL, claude, sibling]);
    }
    assert!(t.state.run().is_none() && !t.state.tracking());
    assert_eq!(t.result(), Identity::Unknown(Reason::AmbiguousCandidates));
}

#[test]
fn failures_keep_the_result_briefly_and_never_end_a_run() {
    let mut t = Trace::new();
    let rows = [SHELL, agent(20, 10, 200, P::Claude)];
    t.fail(0);
    assert_eq!(t.result(), Identity::Unknown(Reason::SampleFailed));
    t.sample(100, &rows);
    // A failure inside the window keeps settling progress.
    t.fail(300);
    t.sample(600, &rows);
    assert_eq!(t.result(), recognized(P::Claude));
    for ms in [700, 1500, 2599] {
        t.fail(ms);
        assert_eq!(t.result(), recognized(P::Claude));
    }
    t.fail(2600);
    assert_eq!(t.result(), Identity::Unknown(Reason::SampleFailed));
    t.fail(10_000);
    assert_eq!(t.published(), Some((1, 20)));
    // The next successful sample decides: the anchor is gone, so the run ends.
    assert_eq!(t.sample(10_100, &[SHELL]), Some(1));
    assert_eq!(t.result(), Identity::None);
}

#[test]
fn detached_and_unresolved_anchors_keep_reasons_and_compatible_publication() {
    // A program that has not attached yet settles once it attaches.
    let mut t = Trace::new();
    let attached = agent(20, 10, 200, P::Claude);
    let detached = AgentProcessRow {
        detached: true,
        ..attached
    };
    t.sample(0, &[SHELL, detached]);
    t.sample(500, &[SHELL, attached]);
    assert_eq!(t.result(), recognized(P::Claude));
    assert_eq!(t.published(), Some((1, 20)));
    // One that stays detached is visible as unknown and never published.
    let mut t = Trace::new();
    t.sample(0, &[SHELL, detached]);
    t.sample(500, &[SHELL, detached]);
    assert_eq!(t.result(), Identity::Unknown(Reason::DetachedConsole));
    assert_eq!(t.published(), None);
    // Unlisted arguments stay unknown but keep their shipped Panel visibility.
    let mut t = Trace::new();
    let unlisted = AgentProcessRow {
        recognition: AgentProcessRecognition::Unresolved {
            provider: Some(P::Claude),
            reason: AgentUnresolvedProcess::UnclassifiedArguments,
        },
        ..attached
    };
    t.sample(0, &[SHELL, unlisted]);
    t.sample(500, &[SHELL, unlisted]);
    assert_eq!(t.result(), Identity::Unknown(Reason::UnclassifiedArguments));
    assert_eq!(t.published(), Some((1, 20)));
}
