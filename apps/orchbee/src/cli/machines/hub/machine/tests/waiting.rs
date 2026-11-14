//! The login episode of commands that wait for a person, as cases: each plays its steps against one
//! Machine on the model's clock and checks, after every step, the events sent with their
//! connections, the replies, and how the episode and the Machine read. Commands are connections 1
//! to 3, interfaces that ask 7 and 8, and a person's own client 9.

use super::*;

#[derive(Clone, Copy, Debug)]
enum Step {
    /// The connections that ask from now on, without telling the Machine.
    Askers(&'static [u64]),
    /// The clock reads this many seconds after the start; due transitions run.
    At(u64),
    /// The Machine's next wake, in seconds after the start.
    Wake(Option<u64>),
    Connect(u64, u32, Answers),
    /// `ssh` asks for the password.
    Question,
    /// The helper took the answer.
    Taken,
    /// `ssh` authenticated.
    Authenticated,
    Claim(u64, ClaimOutcome),
    Dismiss(u64, DismissOutcome),
    /// The connection closed; it no longer asks.
    Close(u64),
    StopAsking(u64),
    StartAsking(u64),
}

use Answers::{Command, Never, User};
use Step::{
    Askers, At, Authenticated, Claim, Close, Connect, Dismiss, Question, StartAsking, StopAsking,
    Taken, Wake,
};

/// One step and what follows it: the events sent, the replies, and the episode with the Machine's
/// state.
type Row = (
    Step,
    &'static [(u64, &'static str)],
    &'static [(u32, &'static str)],
    &'static str,
);

const OFFERED_TO_ALL: &[Row] = &[
    (Askers(&[7, 8]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1"), (8, "offered 1")],
        &[],
        "offered authenticating",
    ),
    // The held question has no bound and stops the attempt's clock: only the waiter's runs.
    (Wake(Some(120)), &[], &[], "offered authenticating"),
    (
        Claim(8, ClaimOutcome::Claimed),
        &[(7, "other 1"), (8, "you 1"), (8, "question 1")],
        &[],
        "claimed:8 authenticating",
    ),
    (
        Claim(7, ClaimOutcome::Taken),
        &[],
        &[],
        "claimed:8 authenticating",
    ),
    (
        Claim(8, ClaimOutcome::Claimed),
        &[],
        &[],
        "claimed:8 authenticating",
    ),
    (Wake(Some(120)), &[], &[], "claimed:8 authenticating"),
    (
        Dismiss(7, DismissOutcome::NotClaimant),
        &[],
        &[],
        "claimed:8 authenticating",
    ),
    (Taken, &[], &[], "claimed:8 authenticating"),
    (
        Authenticated,
        &[(7, "ended succeeded 1"), (8, "ended succeeded 1")],
        &[],
        "- starting",
    ),
    // A login that succeeded waits for `online` under the attempt's clock alone.
    (Wake(Some(60)), &[], &[], "- starting"),
];

const ONE_NOTICE_FOR_SEVERAL: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Connect(2, 2, Command),
        &[(2, "waiting 2"), (7, "offered 2")],
        &[],
        "offered authenticating",
    ),
    (Close(1), &[(7, "offered 1")], &[], "offered authenticating"),
    // Untouched, the notice closes with its last command.
    (
        Close(2),
        &[(7, "ended abandoned 0")],
        &[],
        "- needs-login:auth-required",
    ),
];

const DISMISSED_AND_MUTED: &[Row] = &[
    (Askers(&[7, 8]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (Connect(2, 2, Command), &[], &[], "- connecting"),
    (
        Question,
        &[
            (1, "waiting 1"),
            (2, "waiting 2"),
            (7, "offered 2"),
            (8, "offered 2"),
        ],
        &[],
        "offered authenticating",
    ),
    (
        Dismiss(1, DismissOutcome::NotClaimant),
        &[],
        &[],
        "offered authenticating",
    ),
    (
        Dismiss(8, DismissOutcome::Dismissed),
        &[(7, "ended dismissed 2"), (8, "ended dismissed 2")],
        &[(1, "auth-required"), (2, "auth-required")],
        "- needs-login:login-dismissed",
    ),
    // Muted: a command that cannot answer fails at once, and nobody is shown anything.
    (Connect(3, 3, Never), &[], &[], "- connecting"),
    (
        Question,
        &[],
        &[(3, "auth-required")],
        "- needs-login:login-dismissed",
    ),
    // One that answers itself is asked, as a person's request is.
    (Connect(2, 4, Command), &[], &[], "- connecting"),
    (
        Question,
        &[(2, "waiting 4"), (2, "user you 0"), (2, "question 1")],
        &[],
        "claimed:2 authenticating",
    ),
    (
        Dismiss(2, DismissOutcome::Dismissed),
        &[(2, "user ended dismissed 0")],
        &[(4, "auth-required")],
        "- needs-login:login-dismissed",
    ),
    // A person's own connect lifts the mute.
    (Connect(9, 5, User), &[], &[], "- connecting"),
    (
        Question,
        &[(9, "waiting 5"), (9, "user you 0"), (9, "question 1")],
        &[],
        "claimed:9 authenticating",
    ),
];

const MUTE_ENDS_WITH_THE_LAST_INTERFACE: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Dismiss(7, DismissOutcome::Dismissed),
        &[(7, "ended dismissed 1")],
        &[(1, "auth-required")],
        "- needs-login:login-dismissed",
    ),
    (StopAsking(7), &[], &[], "- needs-login:auth-required"),
    (StartAsking(7), &[], &[], "- needs-login:auth-required"),
    (Connect(1, 2, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 2"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
];

const EACH_WAITER_HAS_ITS_BOUND: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(50), &[], &[], "offered authenticating"),
    (
        Connect(2, 2, Never),
        &[(2, "waiting 2"), (7, "offered 2")],
        &[],
        "offered authenticating",
    ),
    (At(119), &[], &[], "offered authenticating"),
    (
        At(120),
        &[(7, "offered 1")],
        &[(1, "auth-required")],
        "offered authenticating",
    ),
    (Wake(Some(170)), &[], &[], "offered authenticating"),
    (At(160), &[], &[], "offered authenticating"),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 1"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    (Wake(Some(170)), &[], &[], "claimed:7 authenticating"),
    // A claimed login outlives its last waiter and completes.
    (
        At(170),
        &[(7, "you 0")],
        &[(2, "auth-required")],
        "claimed:7 authenticating",
    ),
    // The question's bound started at the claim.
    (Wake(Some(280)), &[], &[], "claimed:7 authenticating"),
    (At(200), &[], &[], "claimed:7 authenticating"),
    // The attempt's 60 seconds stood still for the 200 seconds the question was open.
    (Taken, &[], &[], "claimed:7 authenticating"),
    (Wake(Some(260)), &[], &[], "claimed:7 authenticating"),
    (
        Authenticated,
        &[(7, "ended succeeded 0")],
        &[],
        "- starting",
    ),
];

const A_CLAIMANT_THAT_LEAVES: &[Row] = &[
    (Askers(&[7, 8]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1"), (8, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 1"), (8, "other 1"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    (Taken, &[], &[], "claimed:7 authenticating"),
    (Close(7), &[(8, "offered 1")], &[], "offered authenticating"),
    // `ssh` asks again while the login is offered: the question is held for the next claimant.
    (Question, &[], &[], "offered authenticating"),
    (Wake(Some(120)), &[], &[], "offered authenticating"),
    (
        Claim(8, ClaimOutcome::Claimed),
        &[(8, "you 1"), (8, "question 2")],
        &[],
        "claimed:8 authenticating",
    ),
    // With nobody else to ask, the claimant leaving ends the login.
    (
        StopAsking(8),
        &[(8, "ended abandoned 1")],
        &[(1, "auth-required")],
        "- needs-login:auth-required",
    ),
];

const A_PERSONS_CONNECT_CLAIMS: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Connect(9, 2, User),
        &[
            (9, "waiting 2"),
            (7, "other 1"),
            (9, "you 1"),
            (9, "question 1"),
        ],
        &[],
        "claimed:9 authenticating",
    ),
    (Close(9), &[(7, "offered 1")], &[], "offered authenticating"),
];

const A_LATE_INTERFACE: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        StartAsking(8),
        &[(8, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (StartAsking(8), &[], &[], "offered authenticating"),
    (
        Claim(8, ClaimOutcome::Claimed),
        &[(7, "other 1"), (8, "you 1"), (8, "question 1")],
        &[],
        "claimed:8 authenticating",
    ),
    // Another interface that stops asking leaves a claimed login as it is.
    (StopAsking(7), &[], &[], "claimed:8 authenticating"),
];

const NOBODY_ASKS: &[Row] = &[
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (Connect(2, 2, Command), &[], &[], "- connecting"),
    (
        Question,
        &[
            (1, "waiting 1"),
            (2, "waiting 2"),
            (2, "user you 1"),
            (2, "question 1"),
        ],
        &[],
        "claimed:2 authenticating",
    ),
    (
        Close(2),
        &[(2, "user ended abandoned 1")],
        &[(1, "auth-required")],
        "- needs-login:auth-required",
    ),
];

const CASES: &[(&str, &[Row])] = &[
    ("offered to every interface", OFFERED_TO_ALL),
    ("one notice for several commands", ONE_NOTICE_FOR_SEVERAL),
    ("dismissed and muted", DISMISSED_AND_MUTED),
    (
        "mute ends with the last interface",
        MUTE_ENDS_WITH_THE_LAST_INTERFACE,
    ),
    ("each waiter has its bound", EACH_WAITER_HAS_ITS_BOUND),
    ("a claimant that leaves", A_CLAIMANT_THAT_LEAVES),
    ("a person's connect claims", A_PERSONS_CONNECT_CLAIMS),
    ("a late interface", A_LATE_INTERFACE),
    ("nobody asks", NOBODY_ASKS),
];

#[test]
fn commands_wait_for_a_person_at_an_interface_as_the_cases_play() {
    for (name, rows) in CASES {
        let mut hub = Hub::new(false);
        let mut machine = machine();
        let mut carrier = None;
        for (index, (step, sent, replied, then)) in rows.iter().enumerate() {
            let effects = play(&mut machine, &mut hub, &mut carrier, *step);
            let at = format!("{name}, step {index} {step:?}");
            assert_eq!(events(&effects), *sent, "{at}: sent");
            assert_eq!(outcomes(&effects), *replied, "{at}: replied");
            assert_eq!(read(&machine), *then, "{at}: then");
        }
    }
}

fn play(
    machine: &mut Machine,
    hub: &mut Hub,
    carrier: &mut Option<u64>,
    step: Step,
) -> Vec<Effect> {
    let id = |request: u32| Id::new(request).unwrap();
    match step {
        Askers(askers) => {
            hub.askers = askers.to_vec();
            Vec::new()
        }
        At(seconds) => machine.tick(&mut hub.at(Duration::from_secs(seconds)).cx()),
        Wake(seconds) => {
            let wake = seconds.map(|seconds| hub.start + Duration::from_secs(seconds));
            assert_eq!(machine.next_wake(), wake, "wake");
            Vec::new()
        }
        Connect(peer, request, answers) => {
            let effects = machine.connect(Waiter::new(peer, id(request), answers), &mut hub.cx());
            if let Some(started) = started(&effects) {
                *carrier = Some(started);
            }
            effects
        }
        Question => question(
            machine,
            carrier.unwrap(),
            raised(QuestionKind::Password),
            &mut hub.cx(),
        ),
        Taken => machine.taken(carrier.unwrap(), &hub.cx()),
        Authenticated => machine.answered(carrier.unwrap(), ProductVersion::new("1.0.0").unwrap()),
        Claim(peer, outcome) => {
            let login = machine.login().unwrap();
            let (claimed, effects) = machine.claim(peer, login, &hub.cx());
            assert_eq!(claimed, outcome, "claim");
            effects
        }
        Dismiss(peer, outcome) => {
            let login = machine.login().unwrap();
            let (dismissed, effects) = machine.dismiss(peer, login, &hub.cx());
            assert_eq!(dismissed, outcome, "dismiss");
            effects
        }
        Close(peer) => {
            hub.askers.retain(|asker| *asker != peer);
            machine.forget(peer, &hub.cx())
        }
        StopAsking(peer) => {
            hub.askers.retain(|asker| *asker != peer);
            machine.stopped_asking(peer, &hub.cx())
        }
        StartAsking(peer) => {
            if !hub.askers.contains(&peer) {
                hub.askers.push(peer);
            }
            machine.asking(peer)
        }
    }
}

/// The events `effects` send, each with its connection, in order.
fn events(effects: &[Effect]) -> Vec<(u64, &'static str)> {
    notified(effects)
        .into_iter()
        .map(|(peer, event)| {
            let named = match event {
                Event::ConnectWaiting(waiting) => format!("waiting {}", waiting.request.get()),
                Event::Login(login) => {
                    let phase = match login.phase {
                        LoginPhase::Offered => "offered".to_owned(),
                        LoginPhase::Claimed {
                            claim: Claimant::You,
                        } => "you".to_owned(),
                        LoginPhase::Claimed {
                            claim: Claimant::Other,
                        } => "other".to_owned(),
                        LoginPhase::Ended { end } => format!("ended {}", end.as_str()),
                    };
                    let trigger = match login.trigger {
                        Trigger::User => "user ",
                        Trigger::Command => "",
                    };
                    format!("{trigger}{phase} {}", login.waiters.get())
                }
                Event::Question(question) => format!("question {}", question.question.get()),
                other => format!("{other:?}"),
            };
            (peer, &*named.leak())
        })
        .collect()
}

/// The replies `effects` give, each with its request.
fn outcomes(effects: &[Effect]) -> Vec<(u32, &'static str)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Reply { waiters, result } => Some((waiters, &result.outcome)),
            _ => None,
        })
        .flat_map(|(waiters, outcome)| {
            let named = match outcome {
                ConnectOutcome::Online { .. } => "online",
                ConnectOutcome::NotOnline(ConnectFailure::AuthRequired) => "auth-required",
                ConnectOutcome::NotOnline(ConnectFailure::Timeout) => "timeout",
                ConnectOutcome::NotOnline(_) => "other",
            };
            waiters
                .iter()
                .map(move |waiter| (waiter.request.get(), named))
        })
        .collect()
}

/// The episode, `-` for none, and the Machine's state.
fn read(machine: &Machine) -> String {
    let episode = match machine.episode().map(Episode::claimant) {
        None => "-".to_owned(),
        Some(None) => "offered".to_owned(),
        Some(Some(holder)) => format!("claimed:{}", holder.peer),
    };
    let state = match machine.state() {
        MachineState::Connecting { stage: None } => "connecting".to_owned(),
        MachineState::Connecting { stage: Some(stage) } => stage.as_str().to_owned(),
        MachineState::NeedsLogin { cause: Some(cause) } => {
            format!("needs-login:{}", cause.as_str())
        }
        other => other.name().as_str().to_owned(),
    };
    format!("{episode} {state}")
}
