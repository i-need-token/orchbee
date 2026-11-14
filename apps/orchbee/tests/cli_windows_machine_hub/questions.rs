//! The login episode and the answer path through real processes: a fake `ssh`, which runs only in
//! these tests, asks OpenSSH-worded questions through the askpass helper, and a test client claims
//! the episode and delivers answers straight to each question's endpoint. Each kind reaches the
//! claimant in order and the answers reach `ssh`; wrong answers lead to the next question and then
//! `login-failed`; a changed host key is refused without a question; a silent attempt or a command
//! without answers asks nobody; an unanswered question ends within its bound; an endpoint takes one
//! answer for its own question only; the claimant leaving ends the attempt; and a helper that
//! cannot ask ends the attempt before `ssh` could read an empty answer. `remembered` covers the
//! answers the askpass helper reads from the credential store. After each run that delivered
//! answers, no file under the scratch root and no captured stream holds one.

use super::*;
use orchbee_machine_protocol::{
    AnswerToken, Id,
    hub::{
        AnswerEndpoint, ConnectResult, LoginEnd, LoginEvent, LoginPhase, NeedsLoginCause, Question,
        QuestionKind,
    },
};
use orchbee_workbench_client::application::machine_hub::{DeliveryFailure, deliver};
use sha2::{Digest, Sha256};

#[path = "questions/remembered.rs"]
mod remembered;

/// The host-key confirmation as OpenSSH 9.5 words it.
const NEW_HOST: &str = concat!(
    "The authenticity of host 'host.invalid (192.0.2.1)' can't be established.\n",
    "ED25519 key fingerprint is SHA256:abcDEF0123456789+/abcDEF0123456789abcdefgh.\n",
    "This key is not known by any other names.\n",
    "Are you sure you want to continue connecting (yes/no/[fingerprint])? ",
);
const NEW_FINGERPRINT: &str = "SHA256:abcDEF0123456789+/abcDEF0123456789abcdefgh";
const PASSPHRASE: &str = "Enter passphrase for key 'C:\\Users\\someone/.ssh/id_ed25519': ";
const PASSWORD: &str = "user@host.invalid's password: ";
const CODE: &str = "(user@host.invalid) Verification code: ";
const CHANGED: &str = "SHA256:changedKEY0123456789abcdefghijklmnopqrstuv";

/// Obvious test words, never real ones; the search looks for each of them.
const RIGHT_PASSPHRASE: &str = "test-passphrase-5be1";
const RIGHT_PASSWORD: &str = "test-password-9c4d";
const RIGHT_CODE: &str = "test-code-2718";
const WRONG: &str = "test-wrong-41aa";
const SECRETS: [&str; 4] = [RIGHT_PASSPHRASE, RIGHT_PASSWORD, RIGHT_CODE, WRONG];
/// Bounds how long an answer endpoint stands after it sent its receipt: as long as a client waits
/// for a receipt.
const ENDPOINT_DROP: Duration = Duration::from_secs(5);

#[test]
fn each_kind_reaches_the_claimant_in_order_and_only_ssh_receives_the_answers() {
    let (carried, fake) = Carried::faking(
        "mq1-kinds-",
        &json!({"questions": [
            step(NEW_HOST, "yes", 1, true),
            step(PASSPHRASE, RIGHT_PASSPHRASE, 1, false),
            step(PASSWORD, RIGHT_PASSWORD, 1, false),
            step(CODE, RIGHT_CODE, 1, false),
        ]}),
        &[],
    );
    let mut client = claimant(&carried);
    let id = client.send(user_connect());
    let host = question(&mut client);
    assert_eq!(host.question.get(), 1);
    match &host.kind {
        QuestionKind::HostKey { host_key } => {
            assert_eq!(host_key.algorithm.as_str(), "ed25519");
            assert_eq!(host_key.fingerprint.as_str(), NEW_FINGERPRINT);
        }
        other => panic!("the first question read as {other:?}"),
    }
    assert_eq!(host.prompt.as_str(), NEW_HOST);
    deliver(&host.answer, b"yes").unwrap();
    for (number, kind, answer) in [
        (2, QuestionKind::KeyPassphrase, RIGHT_PASSPHRASE),
        (3, QuestionKind::Password, RIGHT_PASSWORD),
        (4, QuestionKind::Other, RIGHT_CODE),
    ] {
        let asked = question(&mut client);
        assert_eq!((asked.question.get(), &asked.kind), (number, &kind));
        assert_eq!(asked.login, host.login);
        assert!(!asked.repeated);
        deliver(&asked.answer, answer.as_bytes()).unwrap();
    }
    let result = connect_reply(&mut client, id);
    assert!(
        matches!(result.outcome, ConnectOutcome::Online { .. }),
        "{result:?}"
    );
    assert_eq!(login_end(&mut client), LoginEnd::Succeeded);
    assert_eq!(
        fake.transcript(),
        "started\nasked 1\nright 1\nasked 2\nright 2\nasked 3\nright 3\nasked 4\nright 4\nlinked\n"
    );
    drop(client);
    let printed = carried
        .command(&["--machine-hub-state-v1"])
        .output()
        .unwrap();
    search(&carried, &[&printed.stdout, &printed.stderr]);
}

#[test]
fn a_refused_answer_leads_to_the_next_question_and_then_to_login_failed() {
    let (carried, fake) = Carried::faking(
        "mq1-wrong-",
        &json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 2, false)]}),
        &[],
    );
    let mut client = claimant(&carried);
    let id = client.send(user_connect());
    let first = question(&mut client);
    assert!(!first.repeated);
    deliver(&first.answer, WRONG.as_bytes()).unwrap();
    // `ssh` asks again; the hub only marks that the last answer of this kind was refused.
    let second = question(&mut client);
    assert_eq!(second.question.get(), 2);
    assert!(second.repeated);
    deliver(&second.answer, WRONG.as_bytes()).unwrap();
    let result = connect_reply(&mut client, id);
    assert_eq!(
        result.outcome,
        ConnectOutcome::NotOnline(ConnectFailure::AuthFailed)
    );
    assert_eq!(login_end(&mut client), LoginEnd::Failed);
    let printed = carried
        .command(&["--machine-hub-state-v1"])
        .output()
        .unwrap();
    assert_eq!(
        State::parse(&printed).machines,
        [(FIRST.to_owned(), "login failed".to_owned())]
    );
    assert_eq!(
        fake.transcript(),
        "started\nasked 1\nwrong 1\nasked 2\nwrong 2\n"
    );
    // Nothing retries for the person: no further carrier starts.
    thread::sleep(Duration::from_secs(2));
    assert_eq!(fake.transcript().matches("started").count(), 1);
    drop(client);
    search(&carried, &[&printed.stdout, &printed.stderr]);
}

#[test]
fn a_changed_host_key_is_refused_without_a_question() {
    let (carried, fake) = Carried::faking(
        "mq1-changed-",
        &json!({"changed": CHANGED, "questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)]}),
        &[],
    );
    let mut client = claimant(&carried);
    let id = client.send(user_connect());
    let result = connect_reply(&mut client, id);
    assert_eq!(
        result.outcome,
        ConnectOutcome::NotOnline(ConnectFailure::HostKeyChanged)
    );
    assert!(no_question(&mut client));
    assert_eq!(fake.transcript(), "started\nchanged\n");
    assert_eq!(
        carried.state().machines,
        [(FIRST.to_owned(), format!("key changed  ed25519 {CHANGED}"))]
    );
}

#[test]
fn a_silent_attempt_and_a_command_without_answers_ask_nobody() {
    // A command without answers: the helper is refused and the attempt ends `untrusted`.
    let (carried, fake) = Carried::faking(
        "mq1-command-",
        &json!({"questions": [step(NEW_HOST, "yes", 1, true)]}),
        &[],
    );
    let connected = carried.run(&["machine", "connect", FIRST]);
    assert_eq!(connected.code, Some(3), "{connected}");
    assert_eq!(connected.document["result"]["error"], "host-untrusted");
    assert_eq!(
        carried.state().machines,
        [(
            FIRST.to_owned(),
            format!("untrusted  ed25519 {NEW_FINGERPRINT}")
        )]
    );
    assert!(
        fake.transcript().starts_with("started\nasked 1\n"),
        "{}",
        fake.transcript()
    );
    exit_hub(&carried.endpoint());

    // A kept Machine's silent attempt asks nobody, not even a client that asks.
    fake.plan(&json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)]}));
    carried.state();
    let mut keeper = Client::connect(&carried.endpoint());
    attach(&mut keeper, true, true, true);
    let deadline = Instant::now() + WAIT;
    loop {
        let state = keeper.machine_state(deadline.saturating_duration_since(Instant::now()), FIRST);
        if state.name() == MachineStateName::NeedsLogin {
            assert_eq!(
                state,
                MachineState::NeedsLogin {
                    cause: Some(NeedsLoginCause::AuthRequired)
                }
            );
            break;
        }
    }
    assert!(no_question(&mut keeper));
    // The hub ended the carrier before its helper was refused, so the fake may not have noted that.
    let transcript = fake.transcript();
    assert_eq!(transcript.matches("asked").count(), 2, "{transcript}");
    assert!(!transcript.contains("right"), "{transcript}");
}

#[test]
fn an_unanswered_question_ends_within_its_bound_and_a_late_answer_is_refused() {
    let (carried, _fake) = Carried::faking(
        "mq1-bound-",
        &json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)]}),
        &[("ORCHBEE_MACHINE_HUB_QUESTION_MS", "2000")],
    );
    let mut client = claimant(&carried);
    let id = client.send(user_connect());
    let asked = question(&mut client);
    let opened = Instant::now();
    let result = connect_reply(&mut client, id);
    let waited = opened.elapsed();
    assert_eq!(
        result.outcome,
        ConnectOutcome::NotOnline(ConnectFailure::Timeout)
    );
    // Nothing hangs: the hub ends the question at its bound, well inside the wait.
    assert!(waited < WAIT, "{waited:?}");
    assert_eq!(login_end(&mut client), LoginEnd::Unavailable);
    assert_eq!(
        carried.state().machines,
        [(FIRST.to_owned(), "needs login".to_owned())]
    );
    assert!(deliver(&asked.answer, RIGHT_PASSWORD.as_bytes()).is_err());
}

#[test]
fn an_endpoint_takes_one_answer_from_this_user_for_its_own_question_only() {
    let (carried, _fake) = Carried::faking(
        "mq1-endpoint-",
        &json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 2, false)]}),
        &[],
    );
    let mut client = claimant(&carried);
    let id = client.send(user_connect());
    let first = question(&mut client);
    let foreign = AnswerEndpoint {
        endpoint: first.answer.endpoint.clone(),
        token: AnswerToken::from_bytes([0x5a; 16]),
    };
    assert_eq!(
        deliver(&foreign, WRONG.as_bytes()),
        Err(DeliveryFailure::Refused)
    );
    // The endpoint kept waiting for its own token. It sends the receipt of the answer it takes
    // before it drops, so a second answer may still meet it and get only a close; that answer is
    // never taken, and soon there is no endpoint at all.
    deliver(&first.answer, WRONG.as_bytes()).unwrap();
    let dropped = Instant::now() + ENDPOINT_DROP;
    loop {
        match deliver(&first.answer, WRONG.as_bytes()) {
            Err(DeliveryFailure::Unreachable) => break,
            Err(DeliveryFailure::Refused) => assert!(
                Instant::now() < dropped,
                "the answered endpoint still stood after {ENDPOINT_DROP:?}"
            ),
            other => panic!("a second answer met {other:?}"),
        }
        thread::sleep(Duration::from_millis(50));
    }
    let second = question(&mut client);
    assert_ne!(second.answer.endpoint, first.answer.endpoint);
    let crossed = AnswerEndpoint {
        endpoint: second.answer.endpoint.clone(),
        token: first.answer.token,
    };
    assert_eq!(
        deliver(&crossed, WRONG.as_bytes()),
        Err(DeliveryFailure::Refused)
    );
    deliver(&second.answer, RIGHT_PASSWORD.as_bytes()).unwrap();
    let result = connect_reply(&mut client, id);
    assert!(
        matches!(result.outcome, ConnectOutcome::Online { .. }),
        "{result:?}"
    );
    assert_eq!(
        deliver(&second.answer, RIGHT_PASSWORD.as_bytes()),
        Err(DeliveryFailure::Unreachable)
    );
    drop(client);
    search(&carried, &[]);
}

#[test]
fn the_claimant_leaving_with_its_question_open_ends_the_attempt() {
    let (carried, _fake) = Carried::faking(
        "mq1-leave-",
        &json!({"questions": [step(PASSWORD, RIGHT_PASSWORD, 1, false)]}),
        &[],
    );
    let endpoint = carried.endpoint();
    let hub = Hub::observe(query_pid(&endpoint));
    let mut watcher = Client::connect(&endpoint);
    watcher.attach(false, true);
    let mut client = claimant(&carried);
    client.send(user_connect());
    let asked = question(&mut client);
    let carrier = one_child(&hub, None);
    drop(client);
    let deadline = Instant::now() + WAIT;
    loop {
        let state =
            watcher.machine_state(deadline.saturating_duration_since(Instant::now()), FIRST);
        if state.name() == MachineStateName::NeedsLogin {
            break;
        }
    }
    assert!(
        carrier.exited(WAIT).is_some(),
        "the carrier outlived its claimant"
    );
    assert!(deliver(&asked.answer, RIGHT_PASSWORD.as_bytes()).is_err());
}

#[test]
fn a_helper_that_cannot_ask_ends_the_attempt_before_ssh_reads_an_empty_answer() {
    // An empty prompt is no question: the helper tells the hub, which ends the carrier while the
    // helper still waits, so the fake never sees the helper exit.
    let (carried, fake) = Carried::faking(
        "mq1-unable-",
        &json!({"questions": [step("", RIGHT_PASSWORD, 1, false)]}),
        &[],
    );
    let mut client = claimant(&carried);
    let id = client.send(user_connect());
    let result = connect_reply(&mut client, id);
    assert_eq!(
        result.outcome,
        ConnectOutcome::NotOnline(ConnectFailure::AuthRequired)
    );
    assert!(no_question(&mut client));
    assert_eq!(
        carried.state().machines,
        [(FIRST.to_owned(), "needs login".to_owned())]
    );
    assert_eq!(fake.transcript(), "started\nasked 1\n");
}

/// The fake `ssh` of one test: its plan, which holds digests and never an answer, and the
/// transcript it keeps of what happened.
struct Fake {
    plan: PathBuf,
    transcript: PathBuf,
}

impl Fake {
    /// Replaces the plan; the fake reads it at each start.
    fn plan(&self, plan: &Value) {
        fs::write(&self.plan, serde_json::to_vec(plan).unwrap()).unwrap();
    }

    fn transcript(&self) -> String {
        fs::read_to_string(&self.transcript).unwrap_or_default()
    }
}

impl Carried {
    /// A carried Machine whose stand-in is the fake `ssh` running `plan`, with `overrides` set for
    /// the hub, which is started here so its view holds them.
    fn faking(
        prefix: &'static str,
        plan: &Value,
        overrides: &[(&'static str, &str)],
    ) -> (Self, Fake) {
        let mut carried = Self::new(prefix, false);
        let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|folder| folder.join("node.exe"))
            .find(|path| path.is_file())
            .expect("the fake ssh runs on Node.js");
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/windows-fake-ssh.mjs");
        let wrapper = PathBuf::from(carried.fixture.path("standin/fake-ssh.cmd"));
        fs::write(
            &wrapper,
            format!(
                "@echo off\r\n\"{}\" \"{}\" %*\r\n",
                node.display(),
                script.display()
            ),
        )
        .unwrap();
        let fake = Fake {
            plan: PathBuf::from(carried.fixture.path("standin/plan.json")),
            transcript: PathBuf::from(carried.fixture.path("standin/transcript.txt")),
        };
        fake.plan(plan);
        let client =
            Path::new(&carried.fixture.path("cohort")).join(CohortArtifact::Client.file_name());
        carried.stand_in = wrapper;
        carried.variables = vec![
            ("FAKE_SSH_PLAN", fake.plan.clone().into_os_string()),
            (
                "FAKE_SSH_TRANSCRIPT",
                fake.transcript.clone().into_os_string(),
            ),
            ("FAKE_SSH_CLIENT", client.into_os_string()),
        ];
        carried.variables.extend(
            overrides
                .iter()
                .map(|(name, value)| (*name, OsString::from(value))),
        );
        carried.state();
        (carried, fake)
    }
}

/// One question of a plan: its prompt, the digest of the answer it accepts, how often it may be
/// answered wrongly, and whether a refusal reads as a host-key one.
fn step(prompt: &str, accept: &str, tries: u32, host: bool) -> Value {
    let digest: String = Sha256::digest(accept.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    json!({"prompt": prompt, "accept": digest, "tries": tries, "host": host})
}

/// A client attached to the running hub that can be asked.
fn claimant(carried: &Carried) -> Client {
    let mut client = Client::connect(&carried.endpoint());
    attach(&mut client, false, false, false);
    client
}

fn attach(client: &mut Client, presence: bool, asks: bool, watch: bool) {
    let id = client.send(Request::Attach(Attach {
        presence,
        asks,
        watch,
    }));
    match client.answer(WAIT, id) {
        Answer::Reply(Reply::Attach(AttachOutcome::Attached { .. })) => {}
        other => panic!("attach answered {other:?}"),
    }
}

/// A person's `connect` of `FIRST`.
fn user_connect() -> Request {
    Request::Connect(Connect {
        machine: machine(FIRST),
        origin: Origin::User,
        draft: None,
    })
}

fn question(client: &mut Client) -> Question {
    client.until(WAIT, "question", |client| {
        client.take(|delivery| match delivery {
            Delivery::Message(Message::Event {
                event: Event::Question(question),
                ..
            }) => Some(question.clone()),
            _ => None,
        })
    })
}

/// Whether no question arrived, after reading what the hub sent for a moment.
fn no_question(client: &mut Client) -> bool {
    client.pump_until(Instant::now() + Duration::from_millis(500));
    !client.deliveries.iter().any(|delivery| {
        matches!(
            delivery,
            Delivery::Message(Message::Event {
                event: Event::Question(_) | Event::Login(_) | Event::ConnectWaiting(_),
                ..
            })
        )
    })
}

fn connect_reply(client: &mut Client, id: Id) -> ConnectResult {
    match client.answer(WAIT, id) {
        Answer::Reply(Reply::Connect(result)) => result,
        other => panic!("connect answered {other:?}"),
    }
}

/// How the episode the client claimed ended.
fn login_end(client: &mut Client) -> LoginEnd {
    client.until(WAIT, "login end", |client| {
        client.take(|delivery| match delivery {
            Delivery::Message(Message::Event {
                event:
                    Event::Login(LoginEvent {
                        phase: LoginPhase::Ended { end },
                        ..
                    }),
                ..
            }) => Some(*end),
            _ => None,
        })
    })
}

/// Ends the hub and the far Session, then reads every file under the scratch root and every
/// `stream` for each test answer, as UTF-8 and as UTF-16LE. The hub writes no log of its own and
/// its standard streams are the null device; the fake `ssh`, which receives the answers, writes
/// none of them.
fn search(carried: &Carried, streams: &[&[u8]]) {
    carried.quiesce();
    let needles: Vec<(&str, Vec<u8>)> = SECRETS
        .iter()
        .flat_map(|secret| {
            let wide: Vec<u8> = secret.encode_utf16().flat_map(u16::to_le_bytes).collect();
            [(*secret, secret.as_bytes().to_vec()), (*secret, wide)]
        })
        .collect();
    // Every test word begins with `t` in both encodings, so one pass looks only where one could.
    assert!(needles.iter().all(|(_, needle)| needle[0] == b't'));
    let holds = |bytes: &[u8]| {
        (0..bytes.len())
            .filter(|at| bytes[*at] == b't')
            .find_map(|at| {
                needles
                    .iter()
                    .find(|(_, needle)| bytes[at..].starts_with(needle))
            })
            .map(|(secret, _)| *secret)
    };
    for (index, stream) in streams.iter().enumerate() {
        assert_eq!(holds(stream), None, "captured stream {index}");
    }
    let mut folders = vec![PathBuf::from(carried.fixture.path(""))];
    let (mut files, mut unreadable) = (0, Vec::new());
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).unwrap().flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => folders.push(path),
                Ok(kind) if kind.is_file() => match fs::read(&path) {
                    Ok(bytes) => {
                        files += 1;
                        assert_eq!(holds(&bytes), None, "{}", path.display());
                    }
                    // The fixture's own Sessions hold their empty lock files until it ends.
                    Err(_) if fs::metadata(&path).is_ok_and(|file| file.len() == 0) => {}
                    Err(_) => unreadable.push(path),
                },
                _ => {}
            }
        }
    }
    assert!(files > 0);
    assert!(unreadable.is_empty(), "unread: {unreadable:?}");
}
