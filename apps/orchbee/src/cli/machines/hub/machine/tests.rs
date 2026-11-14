use super::*;
use orchbee_connection_handshake::RejectionReason;
use orchbee_machine_protocol::{AnswerToken, HostKey, KeyFingerprint, PathText};

mod remembered;
mod waiting;

/// The model's clock and the hub's serials, advanced by hand.
struct Hub {
    start: Instant,
    after: Duration,
    serials: Serials,
    presence: bool,
    closing: bool,
    askers: Vec<u64>,
}

impl Hub {
    fn new(presence: bool) -> Self {
        Self {
            start: Instant::now(),
            after: Duration::ZERO,
            serials: Serials::default(),
            presence,
            closing: false,
            askers: Vec::new(),
        }
    }

    fn at(&mut self, after: Duration) -> &mut Self {
        self.after = after;
        self
    }

    fn cx(&mut self) -> Context<'_> {
        Context {
            now: Moment {
                at: self.start + self.after,
                wall: 1_000_000 + u64::try_from(self.after.as_millis()).unwrap(),
            },
            presence: self.presence,
            closing: self.closing,
            askers: &self.askers,
            question: QUESTION,
            serials: &mut self.serials,
        }
    }
}

/// Raises a question while nothing is remembered: the recall a password or passphrase starts is
/// answered as a helper with an empty store answers it.
fn question(
    machine: &mut Machine,
    carrier: u64,
    raised: Raised,
    cx: &mut Context<'_>,
) -> Vec<Effect> {
    let effects = machine.question(carrier, raised, true, cx);
    if matches!(effects[..], [Effect::Recall { .. }]) {
        return machine.unrecalled(carrier, cx);
    }
    effects
}

fn machine() -> Machine {
    Machine::new(
        MachineId::new("11111111111111111111111111111111").unwrap(),
        ssh("far"),
    )
}

/// The route toward `user@host.invalid` and the remote Session `session`.
fn toward(session: &str) -> Route {
    Route {
        target: "user@host.invalid".to_owned(),
        session: SessionName::new(session).unwrap(),
    }
}

fn ssh(session: &str) -> Kind {
    Kind::Ssh(toward(session))
}

/// The route of the carrier `effects` start, if any.
fn route(effects: &[Effect]) -> Option<&Route> {
    effects.iter().find_map(|effect| match effect {
        Effect::Start { route, .. } => Some(route),
        _ => None,
    })
}

fn waiter(request: u32) -> Waiter {
    asker(1, request, Answers::Never)
}

fn asker(peer: u64, request: u32, answers: Answers) -> Waiter {
    Waiter::new(peer, Id::new(request).unwrap(), answers)
}

fn host_key() -> HostKey {
    HostKey {
        algorithm: Token::new("ed25519").unwrap(),
        fingerprint: KeyFingerprint::new("SHA256:0123456789abcdefABCDEF").unwrap(),
    }
}

/// `ssh`'s own prompt for `kind` toward `host.invalid`.
fn prompt_of(kind: &QuestionKind) -> String {
    match kind {
        QuestionKind::Password => "user@host.invalid's password: ".to_owned(),
        QuestionKind::KeyPassphrase => "Enter passphrase for key 'id_ed25519': ".to_owned(),
        QuestionKind::HostKey { host_key } => new_host("host.invalid (192.0.2.1)", host_key),
        QuestionKind::Other => "(user@host.invalid) Verification code: ".to_owned(),
    }
}

/// OpenSSH's first-time host sentence naming `named` and offering `host_key`.
fn new_host(named: &str, host_key: &HostKey) -> String {
    format!(
        "The authenticity of host '{named}' can't be established.\n{} key fingerprint is {}.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ",
        host_key.algorithm.as_str().to_ascii_uppercase(),
        host_key.fingerprint.as_str()
    )
}

/// A question as the askpass helper raises it, with `ssh`'s prompt for its kind.
fn raised(kind: QuestionKind) -> Raised {
    let prompt = prompt_of(&kind);
    raised_as(kind, &prompt)
}

/// A question of `kind` the askpass helper raises with `prompt`.
fn raised_as(kind: QuestionKind, prompt: &str) -> Raised {
    Raised {
        kind,
        prompt: Prompt::new(prompt).unwrap(),
        answer: AnswerEndpoint {
            endpoint: PathText::new(r"\\.\pipe\orchbee-answer-1-0").unwrap(),
            token: AnswerToken::from_bytes([7; 16]),
        },
    }
}

/// The events `effects` send, with their connections, in order.
fn notified(effects: &[Effect]) -> Vec<(u64, &Event)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Notify { peer, event } => Some((*peer, event)),
            _ => None,
        })
        .collect()
}

/// The question `effects` put to the claimant: its id and whether it repeats a refused kind.
fn asked(effects: &[Effect]) -> Option<(u32, bool)> {
    notified(effects)
        .into_iter()
        .find_map(|(_, event)| match event {
            Event::Question(question) => Some((question.question.get(), question.repeated)),
            _ => None,
        })
}

/// The end of the episode the `login` events of `effects` report.
fn login_end(effects: &[Effect]) -> Option<LoginEnd> {
    notified(effects)
        .into_iter()
        .find_map(|(_, event)| match event {
            Event::Login(LoginEvent {
                phase: LoginPhase::Ended { end },
                ..
            }) => Some(*end),
            _ => None,
        })
}

/// Whether `effects` end the carrier before they close its helper.
fn ends_then_closes(effects: &[Effect], carrier: u64) -> bool {
    let end = effects.iter().position(|e| *e == Effect::End { carrier });
    let close = effects.iter().position(|e| *e == Effect::Close { carrier });
    matches!((end, close), (Some(end), Some(close)) if end < close)
}

fn started(effects: &[Effect]) -> Option<u64> {
    effects.iter().find_map(|effect| match effect {
        Effect::Start { carrier, .. } => Some(*carrier),
        _ => None,
    })
}

fn ends(effects: &[Effect], carrier: u64) -> bool {
    effects.contains(&Effect::End { carrier })
}

fn replies(effects: &[Effect]) -> Vec<ConnectOutcome> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Reply { result, .. } => Some(result.outcome.clone()),
            _ => None,
        })
        .collect()
}

/// Brings the attempt of `carrier` to `online`.
fn reach_online(machine: &mut Machine, hub: &mut Hub, carrier: u64) -> Vec<Effect> {
    machine.answered(carrier, ProductVersion::new("1.0.0").unwrap());
    let effects = machine.facts(
        carrier,
        Token::new("windows").unwrap(),
        Token::new("x86_64").unwrap(),
        &mut hub.cx(),
    );
    assert!(matches!(effects[..], [Effect::EnsureSession { .. }]));
    machine.session(carrier, true, &mut hub.cx())
}

fn refused() -> End {
    End::Exited {
        code: Some(1),
        stderr: Vec::new(),
    }
}

#[test]
fn a_kept_machine_retries_by_itself_and_an_unkept_one_waits_to_be_asked() {
    for presence in [true, false] {
        let mut hub = Hub::new(presence);
        let mut machine = machine();
        let first = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
        let effects = machine.ended(first, &refused(), &mut hub.at(Duration::from_secs(1)).cx());
        assert_eq!(
            replies(&effects),
            [ConnectOutcome::NotOnline(ConnectFailure::RemoteProtocol)]
        );
        if presence {
            let at = machine.next_wake().unwrap();
            assert!(matches!(machine.state(), MachineState::Reconnecting { .. }));
            assert!(
                machine
                    .tick(&mut hub.at(at - hub.start - Duration::from_millis(1)).cx())
                    .is_empty()
            );
            let retried = started(&machine.tick(&mut hub.at(at - hub.start).cx())).unwrap();
            assert_ne!(retried, first);
        } else {
            assert!(matches!(
                machine.state(),
                MachineState::Offline {
                    cause: Some(_),
                    next_attempt: None
                }
            ));
            assert_eq!(machine.next_wake(), None);
            assert!(
                machine
                    .tick(&mut hub.at(Duration::from_secs(3600)).cx())
                    .is_empty()
            );
        }
    }
}

#[test]
fn a_lost_link_reconnects_with_one_carrier_and_stale_carriers_change_nothing() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let first = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    let effects = reach_online(&mut machine, &mut hub, first);
    assert!(matches!(
        replies(&effects)[..],
        [ConnectOutcome::Online { .. }]
    ));
    assert_eq!(machine.state(), &MachineState::Online);

    let effects = machine.ended(first, &refused(), &mut hub.at(Duration::from_secs(5)).cx());
    assert!(ends(&effects, first));
    assert!(matches!(
        machine.state(),
        MachineState::Reconnecting {
            cause: Some(ReconnectCause::LinkLost),
            next_attempt: Some(_),
            ..
        }
    ));
    let at = machine.next_wake().unwrap() - hub.start;
    let second = started(&machine.tick(&mut hub.at(at).cx())).unwrap();
    assert_eq!(machine.carrier(), Some(second));

    // Events of the first carrier no longer move the Machine.
    let before = machine.state().clone();
    assert!(machine.ended(first, &refused(), &mut hub.cx()).is_empty());
    assert!(machine.broken(first, &mut hub.cx()).is_empty());
    assert_eq!(machine.state(), &before);

    reach_online(&mut machine, &mut hub, second);
    assert_eq!(machine.state(), &MachineState::Online);
    assert_eq!(machine.carrier(), Some(second));
}

#[test]
fn a_permanent_failure_waits_for_a_request_and_disconnect_cancels_waiters() {
    let mut hub = Hub::new(true);
    let mut machine = machine();
    let first = started(&machine.attach(&mut hub.cx())).unwrap();
    machine.failed(
        first,
        refusal(RejectionReason::InitiatorTooOld),
        &mut hub.cx(),
    );
    assert_eq!(machine.state(), &MachineState::LocalNeedsUpdate);
    assert_eq!(machine.next_wake(), None);
    assert!(
        machine
            .tick(&mut hub.at(Duration::from_secs(3600)).cx())
            .is_empty()
    );

    let second = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    assert!(machine.connect(waiter(2), &mut hub.cx()).is_empty());
    let effects = machine.disconnect();
    assert!(ends(&effects, second));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::Cancelled)]
    );
    assert!(matches!(
        machine.state(),
        MachineState::Offline {
            cause: None,
            next_attempt: None
        }
    ));
    assert_eq!(machine.carrier(), None);
}

#[test]
fn closing_starts_nothing_and_answers_waiters_stopping() {
    let mut hub = Hub::new(true);
    let mut machine = machine();
    let first = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    hub.closing = true;
    assert_eq!(
        replies(&machine.closing()),
        [ConnectOutcome::NotOnline(ConnectFailure::Stopping)]
    );
    assert_eq!(
        replies(&machine.connect(waiter(2), &mut hub.cx())),
        [ConnectOutcome::NotOnline(ConnectFailure::Stopping)]
    );
    let effects = machine.ended(first, &refused(), &mut hub.at(Duration::from_secs(1)).cx());
    assert_eq!(started(&effects), None);
    assert_eq!(machine.next_wake(), None);
    assert_eq!(started(&machine.attach(&mut hub.cx())), None);
    assert!(
        machine
            .tick(&mut hub.at(Duration::from_secs(3600)).cx())
            .is_empty()
    );
}

#[test]
fn a_record_a_reload_adds_changes_or_enables_is_attempted_while_presence_holds() {
    for presence in [true, false] {
        let mut hub = Hub::new(presence);
        let attempted = |effects: &[Effect]| route(effects).cloned();
        let expected = |session: &str| presence.then(|| toward(session));

        let mut added = machine();
        assert_eq!(attempted(&added.arrived(&mut hub.cx())), expected("far"));

        // A Machine kept online whose Session changes ends its carrier first.
        let mut changed = machine();
        let first = started(&changed.connect(waiter(1), &mut hub.cx())).unwrap();
        reach_online(&mut changed, &mut hub, first);
        let effects = changed.reconcile(ssh("other"), &mut hub.cx());
        assert!(ends(&effects, first));
        assert_eq!(attempted(&effects), expected("other"));

        let mut enabled = Machine::new(machine().id().clone(), Kind::Disabled);
        let effects = enabled.reconcile(ssh("far"), &mut hub.cx());
        assert_eq!(attempted(&effects), expected("far"));

        // An unchanged record keeps what it has, and a record that cannot connect starts nothing.
        let mut idle = machine();
        assert!(idle.reconcile(ssh("far"), &mut hub.cx()).is_empty());
        let mut disabled = machine();
        assert_eq!(
            attempted(&disabled.reconcile(Kind::Disabled, &mut hub.cx())),
            None
        );
    }
}

#[test]
fn a_connect_ended_by_a_changed_record_reads_cancelled_once() {
    let mut hub = Hub::new(true);
    let mut machine = machine();
    let first = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    let effects = machine.reconcile(ssh("other"), &mut hub.cx());
    assert!(ends(&effects, first));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::Cancelled)]
    );
    // The silent attempt toward the new record owes that request nothing.
    let second = started(&effects).unwrap();
    assert!(replies(&reach_online(&mut machine, &mut hub, second)).is_empty());
    assert_eq!(machine.state(), &MachineState::Online);
}

#[test]
fn the_attempt_deadline_and_an_unanswered_ping_change_the_state() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let first = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    let deadline = machine.next_wake().unwrap() - hub.start;
    let effects = machine.tick(&mut hub.at(deadline).cx());
    assert!(ends(&effects, first));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::HostUnreachable)]
    );

    let second = started(&machine.connect(waiter(2), &mut hub.cx())).unwrap();
    reach_online(&mut machine, &mut hub, second);
    let ping = machine.next_wake().unwrap() - hub.start;
    assert_eq!(
        machine.tick(&mut hub.at(ping).cx()),
        [Effect::Ping { carrier: second }]
    );
    let silent = machine.next_wake().unwrap() - hub.start;
    machine.tick(&mut hub.at(silent).cx());
    assert_eq!(machine.state(), &MachineState::Unresponsive);
    assert!(machine.connect(waiter(3), &mut hub.cx()).is_empty());
    assert!(matches!(
        replies(&machine.pong(second))[..],
        [ConnectOutcome::Online { .. }]
    ));
    assert_eq!(machine.state(), &MachineState::Online);
}

#[test]
fn an_end_before_the_answer_reads_by_exit_status_and_standard_error() {
    let ssh = |text: &str| End::Exited {
        code: Some(255),
        stderr: text.as_bytes().to_vec(),
    };
    let cases = [
        (
            End::NotStarted { missing: true },
            Failure::Offline(OfflineCause::SshUnavailable),
        ),
        (
            End::Exited {
                code: Some(125),
                stderr: Vec::new(),
            },
            Failure::NotInstalled,
        ),
        (
            ssh("Permission denied (publickey,password,keyboard-interactive)."),
            Failure::NeedsLogin,
        ),
        (
            ssh("Permission denied (publickey)."),
            Failure::LoginFailed(orchbee_machine_protocol::hub::LoginFailedCause::KeyRejected),
        ),
        (ssh("Host key verification failed."), Failure::NeedsLogin),
        (
            ssh(
                "@ WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! @\nHost key verification failed.",
            ),
            Failure::KeyChanged(None),
        ),
        (
            ssh("ssh: connect to host host.invalid port 22: Connection refused"),
            Failure::Offline(OfflineCause::HostRefused),
        ),
        (
            ssh("kex_exchange_identification: read: Connection reset"),
            Failure::Offline(OfflineCause::SshFailed),
        ),
        (
            End::Exited {
                code: Some(2),
                stderr: Vec::new(),
            },
            Failure::Offline(OfflineCause::RemoteProtocol),
        ),
    ];
    for (end, failure) in cases {
        assert_eq!(classify(&end), failure, "{end:?}");
    }
}

#[test]
fn a_claimant_receives_each_question_in_order_and_a_refused_answer_is_marked_repeated() {
    // A `user` request, and a `command` that wants answers while nobody else asks, both claim.
    for answers in [Answers::User, Answers::Command] {
        let mut hub = Hub::new(false);
        let mut machine = machine();
        let carrier = started(&machine.connect(asker(4, 1, answers), &mut hub.cx())).unwrap();
        let effects = question(
            &mut machine,
            carrier,
            raised(QuestionKind::Password),
            &mut hub.cx(),
        );
        let kinds: Vec<_> = notified(&effects)
            .into_iter()
            .map(|(peer, event)| {
                assert_eq!(peer, 4);
                match event {
                    Event::ConnectWaiting(waiting) => {
                        assert_eq!(waiting.request.get(), 1);
                        "connect-waiting"
                    }
                    Event::Login(login) => {
                        assert_eq!(login.trigger, Trigger::User);
                        assert_eq!(
                            login.phase,
                            LoginPhase::Claimed {
                                claim: Claimant::You
                            }
                        );
                        "login"
                    }
                    Event::Question(_) => "question",
                    other => panic!("{other:?}"),
                }
            })
            .collect();
        assert_eq!(kinds, ["connect-waiting", "login", "question"]);
        assert_eq!(asked(&effects), Some((1, false)));
        assert_eq!(
            machine.state(),
            &MachineState::Connecting {
                stage: Some(Stage::Authenticating)
            }
        );

        // `ssh` asking the same kind after its answer was taken means it refused that answer.
        machine.taken(carrier, &hub.at(Duration::from_secs(5)).cx());
        let effects = question(
            &mut machine,
            carrier,
            raised(QuestionKind::Password),
            &mut hub.cx(),
        );
        assert_eq!(asked(&effects), Some((2, true)));
        machine.taken(carrier, &hub.cx());
        let effects = question(
            &mut machine,
            carrier,
            raised(QuestionKind::KeyPassphrase),
            &mut hub.cx(),
        );
        assert_eq!(asked(&effects), Some((3, false)));
        machine.taken(carrier, &hub.cx());

        // `ssh` giving up after taken answers refused them; the hub retries nothing.
        let effects = machine.ended(
            carrier,
            &End::Exited {
                code: Some(255),
                stderr: b"user@host.invalid: Permission denied (publickey,password).".to_vec(),
            },
            &mut hub.cx(),
        );
        assert_eq!(login_end(&effects), Some(LoginEnd::Failed));
        assert_eq!(
            replies(&effects),
            [ConnectOutcome::NotOnline(ConnectFailure::AuthFailed)]
        );
        assert_eq!(
            machine.state(),
            &MachineState::LoginFailed {
                cause: Some(LoginFailedCause::PasswordRejected)
            }
        );
        assert_eq!(machine.next_wake(), None);
        let later = machine.tick(&mut hub.at(Duration::from_secs(3600)).cx());
        assert_eq!(started(&later), None);
    }
}

#[test]
fn a_question_nobody_can_answer_ends_the_attempt_in_its_state_without_an_event() {
    // A silent attempt, and a command without answers while no client asks.
    for case in 0..2 {
        for kind in [
            QuestionKind::HostKey {
                host_key: host_key(),
            },
            QuestionKind::Password,
        ] {
            let mut hub = Hub::new(case == 0);
            let mut machine = machine();
            let effects = if case == 0 {
                machine.attach(&mut hub.cx())
            } else {
                machine.connect(waiter(1), &mut hub.cx())
            };
            let carrier = started(&effects).unwrap();
            let host = matches!(kind, QuestionKind::HostKey { .. });
            let effects = question(&mut machine, carrier, raised(kind), &mut hub.cx());
            assert!(notified(&effects).is_empty(), "{effects:?}");
            assert!(ends_then_closes(&effects, carrier), "{effects:?}");
            let (state, outcome) = if host {
                (
                    MachineState::Untrusted {
                        host_key: host_key(),
                    },
                    ConnectFailure::HostUntrusted,
                )
            } else {
                (
                    MachineState::NeedsLogin {
                        cause: Some(NeedsLoginCause::AuthRequired),
                    },
                    ConnectFailure::AuthRequired,
                )
            };
            assert_eq!(machine.state(), &state);
            let expected = if case == 0 {
                Vec::new()
            } else {
                vec![ConnectOutcome::NotOnline(outcome)]
            };
            assert_eq!(replies(&effects), expected);
            // Nothing tries again by itself, a kept Machine included.
            assert_eq!(machine.next_wake(), None);
        }
    }
}

#[test]
fn an_open_question_ends_at_its_own_bound_and_stops_the_attempt_clock() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(1, 1, Answers::User), &mut hub.cx())).unwrap();
    let ten = Duration::from_secs(10);
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.at(ten).cx(),
    );
    // Past the 60-second attempt deadline only the question's bound runs.
    assert_eq!(machine.next_wake(), Some(hub.start + ten + QUESTION));
    let before = ten + QUESTION - Duration::from_millis(1);
    assert!(machine.tick(&mut hub.at(before).cx()).is_empty());
    let effects = machine.tick(&mut hub.at(ten + QUESTION).cx());
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(login_end(&effects), Some(LoginEnd::Unavailable));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::Timeout)]
    );
    assert_eq!(
        machine.state(),
        &MachineState::NeedsLogin {
            cause: Some(NeedsLoginCause::AuthRequired)
        }
    );

    // Answered after 30 seconds, the attempt's deadline moves 30 seconds later.
    let start = ten + QUESTION;
    let carrier = started(&machine.connect(asker(1, 2, Answers::User), &mut hub.cx())).unwrap();
    let at = |offset: Duration| start + offset;
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.at(at(ten)).cx(),
    );
    machine.taken(carrier, &hub.at(at(ten * 4)).cx());
    assert_eq!(
        machine.next_wake(),
        Some(hub.start + start + ATTEMPT + ten * 3)
    );

    // A helper that leaves with its question open ends the attempt as an unanswered one does.
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.at(at(ten * 5)).cx(),
    );
    let effects = machine.unanswered(carrier, &hub.cx());
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::Timeout)]
    );
}

#[test]
fn only_the_claimant_dismisses_and_its_leaving_abandons_the_episode() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(1, 1, Answers::User), &mut hub.cx())).unwrap();
    machine.connect(asker(2, 1, Answers::Never), &mut hub.cx());
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    let login = machine.login().unwrap();
    let other = Id::new(login.get() + 1).unwrap();
    assert_eq!(
        machine.claim(1, login, &hub.cx()),
        (ClaimOutcome::Claimed, Vec::new())
    );
    assert_eq!(
        machine.claim(2, login, &hub.cx()),
        (ClaimOutcome::Taken, Vec::new())
    );
    assert_eq!(
        machine.claim(1, other, &hub.cx()),
        (ClaimOutcome::Ended, Vec::new())
    );
    assert_eq!(
        machine.dismiss(2, login, &hub.cx()),
        (DismissOutcome::NotClaimant, Vec::new())
    );
    assert_eq!(
        machine.dismiss(1, other, &hub.cx()).0,
        DismissOutcome::Ended
    );
    let (outcome, effects) = machine.dismiss(1, login, &hub.cx());
    assert_eq!(outcome, DismissOutcome::Dismissed);
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(login_end(&effects), Some(LoginEnd::Dismissed));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)]
    );
    assert_eq!(machine.login(), None);

    // The claimant's connection closing ends the attempt for the request still waiting.
    let carrier = started(&machine.connect(asker(1, 2, Answers::User), &mut hub.cx())).unwrap();
    machine.connect(asker(2, 2, Answers::Never), &mut hub.cx());
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    machine.taken(carrier, &hub.cx());
    let effects = machine.forget(1, &hub.cx());
    assert!(ends(&effects, carrier));
    assert_eq!(login_end(&effects), Some(LoginEnd::Abandoned));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)]
    );
    assert_eq!(
        machine.state(),
        &MachineState::NeedsLogin {
            cause: Some(NeedsLoginCause::AuthRequired)
        }
    );

    // A `disconnect` that ends an episode leaves the Machine needing a login.
    let carrier = started(&machine.connect(asker(1, 3, Answers::User), &mut hub.cx())).unwrap();
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    let effects = machine.disconnect();
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(
        machine.state(),
        &MachineState::NeedsLogin {
            cause: Some(NeedsLoginCause::Disconnected)
        }
    );
}

#[test]
fn a_trusted_host_then_a_password_reaching_online_ends_the_episode_succeeded() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(1, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = question(
        &mut machine,
        carrier,
        raised(QuestionKind::HostKey {
            host_key: host_key(),
        }),
        &mut hub.cx(),
    );
    let offered = notified(&effects)
        .into_iter()
        .find_map(|(_, event)| match event {
            Event::Question(question) => Some(question.kind.clone()),
            _ => None,
        });
    assert_eq!(
        offered,
        Some(QuestionKind::HostKey {
            host_key: host_key()
        })
    );
    machine.taken(carrier, &hub.cx());
    let effects = question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    assert_eq!(asked(&effects), Some((2, false)));
    machine.taken(carrier, &hub.cx());
    let effects = machine.answered(carrier, ProductVersion::new("1.0.0").unwrap());
    assert_eq!(login_end(&effects), Some(LoginEnd::Succeeded));
    assert_eq!(machine.login(), None);
}

#[test]
fn a_helper_that_cannot_be_asked_ends_its_carrier_before_it_is_closed() {
    let needs_login = MachineState::NeedsLogin {
        cause: Some(NeedsLoginCause::AuthRequired),
    };
    // A second helper while a question is open ends the attempt and the episode.
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(1, 1, Answers::User), &mut hub.cx())).unwrap();
    question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    let effects = question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(login_end(&effects), Some(LoginEnd::Unavailable));
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)]
    );
    assert_eq!(machine.state(), &needs_login);

    // A helper that could not build its question, before any episode.
    let carrier = started(&machine.connect(waiter(2), &mut hub.cx())).unwrap();
    let effects = machine.refuse(carrier, &mut hub.cx());
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)]
    );
    assert_eq!(machine.state(), &needs_login);
    assert_eq!(machine.next_wake(), None);

    // A question after `online` loses the link rather than leave `ssh` an empty answer.
    let carrier = started(&machine.connect(waiter(3), &mut hub.cx())).unwrap();
    reach_online(&mut machine, &mut hub, carrier);
    let effects = question(
        &mut machine,
        carrier,
        raised(QuestionKind::Password),
        &mut hub.cx(),
    );
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_ne!(machine.state(), &MachineState::Online);

    // A carrier the Machine no longer runs is ended again and its helper closed.
    let stale = carrier;
    let current = machine.carrier();
    let effects = machine.refuse(stale, &mut hub.cx());
    assert_eq!(
        effects,
        [
            Effect::End { carrier: stale },
            Effect::Close { carrier: stale }
        ]
    );
    assert_eq!(machine.carrier(), current);
}

#[test]
fn a_question_naming_another_host_is_other_and_never_marks_the_machine_untrusted() {
    let jump_host = new_host("jump.invalid (192.0.2.9)", &host_key());
    let server = format!(
        "(user@host.invalid) {}",
        prompt_of(&QuestionKind::HostKey {
            host_key: host_key()
        })
    );
    let password = prompt_of(&QuestionKind::Password);
    let claimed = || QuestionKind::HostKey {
        host_key: host_key(),
    };
    for prompt in [jump_host.as_str(), server.as_str(), password.as_str()] {
        // Nobody claims: the attempt ends needing a login, never untrusted with that key.
        let mut hub = Hub::new(false);
        let mut machine = machine();
        let carrier = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
        let effects = question(
            &mut machine,
            carrier,
            raised_as(claimed(), prompt),
            &mut hub.cx(),
        );
        assert_eq!(
            replies(&effects),
            [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)],
            "{prompt:?}"
        );
        assert_eq!(
            machine.state(),
            &MachineState::NeedsLogin {
                cause: Some(NeedsLoginCause::AuthRequired)
            }
        );
    }
    // A claimant is asked a jump host's questions as `other`.
    let jump_password = "user@jump.invalid's password: ";
    for (kind, prompt) in [
        (claimed(), jump_host.as_str()),
        (QuestionKind::Password, jump_password),
    ] {
        let mut hub = Hub::new(false);
        let mut machine = machine();
        let carrier = started(&machine.connect(asker(1, 1, Answers::User), &mut hub.cx())).unwrap();
        let effects = question(
            &mut machine,
            carrier,
            raised_as(kind, prompt),
            &mut hub.cx(),
        );
        let put = notified(&effects)
            .into_iter()
            .find_map(|(_, event)| match event {
                Event::Question(question) => Some(question.kind.clone()),
                _ => None,
            });
        assert_eq!(put, Some(QuestionKind::Other), "{prompt:?}");
    }
}
