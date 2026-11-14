//! Remembered answers: the first password or passphrase question of an attempt asks the helper to
//! read the store while the local `ssh` is recent enough, an answer `ssh` refuses is deleted and
//! never tried again in that attempt, a passphrase counts as refused only when asked in the same
//! words, and an answer an attempt handed over before it ended, or one whose deletion failed, is
//! not recalled again by the hub alone.

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

/// OpenSSH's end when the server closes the connection after too many tries: no refusal.
fn dropped() -> End {
    End::Exited {
        code: Some(255),
        stderr: b"Received disconnect from 192.0.2.1 port 22:2: Too many authentication failures"
            .to_vec(),
    }
}

/// Starts the retry the backoff scheduled and returns its carrier.
fn retried(machine: &mut Machine, hub: &mut Hub) -> u64 {
    let at = machine.next_wake().unwrap();
    started(&machine.tick(&mut hub.at(at - hub.start).cx())).unwrap()
}

#[test]
fn a_remembered_answer_is_tried_once_and_a_refused_one_is_deleted() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
    assert_eq!(
        asked(&effects),
        None,
        "nobody is asked while the store is read"
    );
    assert!(machine.recalled(carrier).is_empty());

    // `ssh` asking again refused the remembered answer.
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(forgets(&effects), [SecretKind::Password]);
    assert!(recalls(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(true));

    // The person's answer is refused too: no store read and nothing deleted.
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert!(recalls(&effects).is_empty() && forgets(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(false));
    assert_eq!(asked(&effects).map(|(_, repeated)| repeated), Some(true));
}

#[test]
fn nothing_remembered_poses_the_question_and_the_store_is_read_once_per_attempt() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
    let effects = machine.unrecalled(carrier, &mut hub.cx());
    assert_eq!(marked_rejected(&effects), Some(false));
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert!(recalls(&effects).is_empty());
    // A passphrase is its own kind and is read once as well.
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(
        carrier,
        raised(QuestionKind::KeyPassphrase),
        true,
        &mut hub.cx(),
    );
    assert_eq!(recalls(&effects), [SecretKind::KeyPassphrase]);
    // A new host is never remembered.
    let mut machine = super::machine();
    let carrier = started(&machine.connect(asker(4, 2, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(
        carrier,
        raised(QuestionKind::HostKey {
            host_key: host_key(),
        }),
        true,
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
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
    machine.recalled(carrier);
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(forgets(&effects), [SecretKind::Password]);
    assert!(ends_then_closes(&effects, carrier), "{effects:?}");
    assert_eq!(
        replies(&effects),
        [ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)]
    );
    assert_eq!(machine.state(), &rejected);

    // `ssh` gives up after the remembered answer.
    let mut machine = super::machine();
    let carrier = started(&machine.connect(waiter(2), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
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
    machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    machine.recalled(carrier);
    let effects = reach_online(&mut machine, &mut hub, carrier);
    assert!(forgets(&effects).is_empty());
    assert_eq!(machine.state(), &MachineState::Online);
}

#[test]
fn an_answer_handed_over_before_the_attempt_ended_waits_for_online_or_a_person() {
    // The server drops the connection after the remembered answer: no refusal, and nothing is
    // deleted, but the hub's own retry does not send the answer again.
    let mut hub = Hub::new(true);
    let mut machine = machine();
    let carrier = started(&machine.connect(waiter(1), &mut hub.cx())).unwrap();
    machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    machine.recalled(carrier);
    let effects = machine.ended(carrier, &dropped(), &mut hub.cx());
    assert!(forgets(&effects).is_empty());
    assert!(matches!(machine.state(), MachineState::Reconnecting { .. }));
    let carrier = retried(&mut machine, &mut hub);
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert!(recalls(&effects).is_empty(), "{effects:?}");
    assert!(ends(&effects, carrier), "nobody can answer the retry");

    // A person asking for the connection may have it tried once more.
    let carrier = started(&machine.connect(asker(4, 2, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);

    // An attempt that reaches `online` lets a later one recall it again.
    let mut machine = super::machine();
    let carrier = started(&machine.connect(waiter(3), &mut hub.cx())).unwrap();
    machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    machine.recalled(carrier);
    machine.ended(carrier, &dropped(), &mut hub.cx());
    let carrier = retried(&mut machine, &mut hub);
    reach_online(&mut machine, &mut hub, carrier);
    machine.ended(carrier, &refused(), &mut hub.cx());
    let carrier = retried(&mut machine, &mut hub);
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
}

#[test]
fn a_remembered_answer_whose_deletion_failed_waits_for_a_person_to_log_in() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    machine.recalled(carrier);
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(forgets(&effects), [SecretKind::Password]);
    machine.undeleted(SecretKind::Password);
    machine.disconnect();

    // Asking for the connection is not enough: the person logs in themselves.
    let carrier = started(&machine.connect(asker(4, 2, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert!(recalls(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(false));
    machine.taken(carrier, &hub.cx());
    reach_online(&mut machine, &mut hub, carrier);
    machine.disconnect();

    let carrier = started(&machine.connect(waiter(3), &mut hub.cx())).unwrap();
    let effects = machine.question(carrier, raised(QuestionKind::Password), true, &mut hub.cx());
    assert_eq!(recalls(&effects), [SecretKind::Password]);
}

#[test]
fn nothing_is_recalled_for_an_ssh_too_old_and_a_repeated_question_deletes_nothing() {
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(
        carrier,
        raised(QuestionKind::Password),
        false,
        &mut hub.cx(),
    );
    assert!(recalls(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(false));
    machine.taken(carrier, &hub.cx());
    let effects = machine.question(
        carrier,
        raised(QuestionKind::Password),
        false,
        &mut hub.cx(),
    );
    assert!(recalls(&effects).is_empty() && forgets(&effects).is_empty());
    assert_eq!(asked(&effects).map(|(_, repeated)| repeated), Some(true));
}

#[test]
fn a_passphrase_is_refused_only_when_asked_again_in_the_same_words() {
    let first = "Enter passphrase for key 'id_ed25519': ";
    let mut hub = Hub::new(false);
    let mut machine = machine();
    let carrier = started(&machine.connect(asker(4, 1, Answers::User), &mut hub.cx())).unwrap();
    let effects = machine.question(
        carrier,
        raised_as(QuestionKind::KeyPassphrase, first),
        true,
        &mut hub.cx(),
    );
    assert_eq!(recalls(&effects), [SecretKind::KeyPassphrase]);
    machine.recalled(carrier);

    // Another key asks in its own words: asked as any question, nothing deleted.
    let effects = machine.question(
        carrier,
        raised_as(
            QuestionKind::KeyPassphrase,
            "Enter passphrase for key 'id_rsa': ",
        ),
        true,
        &mut hub.cx(),
    );
    assert!(forgets(&effects).is_empty() && recalls(&effects).is_empty());
    assert_eq!(marked_rejected(&effects), Some(false));
    machine.taken(carrier, &hub.cx());

    // The first key asking again refused its remembered passphrase.
    let effects = machine.question(
        carrier,
        raised_as(QuestionKind::KeyPassphrase, first),
        true,
        &mut hub.cx(),
    );
    assert_eq!(forgets(&effects), [SecretKind::KeyPassphrase]);
    assert_eq!(marked_rejected(&effects), Some(true));

    // A server that refuses the key after its passphrase was given deletes no passphrase.
    let mut machine = super::machine();
    let carrier = started(&machine.connect(waiter(2), &mut hub.cx())).unwrap();
    machine.question(
        carrier,
        raised_as(QuestionKind::KeyPassphrase, first),
        true,
        &mut hub.cx(),
    );
    machine.recalled(carrier);
    let effects = machine.ended(
        carrier,
        &End::Exited {
            code: Some(255),
            stderr: b"user@host.invalid: Permission denied (publickey).".to_vec(),
        },
        &mut hub.cx(),
    );
    assert!(forgets(&effects).is_empty(), "{effects:?}");
    assert_eq!(
        machine.state(),
        &MachineState::LoginFailed {
            cause: Some(LoginFailedCause::KeyRejected)
        }
    );
}
