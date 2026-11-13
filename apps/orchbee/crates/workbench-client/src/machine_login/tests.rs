//! The client-side rules of one login episode, as data: each case drives notices and the person's
//! acts and checks what is shown, what the holder is asked to do, and how the episode ended.

use super::*;
use orchbee_machine_protocol::{AnswerToken, PathText};

const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const TARGET: &str = "user@devbox";
const PASSWORD: &str = "user@devbox's password: ";
const OTHER_HOST_PASSWORD: &str = "user@jump's password: ";
const PASSPHRASE: &str = "Enter passphrase for key '/home/user/.ssh/id_ed25519': ";
const NEW_HOST: &str = "The authenticity of host 'devbox (10.0.0.8)' can't be established.\n\
ED25519 key fingerprint is SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG.\n\
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
const OTHER_NEW_HOST: &str = "The authenticity of host 'jump (10.0.0.9)' can't be established.\n\
ED25519 key fingerprint is SHA256:abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG.\n\
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
/// Another Machine this client knows.
const OTHER_MACHINE: &str = "fedcba9876543210fedcba9876543210";
const CHANGED_KEY: &str = "@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@\n\
@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n\
@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Password,
    Passphrase,
    HostKey,
    Other,
}

fn kind_of(kind: &QuestionKind) -> Kind {
    match kind {
        QuestionKind::Password => Kind::Password,
        QuestionKind::KeyPassphrase => Kind::Passphrase,
        QuestionKind::HostKey { .. } => Kind::HostKey,
        QuestionKind::Other => Kind::Other,
    }
}

fn id(value: u32) -> Id {
    Id::new(value).unwrap()
}

/// The kind the hub sent with a prompt: its own reading, which the client reads again.
fn hub_kind(prompt: &str) -> QuestionKind {
    crate::machine_question::classify(prompt)
}

#[derive(Clone, Copy, Debug)]
enum Step {
    /// The person's selection now waits on this Machine, or on none.
    Wait(Option<&'static str>),
    Claim(u32),
    ClaimUnknown(u32),
    /// Question `question` of episode `login`, with `ssh`'s prompt, maybe repeated.
    Ask {
        login: u32,
        question: u32,
        prompt: &'static str,
        repeated: bool,
    },
    /// The hub's kind is `host-key` whatever the prompt says.
    AskHostKey {
        question: u32,
        prompt: &'static str,
    },
    Type(&'static str),
    Act(Act),
    /// An act on the overlay painted for question `question` of episode 1.
    ActOn(u32, Act),
    Delivered(Delivery),
    Ended(u32, LoginEnd),
    Lost,
}

#[derive(Debug, Eq, PartialEq)]
enum Did {
    Deliver(Vec<u8>),
    Dismiss(u32),
}

#[derive(Debug, Eq, PartialEq)]
struct Seen {
    kind: Kind,
    notice: Option<AskNotice>,
    sending: bool,
    characters: usize,
    outcome: Option<Ending>,
}

const fn shown(kind: Kind, notice: Option<AskNotice>, sending: bool, characters: usize) -> Seen {
    Seen {
        kind,
        notice,
        sending,
        characters,
        outcome: None,
    }
}

/// The overlay of a login that ended without the person: how it ended, and an empty field.
const fn ended(kind: Kind, ending: Ending) -> Seen {
    Seen {
        kind,
        notice: None,
        sending: false,
        characters: 0,
        outcome: Some(ending),
    }
}

struct Run {
    logins: Logins,
    /// The token of the question last shown, for acts on the current overlay.
    token: Option<Token>,
}

impl Run {
    /// The person's selection waits on `MACHINE`.
    fn new() -> Self {
        let mut logins = Logins::default();
        assert!(logins.wait_for(Some(MACHINE)).is_empty());
        Self {
            logins,
            token: None,
        }
    }

    fn step(&mut self, step: Step) -> Vec<Did> {
        let target = |machine: &str| {
            [MACHINE, OTHER_MACHINE]
                .contains(&machine)
                .then(|| TARGET.to_owned())
        };
        let effects = match step {
            Step::Wait(machine) => self.logins.wait_for(machine),
            Step::Claim(login) => self.logins.notice(
                Notice::Claimed {
                    login: id(login),
                    machine: MACHINE.into(),
                },
                target,
            ),
            Step::ClaimUnknown(login) => self.logins.notice(
                Notice::Claimed {
                    login: id(login),
                    machine: "00000000000000000000000000000000".into(),
                },
                target,
            ),
            Step::Ask {
                login,
                question,
                prompt,
                repeated,
            } => self.logins.notice(
                Notice::Question(ask(login, question, prompt, repeated, hub_kind(prompt))),
                target,
            ),
            Step::AskHostKey { question, prompt } => {
                let host_key = match hub_kind(NEW_HOST) {
                    QuestionKind::HostKey { host_key } => host_key,
                    _ => unreachable!("the new-host prompt reads as a new host"),
                };
                self.logins.notice(
                    Notice::Question(ask(
                        1,
                        question,
                        prompt,
                        false,
                        QuestionKind::HostKey { host_key },
                    )),
                    target,
                )
            }
            Step::Type(text) => {
                let field = &self.logins.presented().expect("a question is shown").field;
                for character in text.chars() {
                    field.push(character);
                }
                Vec::new()
            }
            Step::Act(act) => self
                .logins
                .act(self.token.expect("a question was shown"), act),
            Step::ActOn(question, act) => self.logins.act(
                Token {
                    login: id(1),
                    question: id(question),
                },
                act,
            ),
            Step::Delivered(outcome) => self.logins.notice(
                Notice::Delivered {
                    token: self.token.expect("a question was shown"),
                    outcome,
                },
                target,
            ),
            Step::Ended(login, end) => self.logins.notice(
                Notice::Ended {
                    login: id(login),
                    end,
                },
                target,
            ),
            Step::Lost => self.logins.notice(Notice::Lost, target),
        };
        if let Some(presented) = self.logins.presented() {
            self.token = Some(presented.token);
        }
        effects
            .into_iter()
            .map(|effect| match effect {
                Effect::Deliver { answer, .. } => Did::Deliver(answer.as_bytes().to_vec()),
                Effect::Dismiss(login) => Did::Dismiss(login.get()),
            })
            .collect()
    }

    fn seen(&self) -> Option<Seen> {
        self.logins.presented().map(|presented| Seen {
            kind: kind_of(&presented.kind),
            notice: presented.notice,
            sending: presented.sending,
            characters: presented.field.characters(),
            outcome: presented.outcome,
        })
    }
}

fn ask(login: u32, question: u32, prompt: &str, repeated: bool, kind: QuestionKind) -> Question {
    Question {
        login: id(login),
        question: id(question),
        kind,
        prompt: Prompt::new(prompt).unwrap(),
        repeated,
        remembered_rejected: false,
        answer: AnswerEndpoint {
            endpoint: PathText::new(format!("answer-{login}-{question}")).unwrap(),
            token: AnswerToken::from_bytes([7; 16]),
        },
    }
}

type Case = (
    &'static str,
    Vec<(Step, Vec<Did>, Option<Seen>)>,
    Option<Ending>,
);

fn password(question: u32, repeated: bool) -> Step {
    Step::Ask {
        login: 1,
        question,
        prompt: PASSWORD,
        repeated,
    }
}

#[test]
fn the_episode_follows_its_rules() {
    use Act::{Cancel, Close, Submit, Trust};
    use AskNotice::{Inadmissible, Repeated};
    use Kind::{HostKey, Other, Passphrase, Password};
    let cases: Vec<Case> = vec![
        (
            "a password is asked, typed, delivered and the login succeeds",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("sesame"),
                    vec![],
                    Some(shown(Password, None, false, 6)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
                (
                    Step::Delivered(Delivery::Taken),
                    vec![],
                    Some(shown(Password, None, true, 0)),
                ),
                (Step::Ended(1, LoginEnd::Succeeded), vec![], None),
            ],
            Some(Ending::Hub(LoginEnd::Succeeded)),
        ),
        (
            "a refused answer asks again with the refusal and an empty field",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("wrong"),
                    vec![],
                    Some(shown(Password, None, false, 5)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
                (
                    Step::Delivered(Delivery::Taken),
                    vec![],
                    Some(shown(Password, None, true, 0)),
                ),
                (
                    password(2, true),
                    vec![],
                    Some(shown(Password, Some(Repeated), false, 0)),
                ),
                (
                    Step::Type("right"),
                    vec![],
                    Some(shown(Password, Some(Repeated), false, 5)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
                (Step::Ended(1, LoginEnd::Succeeded), vec![], None),
            ],
            Some(Ending::Hub(LoginEnd::Succeeded)),
        ),
        (
            "a new host, then a passphrase, follow each other in one episode",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    Step::Ask {
                        login: 1,
                        question: 1,
                        prompt: NEW_HOST,
                        repeated: false,
                    },
                    vec![],
                    Some(shown(HostKey, None, false, 0)),
                ),
                // A new host is trusted, never answered from the field.
                (
                    Step::Act(Submit),
                    vec![],
                    Some(shown(HostKey, None, false, 0)),
                ),
                (
                    Step::Act(Trust),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(HostKey, None, true, 0)),
                ),
                (
                    Step::Delivered(Delivery::Taken),
                    vec![],
                    Some(shown(HostKey, None, true, 0)),
                ),
                (
                    Step::Ask {
                        login: 1,
                        question: 2,
                        prompt: PASSPHRASE,
                        repeated: false,
                    },
                    vec![],
                    Some(shown(Passphrase, None, false, 0)),
                ),
                // Trust has no meaning for a secret.
                (
                    Step::Act(Trust),
                    vec![],
                    Some(shown(Passphrase, None, false, 0)),
                ),
                (
                    Step::Type("phrase"),
                    vec![],
                    Some(shown(Passphrase, None, false, 6)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Passphrase, None, true, 0)),
                ),
                (Step::Ended(1, LoginEnd::Succeeded), vec![], None),
            ],
            Some(Ending::Hub(LoginEnd::Succeeded)),
        ),
        (
            "cancelling dismisses the episode and later notices of it change nothing",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("abc"),
                    vec![],
                    Some(shown(Password, None, false, 3)),
                ),
                (Step::Act(Cancel), vec![Did::Dismiss(1)], None),
                (password(2, true), vec![], None),
                (Step::Ended(1, LoginEnd::Dismissed), vec![], None),
            ],
            Some(Ending::Dismissed),
        ),
        (
            "cancelling acts on the episode even from an overlay of an earlier question",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
                (
                    password(2, true),
                    vec![],
                    Some(shown(Password, Some(Repeated), false, 0)),
                ),
                // A submit painted for the first question does not answer the second.
                (
                    Step::ActOn(1, Submit),
                    vec![],
                    Some(shown(Password, Some(Repeated), false, 0)),
                ),
                (Step::ActOn(1, Cancel), vec![Did::Dismiss(1)], None),
            ],
            Some(Ending::Dismissed),
        ),
        (
            "an answer ssh cannot take is asked again, not sent",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("ab"),
                    vec![],
                    Some(shown(Password, None, false, 2)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
                (
                    Step::Delivered(Delivery::Inadmissible),
                    vec![],
                    Some(shown(Password, Some(Inadmissible), false, 0)),
                ),
                (
                    Step::Type("cd"),
                    vec![],
                    Some(shown(Password, Some(Inadmissible), false, 2)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
            ],
            None,
        ),
        (
            "an answer that reaches no endpoint ends the login; its overlay stays until closed",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Act(Submit),
                    vec![Did::Deliver(Vec::new())],
                    Some(shown(Password, None, true, 0)),
                ),
                (
                    Step::Delivered(Delivery::Undelivered),
                    vec![Did::Dismiss(1)],
                    Some(ended(Password, Ending::Undelivered)),
                ),
                (Step::Act(Close), vec![], None),
            ],
            Some(Ending::Undelivered),
        ),
        (
            "the hub ending the login while the person types keeps the overlay with the end",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("half"),
                    vec![],
                    Some(shown(Password, None, false, 4)),
                ),
                (
                    Step::Ended(1, LoginEnd::Failed),
                    vec![],
                    Some(ended(Password, Ending::Hub(LoginEnd::Failed))),
                ),
                // Nothing typed before the end can still be sent.
                (
                    Step::Act(Submit),
                    vec![],
                    Some(ended(Password, Ending::Hub(LoginEnd::Failed))),
                ),
                (Step::Act(Close), vec![], None),
            ],
            Some(Ending::Hub(LoginEnd::Failed)),
        ),
        (
            "a lost link keeps the overlay with the end until the person closes it",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("ab"),
                    vec![],
                    Some(shown(Password, None, false, 2)),
                ),
                (Step::Lost, vec![], Some(ended(Password, Ending::Lost))),
                (Step::Act(Cancel), vec![], None),
            ],
            Some(Ending::Lost),
        ),
        (
            "success before the person answered keeps the overlay; only an answered one closes",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("ab"),
                    vec![],
                    Some(shown(Password, None, false, 2)),
                ),
                (
                    Step::Ended(1, LoginEnd::Succeeded),
                    vec![],
                    Some(ended(Password, Ending::Hub(LoginEnd::Succeeded))),
                ),
                (Step::Act(Close), vec![], None),
            ],
            Some(Ending::Hub(LoginEnd::Succeeded)),
        ),
        (
            "a selection abandoned before its question is dismissed and nothing is shown",
            vec![
                (Step::Wait(None), vec![], None),
                (Step::Claim(1), vec![Did::Dismiss(1)], None),
                (password(1, false), vec![], None),
            ],
            None,
        ),
        (
            "choosing another Machine abandons the first one's login",
            vec![
                (Step::Claim(1), vec![], None),
                (Step::Wait(Some(OTHER_MACHINE)), vec![Did::Dismiss(1)], None),
                (password(1, false), vec![], None),
                // The first Machine is no longer waited on: a new login of it is let go.
                (Step::Claim(2), vec![Did::Dismiss(2)], None),
            ],
            Some(Ending::Dismissed),
        ),
        (
            "a shown overlay is never replaced: another login is dismissed on arrival",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Type("abc"),
                    vec![],
                    Some(shown(Password, None, false, 3)),
                ),
                (
                    Step::Claim(2),
                    vec![Did::Dismiss(2)],
                    Some(shown(Password, None, false, 3)),
                ),
                (
                    Step::Ask {
                        login: 2,
                        question: 1,
                        prompt: PASSPHRASE,
                        repeated: false,
                    },
                    vec![],
                    Some(shown(Password, None, false, 3)),
                ),
                (
                    Step::Ended(1, LoginEnd::Failed),
                    vec![],
                    Some(ended(Password, Ending::Hub(LoginEnd::Failed))),
                ),
                (
                    Step::Claim(3),
                    vec![Did::Dismiss(3)],
                    Some(ended(Password, Ending::Hub(LoginEnd::Failed))),
                ),
            ],
            Some(Ending::Hub(LoginEnd::Failed)),
        ),
        (
            "a selection that ends while its question is shown keeps the overlay with that end",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    password(1, false),
                    vec![],
                    Some(shown(Password, None, false, 0)),
                ),
                (
                    Step::Wait(None),
                    vec![Did::Dismiss(1)],
                    Some(ended(Password, Ending::Dismissed)),
                ),
                (Step::Act(Close), vec![], None),
            ],
            Some(Ending::Dismissed),
        ),
        (
            "a changed key, or a question naming another host, never asks for trust",
            vec![
                (Step::Claim(1), vec![], None),
                (
                    Step::AskHostKey {
                        question: 1,
                        prompt: CHANGED_KEY,
                    },
                    vec![],
                    Some(shown(Other, None, false, 0)),
                ),
                (Step::Act(Trust), vec![], Some(shown(Other, None, false, 0))),
                (
                    Step::AskHostKey {
                        question: 2,
                        prompt: OTHER_NEW_HOST,
                    },
                    vec![],
                    Some(shown(Other, None, false, 0)),
                ),
                (Step::Act(Trust), vec![], Some(shown(Other, None, false, 0))),
                (
                    Step::Ask {
                        login: 1,
                        question: 3,
                        prompt: OTHER_HOST_PASSWORD,
                        repeated: false,
                    },
                    vec![],
                    Some(shown(Other, None, false, 0)),
                ),
            ],
            None,
        ),
        (
            "a question of an episode this client did not claim is not shown",
            vec![
                (
                    Step::Ask {
                        login: 2,
                        question: 1,
                        prompt: PASSWORD,
                        repeated: false,
                    },
                    vec![],
                    None,
                ),
                (Step::ClaimUnknown(3), vec![Did::Dismiss(3)], None),
                (
                    Step::Ask {
                        login: 3,
                        question: 1,
                        prompt: PASSWORD,
                        repeated: false,
                    },
                    vec![],
                    None,
                ),
            ],
            None,
        ),
    ];
    for (name, steps, ending) in &cases {
        let mut run = Run::new();
        for (index, (step, did, seen)) in steps.iter().enumerate() {
            let effects = run.step(*step);
            // Deliveries compare by kind here; what they carry is checked below.
            let effects: Vec<Did> = effects
                .into_iter()
                .map(|effect| match effect {
                    Did::Deliver(_) => Did::Deliver(Vec::new()),
                    other => other,
                })
                .collect();
            assert_eq!(&effects, did, "{name}: effects of step {index} {step:?}");
            assert_eq!(
                &run.seen(),
                seen,
                "{name}: shown after step {index} {step:?}"
            );
        }
        assert_eq!(run.logins.take_ending(MACHINE), *ending, "{name}: ending");
    }
}

#[test]
fn a_delivery_carries_exactly_what_was_typed_and_trust_carries_yes() {
    let mut run = Run::new();
    run.step(Step::Claim(1));
    run.step(password(1, false));
    run.step(Step::Type("tést-wörd"));
    assert!(run.logins.presented().unwrap().field.pop());
    assert_eq!(
        run.step(Step::Act(Act::Submit)),
        [Did::Deliver("tést-wör".as_bytes().to_vec())]
    );
    run.step(Step::Ask {
        login: 1,
        question: 2,
        prompt: NEW_HOST,
        repeated: false,
    });
    assert_eq!(
        run.step(Step::Act(Act::Trust)),
        [Did::Deliver(b"yes".to_vec())]
    );
}

#[test]
fn every_end_of_a_question_wipes_its_field() {
    let ends: [&[Step]; 5] = [
        &[Step::Act(Act::Cancel)],
        &[Step::Ended(1, LoginEnd::Failed)],
        &[Step::Lost],
        &[password(2, true)],
        &[Step::Wait(None)],
    ];
    for end in ends {
        let mut run = Run::new();
        run.step(Step::Claim(1));
        run.step(password(1, false));
        run.step(Step::Type("secret"));
        let held = run.logins.presented().unwrap().field;
        for step in end {
            run.step(*step);
        }
        assert_eq!(held.characters(), 0, "{end:?}");
        assert!(held.take().as_bytes().is_empty(), "{end:?}");
    }
}
