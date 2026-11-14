//! Remembered answers in one attempt: the first password or passphrase question asks the helper to
//! read the store, an answer `ssh` refuses is deleted and never tried again in that attempt, and
//! every later attempt reads the store once more.

use super::*;

fn id() -> MachineId {
    MachineId::new("11111111111111111111111111111111").unwrap()
}

fn recalls(effects: &[Effect]) -> Vec<SecretKind> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Recall { kind, machine, .. } if *machine == id() => Some(*kind),
            _ => None,
        })
        .collect()
}

fn forgets(effects: &[Effect]) -> Vec<SecretKind> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Forget { kind, machine } if *machine == id() => Some(*kind),
            _ => None,
        })
        .collect()
}

/// Whether the question `effects` put to the claimant says the remembered answer was refused.
fn marked_rejected(effects: &[Effect]) -> Option<bool> {
    notified(effects)
        .into_iter()
        .find_map(|(_, event)| match event {
            Event::Question(question) => Some(question.remembered_rejected),
            _ => None,
        })
}

#[test]
fn a_remembered_answer_is_tried_once_and_a_refused_one_is_deleted() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
    assert_eq!(
        asked(&effects),
        None,
        "nobody is asked while the store is read"
    );
    assert!(machine.recalled(carrier).is_empty());

    // `ssh` asking again refused the remembered answer.
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert_eq!(forgets(&effects), [SecretKind::Password]);
    assert!(recalls(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(true));

    // The person's answer is refused too: no store read and nothing deleted.
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert!(recalls(&effects).is_empty() && forgets(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(false));
    assert_eq!(asked(&effects).map(|(_, repeated)| repeated), Some(true));
}

#[test]
fn nothing_remembered_poses_the_question_and_the_store_is_read_once_per_attempt() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
    let effects = machine.unrecalled(carrier, &mut hub.cx());
    assert_eq!(marked_rejected(&effects), Some(false));
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert!(recalls(&effects).is_empty());
    // A passphrase is its own kind and is read once as well.
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(carrier, raised(QuestionKind::KeyPassphrase), &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::KeyPassphrase]);
    // A new host is never remembered.
    let mut machine = super::machine();
    let carrier = started(&machine.connect(asker(4, 2, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(
        carrier,
        raised(QuestionKind::HostKey {
            host_key: host_key(),
        }),
        &mut hub.cx(),
    );
    assert!(recalls(&effects).is_empty());
}

#[test]
fn a_refused_remembered_answer_with_nobody_to_ask_ends_in_remembered_rejected() {
    let rejected = MachineState::NeedsLogin {
        cause: Some(NeedsLoginCause::RememberedRejected),
    };
    // `ssh` asks again and nobody can answer.
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
    machine.recalled(carrier);
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert_eq!(forgets(&effects), [SecretKind::Password]);
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)]
    );
    assert_eq!(machine.state(), &rejected);

    // `ssh` gives up after the remembered answer.
    let carrier = started(&machine.connect(waiter(2), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    assert_eq!(
        recalls(&effects),
        [SecretKind::Password],
        "a new attempt reads again"
    );
    machine.recalled(carrier);
    let effects = machine.ended(
        carrier,
        &End::Exited {
            code: Some(255),
            stderr: b"user@host.invalid: Permission denied (publickey,password).".to_vec(),
        },
        &mut hub.cx(),
    );
    assert_eq!(forgets(&effects), [SecretKind::Password]);
    assert_eq!(machine.state(), &rejected);
}

#[test]
fn a_remembered_answer_that_reaches_online_is_kept() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    machine.question(carrier, raised(QuestionKind::Password), &mut hub.cx());
    machine.recalled(carrier);
    let effects = reach_online(&mut machine, &mut hub, carrier);
    assert!(forgets(&effects).is_empty());
    assert_eq!(machine.state(), &MachineState::Online);
}
