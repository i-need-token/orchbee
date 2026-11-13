//! One login episode as the client that claimed it sees it: the questions the hub puts to it, the
//! person's answer in one fixed buffer, the answer's delivery to the question's endpoint, and the
//! end. Pure and interface-neutral: the holder brings notices and carries out effects, and an
//! overlay shows [`Presented`]. `apps/orchbee/docs/machines.md#the-tui-as-a-hub-client` owns the
//! rules.

mod secret;
#[cfg(test)]
mod tests;

pub(crate) use secret::{Answer, SecretField};

use crate::machine_question::attributed;
use orchbee_machine_protocol::{
    Id, Prompt,
    hub::{AnswerEndpoint, LoginEnd, Question, QuestionKind},
};

/// The answer a trusted new host takes.
const YES: &[u8] = b"yes";

/// One question of one episode, so an act on an overlay painted for an earlier question changes
/// nothing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Token {
    login: Id,
    question: Id,
}

/// What the hub and the delivery report about the episodes this client claimed.
#[derive(Debug)]
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "only the Windows holder reaches a hub")
)]
pub(crate) enum Notice {
    /// This client claimed episode `login` of `machine`.
    Claimed {
        login: Id,
        machine: String,
    },
    Question(Question),
    Ended {
        login: Id,
        end: LoginEnd,
    },
    /// The answer to the question `token` names was taken, or why not.
    Delivered {
        token: Token,
        outcome: Delivery,
    },
    /// The link to the hub ended, and every episode on it with it.
    Lost,
}

/// How a delivery ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "only the Windows holder reaches a hub")
)]
pub(crate) enum Delivery {
    Taken,
    /// The answer held a character `ssh` cannot take; nothing was sent.
    Inadmissible,
    /// The endpoint was gone or refused the answer.
    Undelivered,
}

/// The person's act on an open question.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Act {
    /// Send what the field holds.
    Submit,
    /// Trust a new host: Orchbee answers `yes`.
    Trust,
    /// End the login.
    Cancel,
}

/// What the holder carries out.
#[derive(Debug)]
pub(crate) enum Effect {
    /// Write `answer` to the question's endpoint off the interface thread, then report
    /// [`Notice::Delivered`]; the answer is wiped after the receipt or the 5 s limit.
    Deliver {
        token: Token,
        endpoint: AnswerEndpoint,
        answer: Answer,
    },
    /// Ask the hub to dismiss the episode.
    Dismiss(Id),
}

/// How the last episode of a Machine ended, as its selection reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ending {
    /// The hub ended it as it says.
    Hub(LoginEnd),
    /// The person dismissed it.
    Dismissed,
    /// An answer did not reach `ssh`, so the episode was dismissed.
    Undelivered,
    /// The link to the hub ended.
    Lost,
}

/// Why an overlay speaks up beside the question.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AskNotice {
    /// `ssh` refused the previous answer of this kind.
    Repeated,
    /// The answer held a character `ssh` cannot take; the same question asks again.
    Inadmissible,
}

/// What an overlay shows of the open question. No member holds an answer: the field is a handle on
/// the fixed buffer, which every copy shares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Presented {
    pub(crate) token: Token,
    pub(crate) machine: String,
    pub(crate) target: String,
    /// The kind as this client reads it: a new host or a password naming another host than the
    /// Machine's is `other`.
    pub(crate) kind: QuestionKind,
    pub(crate) prompt: Prompt,
    pub(crate) notice: Option<AskNotice>,
    /// The answer is on its way or was taken; the next question or the end follows.
    pub(crate) sending: bool,
    pub(crate) field: SecretField,
}

struct Open {
    question: Id,
    kind: QuestionKind,
    prompt: Prompt,
    answer: AnswerEndpoint,
    notice: Option<AskNotice>,
    sending: bool,
    field: SecretField,
}

struct Episode {
    login: Id,
    machine: String,
    target: String,
    open: Option<Open>,
}

impl Episode {
    fn token(&self) -> Option<Token> {
        self.open.as_ref().map(|open| Token {
            login: self.login,
            question: open.question,
        })
    }
}

impl Drop for Episode {
    fn drop(&mut self) {
        if let Some(open) = &self.open {
            open.field.clear();
        }
    }
}

/// The client's login episodes: at most one, claimed for the person's selection, and how the last
/// one of each Machine ended.
#[derive(Default)]
pub(crate) struct Logins {
    episode: Option<Episode>,
    endings: Vec<(String, Ending)>,
}

impl Logins {
    /// Applies `notice`; `target` names the `ssh` target of a Machine this client knows.
    pub(crate) fn notice(
        &mut self,
        notice: Notice,
        target: impl Fn(&str) -> Option<String>,
    ) -> Vec<Effect> {
        match notice {
            Notice::Claimed { login, machine } => {
                if let Some(previous) = self.episode.take() {
                    self.end(previous, Ending::Lost);
                }
                // A Machine this client cannot name is not shown as anyone's: let it go.
                let Some(target) = target(&machine) else {
                    return vec![Effect::Dismiss(login)];
                };
                self.episode = Some(Episode {
                    login,
                    machine,
                    target,
                    open: None,
                });
                Vec::new()
            }
            Notice::Question(question) => {
                let Some(episode) = self
                    .episode
                    .as_mut()
                    .filter(|episode| episode.login == question.login)
                else {
                    return Vec::new();
                };
                // The client reads the kind again: only `ssh`'s own sentence naming the Machine's
                // host is a password or a new host; anything else is asked as `other`.
                let kind = attributed(question.kind, question.prompt.as_str(), &episode.target);
                if let Some(previous) = episode.open.take() {
                    previous.field.clear();
                }
                episode.open = Some(Open {
                    question: question.question,
                    kind,
                    prompt: question.prompt,
                    answer: question.answer,
                    notice: question.repeated.then_some(AskNotice::Repeated),
                    sending: false,
                    field: SecretField::new(),
                });
                Vec::new()
            }
            Notice::Ended { login, end } => {
                if let Some(episode) = self.episode.take_if(|episode| episode.login == login) {
                    self.end(episode, Ending::Hub(end));
                }
                Vec::new()
            }
            Notice::Delivered { token, outcome } => {
                let Some(episode) = self
                    .episode
                    .as_mut()
                    .filter(|episode| episode.token() == Some(token))
                else {
                    return Vec::new();
                };
                let Some(open) = episode.open.as_mut().filter(|open| open.sending) else {
                    return Vec::new();
                };
                match outcome {
                    // The next question or the end follows.
                    Delivery::Taken => Vec::new(),
                    // Nothing was sent: the same question asks again.
                    Delivery::Inadmissible => {
                        open.sending = false;
                        open.notice = Some(AskNotice::Inadmissible);
                        open.field.clear();
                        Vec::new()
                    }
                    // The question closed without the answer: the login stops.
                    Delivery::Undelivered => {
                        let login = episode.login;
                        if let Some(episode) = self.episode.take() {
                            self.end(episode, Ending::Undelivered);
                        }
                        vec![Effect::Dismiss(login)]
                    }
                }
            }
            Notice::Lost => {
                if let Some(episode) = self.episode.take() {
                    self.end(episode, Ending::Lost);
                }
                Vec::new()
            }
        }
    }

    /// Applies the person's `act` on the question `token` names. Cancelling acts on the episode
    /// whichever of its questions the overlay showed; the others act only on the open question
    /// while it waits for an answer.
    pub(crate) fn act(&mut self, token: Token, act: Act) -> Vec<Effect> {
        let Some(episode) = self.episode.as_mut() else {
            return Vec::new();
        };
        if act == Act::Cancel {
            return if episode.login == token.login {
                self.dismiss()
            } else {
                Vec::new()
            };
        }
        if episode.token() != Some(token) {
            return Vec::new();
        }
        let Some(open) = episode.open.as_mut().filter(|open| !open.sending) else {
            return Vec::new();
        };
        let answer = match (act, &open.kind) {
            (Act::Trust, QuestionKind::HostKey { .. }) => Answer::fixed(YES),
            (Act::Submit, kind) if !matches!(kind, QuestionKind::HostKey { .. }) => {
                open.field.take()
            }
            _ => return Vec::new(),
        };
        open.sending = true;
        open.notice = None;
        vec![Effect::Deliver {
            token,
            endpoint: open.answer.clone(),
            answer,
        }]
    }

    /// The person's selection of `machine` ended before its login: the episode is dismissed.
    pub(crate) fn abandon(&mut self, machine: &str) -> Vec<Effect> {
        if self.active(machine) {
            self.dismiss()
        } else {
            Vec::new()
        }
    }

    /// Wipes what the open question's field holds, as a resize or a lost focus does.
    pub(crate) fn clear_field(&self) {
        if let Some(open) = self.episode.as_ref().and_then(|e| e.open.as_ref()) {
            open.field.clear();
        }
    }

    /// The open question, while one is shown.
    pub(crate) fn presented(&self) -> Option<Presented> {
        let episode = self.episode.as_ref()?;
        let open = episode.open.as_ref()?;
        Some(Presented {
            token: Token {
                login: episode.login,
                question: open.question,
            },
            machine: episode.machine.clone(),
            target: episode.target.clone(),
            kind: open.kind.clone(),
            prompt: open.prompt.clone(),
            notice: open.notice,
            sending: open.sending,
            field: open.field.clone(),
        })
    }

    /// Whether an episode of `machine` is claimed here: its selection waits for the person.
    pub(crate) fn active(&self, machine: &str) -> bool {
        self.episode.as_ref().is_some_and(|e| e.machine == machine)
    }

    /// How the last episode of `machine` ended, once.
    pub(crate) fn take_ending(&mut self, machine: &str) -> Option<Ending> {
        let index = self.endings.iter().position(|(m, _)| m == machine)?;
        Some(self.endings.swap_remove(index).1)
    }

    /// Ends the episode as the person's and asks the hub to dismiss it.
    fn dismiss(&mut self) -> Vec<Effect> {
        let Some(episode) = self.episode.take() else {
            return Vec::new();
        };
        let login = episode.login;
        self.end(episode, Ending::Dismissed);
        vec![Effect::Dismiss(login)]
    }

    fn end(&mut self, episode: Episode, ending: Ending) {
        self.endings
            .retain(|(machine, _)| *machine != episode.machine);
        self.endings.push((episode.machine.clone(), ending));
        drop(episode);
    }
}
