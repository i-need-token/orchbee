//! Remembered answers through real processes: the askpass helper reads the store of this test
//! build's environment, which the test writes with test words only, under a Machine id no other
//! test of this binary uses. A silent attempt with the right remembered answer reaches `online`
//! without a question; a remembered answer the fake `ssh` refuses is tried once, deleted, and
//! followed by the claimant's question marked `remembered: rejected`; with nobody to ask the
//! attempt ends `remembered-rejected`; removing the Machine deletes its items. The store items are
//! deleted on every path.

use super::*;
use orchbee_workbench_client::application::machine_credentials::{
    Item, SecretKind, Store, readable_label,
};

/// This module's Machine: the store is shared by the tests running at once, so no other test may
/// name it.
const MINE: &str = "2e4e4be4e4be4e4be4e4be4e4be4e4b2";

/// The password item of `MINE`, deleted, with the passphrase item, before and after the test.
struct Remembered(Item);

impl Remembered {
    fn new() -> Self {
        let remembered = Self(Item::new(machine(MINE), SecretKind::Password));
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
        let _forgotten = Store::select().forget(&machine(MINE));
    }
}

impl Drop for Remembered {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Makes `MINE` the fixture's only Machine, for the hub that starts next.
fn adopt(carried: &Carried) {
    exit_hub(&carried.endpoint());
    let document = json!({"version": 1, "revision": 2, "profiles": [{
        "id": MINE, "label": "Mine",
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
    let mut keeper = Client::connect(&carried.endpoint());
    attach(&mut keeper, true, true, true);
    let deadline = Instant::now() + WAIT;
    loop {
        let state = keeper.machine_state(deadline.saturating_duration_since(Instant::now()), MINE);
        if until(&state) {
            return (keeper, state);
        }
    }
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
