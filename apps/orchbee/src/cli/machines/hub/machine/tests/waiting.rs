//! The login episode of commands that wait for a person, as cases: each plays its steps against one
//! Machine on the model's clock and checks, after every step, the events sent with their
//! connections, the replies, and how the episode and the Machine read. Commands are connections 1
//! to 3, interfaces that ask 7 and 8, and a person's own client 9. No step of any case deletes a
//! remembered answer.

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
    /// `ssh` asks for the password and the helper gives it the remembered one.
    Recalled,
    /// The helper took the answer.
    Taken,
    /// The helper left without an answer.
    Unanswered,
    /// `ssh` ended on a connection the server closed, as after its `LoginGraceTime`.
    Dropped,
    /// `ssh` authenticated.
    Authenticated,
    Claim(u64, ClaimOutcome),
    Dismiss(u64, DismissOutcome),
    /// The connection closed; it no longer asks.
    Close(u64),
    StopAsking(u64),
    StartAsking(u64),
    /// The hub entered `closing`.
    Closing,
    /// A reload changed the record's Session.
    Reload,
    Disconnect,
}

use Answers::{Command, Never, User};
use Step::{
    Askers, At, Authenticated, Claim, Close, Closing, Connect, Disconnect, Dismiss, Dropped,
    Question, Recalled, Reload, StartAsking, StopAsking, Taken, Unanswered, Wake,
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
    // The held question stops the attempt's clock; its bound and the waiter's both count from now.
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

const AN_OFFER_NOBODY_CLAIMS: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(60), &[], &[], "offered authenticating"),
    (
        Connect(2, 2, Never),
        &[(2, "waiting 2"), (7, "offered 2")],
        &[],
        "offered authenticating",
    ),
    // An unclaimed question's bound counts from when `ssh` asked it, whoever joined since.
    (Wake(Some(120)), &[], &[], "offered authenticating"),
    (At(119), &[], &[], "offered authenticating"),
    (
        At(120),
        &[(7, "ended abandoned 2")],
        &[(1, "auth-required"), (2, "auth-required")],
        "- needs-login:auth-required",
    ),
    (Wake(None), &[], &[], "- needs-login:auth-required"),
];

const A_LATE_CLAIM: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(100), &[], &[], "offered authenticating"),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 1"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    // A claimed login outlives its last waiter.
    (
        At(120),
        &[(7, "you 0")],
        &[(1, "auth-required")],
        "claimed:7 authenticating",
    ),
    // The claim gave the person the whole bound.
    (Wake(Some(220)), &[], &[], "claimed:7 authenticating"),
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

const THE_HELPER_LEAVES_AFTER_A_LATE_CLAIM: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (Connect(2, 2, Command), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (2, "waiting 2"), (7, "offered 2")],
        &[],
        "offered authenticating",
    ),
    (At(100), &[], &[], "offered authenticating"),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 2"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    (At(110), &[], &[], "claimed:7 authenticating"),
    (
        Unanswered,
        &[(7, "ended unavailable 2")],
        &[(1, "auth-required"), (2, "auth-required")],
        "- needs-login:auth-required",
    ),
];

const THE_HELPER_LEAVES_WHILE_OFFERED: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Unanswered,
        &[(7, "ended unavailable 1")],
        &[(1, "auth-required")],
        "- needs-login:auth-required",
    ),
];

const A_WAITER_AFTER_THE_CLAIM: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(100), &[], &[], "offered authenticating"),
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
    (
        At(120),
        &[(7, "other 0"), (9, "you 0")],
        &[(1, "auth-required")],
        "claimed:9 authenticating",
    ),
    (At(150), &[], &[], "claimed:9 authenticating"),
    (
        Connect(3, 3, Never),
        &[(3, "waiting 3"), (7, "other 1"), (9, "you 1")],
        &[],
        "claimed:9 authenticating",
    ),
    (Wake(Some(220)), &[], &[], "claimed:9 authenticating"),
    // The question's bound comes before the late waiter's: that command reads `auth-required`,
    // as nothing was submitted, and only the person's own request reads `timeout`.
    (
        At(220),
        &[(7, "ended unavailable 1"), (9, "ended unavailable 1")],
        &[(3, "auth-required"), (2, "timeout")],
        "- needs-login:auth-required",
    ),
];

const A_CLAIM_AFTER_A_CLAIMANT_LEFT: &[Row] = &[
    (Askers(&[7, 8]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1"), (8, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(50), &[], &[], "offered authenticating"),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 1"), (8, "other 1"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    (At(90), &[], &[], "claimed:7 authenticating"),
    (
        Connect(2, 2, Never),
        &[(2, "waiting 2"), (7, "you 2"), (8, "other 2")],
        &[],
        "claimed:7 authenticating",
    ),
    (At(100), &[], &[], "claimed:7 authenticating"),
    (Close(7), &[(8, "offered 2")], &[], "offered authenticating"),
    (
        At(120),
        &[(8, "offered 1")],
        &[(1, "auth-required")],
        "offered authenticating",
    ),
    // Offered again, the unanswered question keeps the bound the first claim gave it.
    (Wake(Some(170)), &[], &[], "offered authenticating"),
    (At(160), &[], &[], "offered authenticating"),
    (
        Claim(8, ClaimOutcome::Claimed),
        &[(8, "you 1"), (8, "question 1")],
        &[],
        "claimed:8 authenticating",
    ),
    (
        At(210),
        &[(8, "you 0")],
        &[(2, "auth-required")],
        "claimed:8 authenticating",
    ),
    // A second claim has the whole bound again, but never past twice it after `ssh` asked.
    (Wake(Some(240)), &[], &[], "claimed:8 authenticating"),
    (
        At(240),
        &[(8, "ended unavailable 0")],
        &[],
        "- needs-login:auth-required",
    ),
];

const AN_OFFER_A_CLAIMANT_LEFT: &[Row] = &[
    (Askers(&[7, 8]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1"), (8, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(50), &[], &[], "offered authenticating"),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 1"), (8, "other 1"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    (At(90), &[], &[], "claimed:7 authenticating"),
    (
        Connect(2, 2, Never),
        &[(2, "waiting 2"), (7, "you 2"), (8, "other 2")],
        &[],
        "claimed:7 authenticating",
    ),
    (
        StopAsking(7),
        &[(8, "offered 2")],
        &[],
        "offered authenticating",
    ),
    (
        At(120),
        &[(8, "offered 1")],
        &[(1, "auth-required")],
        "offered authenticating",
    ),
    (
        At(170),
        &[(8, "ended abandoned 1")],
        &[(2, "auth-required")],
        "- needs-login:auth-required",
    ),
];

const DISMISSED_AFTER_A_CLAIM: &[Row] = &[
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
        Claim(8, ClaimOutcome::Claimed),
        &[(7, "other 2"), (8, "you 2"), (8, "question 1")],
        &[],
        "claimed:8 authenticating",
    ),
    (
        Dismiss(7, DismissOutcome::NotClaimant),
        &[],
        &[],
        "claimed:8 authenticating",
    ),
    (
        Dismiss(8, DismissOutcome::Dismissed),
        &[(7, "ended dismissed 2"), (8, "ended dismissed 2")],
        &[(1, "auth-required"), (2, "auth-required")],
        "- needs-login:login-dismissed",
    ),
    (Connect(3, 3, Never), &[], &[], "- connecting"),
    (
        Question,
        &[],
        &[(3, "auth-required")],
        "- needs-login:login-dismissed",
    ),
];

const CLOSING_WHILE_OFFERED: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Closing,
        &[(7, "ended unavailable 1")],
        &[(1, "stopping")],
        "- authenticating",
    ),
];

const RELOADED_WHILE_OFFERED: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Reload,
        &[(7, "ended unavailable 1")],
        &[(1, "cancelled")],
        "- offline",
    ),
];

const DISCONNECTED_WHILE_OFFERED: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Disconnect,
        &[(7, "ended unavailable 1")],
        &[(1, "cancelled")],
        "- needs-login:disconnected",
    ),
];

const SSH_ENDS_WHILE_OFFERED: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (
        Dropped,
        &[(7, "ended unavailable 1")],
        &[(1, "ssh-failed")],
        "- offline:ssh-failed",
    ),
];

/// A late claim meets a server that stopped waiting: the answer the person typed is no refused
/// answer, nor is a remembered one.
const THE_SERVER_STOPPED_WAITING: &[Row] = &[
    (Askers(&[7]), &[], &[], "- offline"),
    (Connect(1, 1, Never), &[], &[], "- connecting"),
    (
        Question,
        &[(1, "waiting 1"), (7, "offered 1")],
        &[],
        "offered authenticating",
    ),
    (At(100), &[], &[], "offered authenticating"),
    (
        Claim(7, ClaimOutcome::Claimed),
        &[(7, "you 1"), (7, "question 1")],
        &[],
        "claimed:7 authenticating",
    ),
    (At(110), &[], &[], "claimed:7 authenticating"),
    (Taken, &[], &[], "claimed:7 authenticating"),
    (
        Dropped,
        &[(7, "ended unavailable 1")],
        &[(1, "ssh-failed")],
        "- offline:ssh-failed",
    ),
    (Connect(2, 2, Never), &[], &[], "- connecting"),
    (Recalled, &[], &[], "- connecting"),
    (Dropped, &[], &[(2, "ssh-failed")], "- offline:ssh-failed"),
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
    ("an offer nobody claims", AN_OFFER_NOBODY_CLAIMS),
    ("a late claim", A_LATE_CLAIM),
    (
        "the helper leaves after a late claim",
        THE_HELPER_LEAVES_AFTER_A_LATE_CLAIM,
    ),
    (
        "the helper leaves while offered",
        THE_HELPER_LEAVES_WHILE_OFFERED,
    ),
    ("a waiter after the claim", A_WAITER_AFTER_THE_CLAIM),
    (
        "a claim after a claimant left",
        A_CLAIM_AFTER_A_CLAIMANT_LEFT,
    ),
    ("an offer a claimant left", AN_OFFER_A_CLAIMANT_LEFT),
    ("dismissed after a claim", DISMISSED_AFTER_A_CLAIM),
    ("closing while offered", CLOSING_WHILE_OFFERED),
    ("reloaded while offered", RELOADED_WHILE_OFFERED),
    ("disconnected while offered", DISCONNECTED_WHILE_OFFERED),
    ("ssh ends while offered", SSH_ENDS_WHILE_OFFERED),
    ("the server stopped waiting", THE_SERVER_STOPPED_WAITING),
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
            assert!(
                !effects
                    .iter()
                    .any(|effect| matches!(effect, Effect::Forget { .. })),
                "{at}: forgot"
            );
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
        Recalled => {
            let carrier = carrier.unwrap();
            let effects =
                machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
            assert!(
                matches!(effects[..], [Effect::Recall { .. }]),
                "{effects:?}"
            );
            machine.recalled(carrier)
        }
        Taken => machine.taken(carrier.unwrap(), &hub.cx()),
        Unanswered => machine.unanswered(carrier.unwrap(), &hub.cx()),
        Dropped => {
            let end = End::Exited {
                code: Some(255),
                stderr: b"Connection closed by 192.0.2.1 port 22\r\n".to_vec(),
            };
            machine.ended(carrier.unwrap(), &end, &mut hub.cx())
        }
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
        Closing => {
            hub.closing = true;
            machine.closing()
        }
        Reload => machine.reconcile(ssh("other"), &mut hub.cx()),
        Disconnect => machine.disconnect(),
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
                ConnectOutcome::NotOnline(failure) => failure.as_str(),
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
        MachineState::Offline {
            cause: Some(cause), ..
        } => format!("offline:{}", cause.as_str()),
        other => other.name().as_str().to_owned(),
    };
    format!("{episode} {state}")
}
