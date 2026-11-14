//! Remembered answers through real processes: the askpass helper reads the store of this test
//! build's environment, which the tests write with test words only, under Machine ids no other test
//! of this binary uses. A silent attempt with the right remembered answer reaches `online` without
//! a question; a remembered answer the fake `ssh` refuses is tried once, deleted, and followed by
//! the claimant's question marked `remembered: rejected`; with nobody to ask the attempt ends
//! `remembered-rejected`; removing the Machine deletes its items. A server that drops the
//! connection after the remembered answer receives it once however often the hub retries; an `ssh`
//! older than OpenSSH 8.5 is never given one; a second key's passphrase question deletes nothing.
//! The store items are deleted on every path.

use super::*;
use orchbee_workbench_client::application::machine_credentials::{
    Item, SecretKind, Store, readable_label,
};

/// This module's Machines, one per test: the store is shared by the tests running at once, so no
/// other test may name them.
const MINE: &str = "2e4e4be4e4be4e4be4e4be4e4be4e4b2";
const DROPPED: &str = "2e4e4be4e4be4e4be4e4be4e4be4e4b3";
const OLD_SSH: &str = "2e4e4be4e4be4e4be4e4be4e4be4e4b4";
const TWO_KEYS: &str = "2e4e4be4e4be4e4be4e4be4e4be4e4b5";

/// One kind's item of a Machine of this module, deleted, with the other kind's item, before and
/// after the test.
struct Remembered(Item, &'static str);

impl Remembered {
    fn new() -> Self {
        Self::of(MINE, SecretKind::Password)
    }

    fn of(id: &'static str, kind: SecretKind) -> Self {
        let remembered = Self(Item::new(machine(id), kind), id);
        remembered.clear();
        remembered
    }

    fn write(&self, word: &str) {
        Store::select()
            .write(&self.0, word.as_bytes(), &readable_label("test machine"))
            .unwrap();
    }

    fn held(&self) -> bool {
        Store::select().read(&self.0).unwrap().is_some()
    }

    /// Waits until the hub's deletion has run.
    fn gone(&self) -> bool {
        let deadline = Instant::now() + WAIT;
        while Instant::now() < deadline {
            if !self.held() {
                return true;
            }
            thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn clear(&self) {
        let _forgotten = Store::select().forget(&machine(self.1));
    }
}

impl Drop for Remembered {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Makes `MINE` the fixture's only Machine, for the hub that starts next.
fn adopt(carried: &Carried) {
    adopt_as(carried, MINE);
}

/// Makes `id` the fixture's only Machine, for the hub that starts next.
fn adopt_as(carried: &Carried, id: &str) {
    exit_hub(&carried.endpoint());
    let document = json!({"version": 1, "revision": 2, "profiles": [{
        "id": id, "label": "Mine",
        "connection": {"kind": "ssh", "target": "user@host.invalid"},
        "session": "far", "enabled": true, "remoteInstall": "manual"}]});
    PrivateCache::acquire(&carried.root.state(), "machines")
        .unwrap()
        .replace("machines.json", &serde_json::to_vec(&document).unwrap())
        .unwrap();
}

/// A client that holds the hub, so it attempts the kept Machine by itself, and watches it until
/// its state reads as `until` says.
fn keep_until(carried: &Carried, until: impl Fn(&MachineState) -> bool) -> (Client, MachineState) {
    keep_until_as(carried, MINE, until)
}

/// [`keep_until`] for the Machine `id`.
fn keep_until_as(
    carried: &Carried,
    id: &str,
    until: impl Fn(&MachineState) -> bool,
) -> (Client, MachineState) {
    let mut keeper = Client::connect(&carried.endpoint());
    attach(&mut keeper, true, true, true);
    let deadline = Instant::now() + WAIT;
    loop {
        let state = keeper.machine_state(deadline.saturating_duration_since(Instant::now()), id);
        if until(&state) {
            return (keeper, state);
        }
    }
}

/// How many answers the fake `ssh` received, right or wrong.
fn received(transcript: &str) -> usize {
    transcript
        .lines()
        .filter(|line| line.starts_with("right ") || line.starts_with("wrong "))
        .count()
}

#[test]
fn a_dropped_connection_after_a_remembered_answer_is_not_answered_again_by_the_hub() {
    let mut dropping = step(PASSWORD, RIGHT_PASSWORD, 1, false);
    dropping["drop"] = json!(true);
    let (carried, fake) = Carried::faking("mq1-dropped-", &json!({"questions": [dropping]}), &[]);
    let remembered = Remembered::of(DROPPED, SecretKind::Password);
    adopt_as(&carried, DROPPED);
    remembered.write(WRONG);
    carried.state();
    // The server closes the connection after the remembered answer, which reads as no refusal;
    // the hub retries by itself, asks nobody on that retry, and the Machine needs a login.
    let (mut keeper, state) = keep_until_as(&carried, DROPPED, |state| {
        state.name() == MachineStateName::NeedsLogin
    });
    assert_eq!(
        state,
        MachineState::NeedsLogin {
            cause: Some(NeedsLoginCause::AuthRequired)
        }
    );
    assert!(no_question(&mut keeper));
    let transcript = fake.transcript();
    assert_eq!(transcript.matches("started").count(), 2, "{transcript}");
    assert_eq!(received(&transcript), 1, "{transcript}");
    assert!(remembered.held(), "no refusal deletes nothing");
    drop(keeper);
    let printed = carried
        .command(&["--machine-hub-state-v1"])
        .output()
        .unwrap();
    search(&carried, &[&printed.stdout, &printed.stderr]);
}

#[test]
fn an_ssh_older_than_8_5_is_never_given_a_remembered_answer() {
    let (carried, fake) = Carried::faking(
        "mq1-oldssh-",
        &json!({
            "version": "OpenSSH_for_Windows_8.1p1, LibreSSL 3.0.2",
            "questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)],
        }),
        &[],
    );
    let remembered = Remembered::of(OLD_SSH, SecretKind::Password);
    adopt_as(&carried, OLD_SSH);
    remembered.write(RIGHT_PASSWORD);
    carried.state();
    // A silent attempt meets the password question and asks nobody, with the answer remembered.
    let (mut keeper, state) = keep_until_as(&carried, OLD_SSH, |state| {
        state.name() == MachineStateName::NeedsLogin
    });
    assert_eq!(
        state,
        MachineState::NeedsLogin {
            cause: Some(NeedsLoginCause::AuthRequired)
        }
    );
    assert!(no_question(&mut keeper));
    let transcript = fake.transcript();
    assert_eq!(received(&transcript), 0, "{transcript}");
    assert!(remembered.held());
    drop(keeper);

    // A person's login on that `ssh` works as without remembering.
    let mut client = claimant(&carried);
    let id = client.send(Request::Connect(Connect {
        machine: machine(OLD_SSH),
        origin: Origin::User,
        draft: None,
    }));
    let asked = question(&mut client);
    assert!(!asked.remembered_rejected);
    deliver(&asked.answer, RIGHT_PASSWORD.as_bytes()).unwrap();
    let result = connect_reply(&mut client, id);
    assert!(
        matches!(result.outcome, ConnectOutcome::Online { .. }),
        "{result:?}"
    );
    drop(client);
    let printed = carried
        .command(&["--machine-hub-state-v1"])
        .output()
        .unwrap();
    search(&carried, &[&printed.stdout, &printed.stderr]);
}

#[test]
fn a_second_key_asking_for_its_passphrase_deletes_nothing() {
    const SECOND: &str = "Enter passphrase for key 'C:\\Users\\someone/.ssh/id_rsa': ";
    let (carried, fake) = Carried::faking(
        "mq1-twokeys-",
        &json!({"questions": [
            step(PASSPHRASE, RIGHT_PASSPHRASE, 1, false),
            step(SECOND, RIGHT_CODE, 1, false),
        ]}),
        &[],
    );
    let remembered = Remembered::of(TWO_KEYS, SecretKind::KeyPassphrase);
    adopt_as(&carried, TWO_KEYS);
    remembered.write(RIGHT_PASSPHRASE);
    carried.state();
    let mut client = claimant(&carried);
    let id = client.send(Request::Connect(Connect {
        machine: machine(TWO_KEYS),
        origin: Origin::User,
        draft: None,
    }));
    // The first key's passphrase came from the store; the second key is asked as any question.
    let asked = question(&mut client);
    assert_eq!(asked.kind, QuestionKind::KeyPassphrase);
    assert!(!asked.remembered_rejected, "{asked:?}");
    assert_eq!(asked.prompt.as_str(), SECOND);
    deliver(&asked.answer, RIGHT_CODE.as_bytes()).unwrap();
    let result = connect_reply(&mut client, id);
    assert!(
        matches!(result.outcome, ConnectOutcome::Online { .. }),
        "{result:?}"
    );
    assert_eq!(
        fake.transcript(),
        "started\nasked 1\nright 1\nasked 2\nright 2\nlinked\n"
    );
    assert!(remembered.held(), "the first key's passphrase stays");
    drop(client);
    let printed = carried
        .command(&["--machine-hub-state-v1"])
        .output()
        .unwrap();
    search(&carried, &[&printed.stdout, &printed.stderr]);
}

#[test]
fn a_remembered_password_is_tried_once_per_attempt_and_a_refused_one_is_deleted() {
    let (carried, fake) = Carried::faking(
        "mq1-remember-",
        &json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)]}),
        &[],
    );
    let remembered = Remembered::new();
    adopt(&carried);

    // A silent attempt finds the right answer remembered: `online`, and nobody is asked.
    remembered.write(RIGHT_PASSWORD);
    carried.state();
    let (mut keeper, _) = keep_until(&carried, |state| *state == MachineState::Online);
    assert!(no_question(&mut keeper));
    assert_eq!(
        fake.transcript(),
        "started\nasked 1\nright 1\nlinked\n",
        "one remembered try"
    );
    assert!(remembered.held(), "an answer that worked stays");
    drop(keeper);
    exit_hub(&carried.endpoint());

    // The password changed: the remembered one is tried once, deleted, and the claimant is asked.
    fs::write(&fake.transcript, "").unwrap();
    fake.plan(&json!({"questions": [step(PASSWORD, RIGHT_CODE, 2, false)]}));
    carried.state();
    let mut client = claimant(&carried);
    let id = client.send(Request::Connect(Connect {
        machine: machine(MINE),
        origin: Origin::User,
        draft: None,
    }));
    let asked = question(&mut client);
    assert!(asked.remembered_rejected, "{asked:?}");
    assert!(remembered.gone(), "a refused remembered answer is deleted");
    deliver(&asked.answer, RIGHT_CODE.as_bytes()).unwrap();
    let result = connect_reply(&mut client, id);
    assert!(
        matches!(result.outcome, ConnectOutcome::Online { .. }),
        "{result:?}"
    );
    assert_eq!(
        fake.transcript(),
        "started\nasked 1\nwrong 1\nasked 2\nright 2\nlinked\n"
    );
    drop(client);
    exit_hub(&carried.endpoint());

    // With nobody to ask, a refused remembered answer ends the attempt as `remembered-rejected`.
    fs::write(&fake.transcript, "").unwrap();
    fake.plan(&json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)]}));
    remembered.write(WRONG);
    carried.state();
    let (mut keeper, state) = keep_until(&carried, |state| {
        state.name() == MachineStateName::NeedsLogin
    });
    assert_eq!(
        state,
        MachineState::NeedsLogin {
            cause: Some(NeedsLoginCause::RememberedRejected)
        }
    );
    assert!(no_question(&mut keeper));
    assert!(remembered.gone());
    let transcript = fake.transcript();
    assert_eq!(transcript.matches("asked").count(), 1, "{transcript}");
    drop(keeper);

    // Removing the Machine forgets what was remembered for it.
    remembered.write(RIGHT_PASSWORD);
    let removed = carried.run(&["machine", "remove", MINE]);
    assert_eq!(removed.code, Some(0), "{removed}");
    assert!(!remembered.held(), "a removed Machine's answer stays");

    let printed = carried
        .command(&["--machine-hub-state-v1"])
        .output()
        .unwrap();
    search(&carried, &[&printed.stdout, &printed.stderr]);
}
