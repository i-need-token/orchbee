use super::*;
use orchbee_connection_handshake::RejectionReason;

/// The model's clock and the hub's carrier numbers, advanced by hand.
struct Hub {
    start: Instant,
    after: Duration,
    carriers: u64,
    presence: bool,
    closing: bool,
}

impl Hub {
    fn new(presence: bool) -> Self {
        Self {
            start: Instant::now(),
            after: Duration::ZERO,
            carriers: 0,
            presence,
            closing: false,
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
            carriers: &mut self.carriers,
        }
    }
}

fn machine() -> Machine {
    Machine::new(
        MachineId::new("11111111111111111111111111111111").unwrap(),
        Kind::Ssh(Route {
            target: "user@host.invalid".to_owned(),
            session: SessionName::new("far").unwrap(),
        }),
    )
}

fn waiter(request: u32) -> Waiter {
    Waiter {
        peer: 1,
        request: Id::new(request).unwrap(),
    }
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
            Failure::KeyChanged,
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
