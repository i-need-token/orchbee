//! One login episode as the client that claimed it sees it: the questions the hub puts to it, the
//! person's answer in one fixed buffer, the answer's delivery to the question's endpoint, and the
//! end; and the command logins the hub offers to every interface that asks, until the person
//! accepts one. Pure and interface-neutral: the holder brings notices and carries out effects, an
//! overlay shows [`Presented`] and a notice [`Offered`].
//! `apps/orchbee/docs/machines.md#the-tui-as-a-hub-client` owns the rules.

mod offers;
mod remember;
mod secret;
#[cfg(test)]
mod tests;

pub(crate) use offers::Offered;
pub(crate) use remember::RememberBox;
pub(crate) use secret::{Answer, SecretField};

use crate::machine_credentials::{Remembering, SecretKind};
use offers::Offers;
use remember::Remember;

use crate::machine_question::attributed;
use orchbee_machine_protocol::{
    Id, Prompt,
    hub::{AnswerEndpoint, LoginEnd, Question, QuestionKind},
};

/// The answer a trusted new host takes, and the word its field must read.
pub(crate) const YES: &str = "yes";

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
    /// The hub offers command episode `login` of `machine` to every client that asks, with
    /// `waiters` commands waiting on it; again when it counts them again.
    Offered {
        login: Id,
        machine: String,
        waiters: u64,
    },
    /// Another client claimed episode `login`, or this client's claim did not take it.
    Taken {
        login: Id,
    },
    /// This client claimed episode `login` of `machine`; again when the hub counts its waiters.
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

/// The person's act on an open question, or on the overlay of a login that ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Act {
    /// Send what the field holds.
    Submit,
    /// Trust a new host: Orchbee answers `yes` itself.
    Trust,
    /// End the login.
    Cancel,
    /// Close the overlay of a login that ended without the person.
    Close,
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
    /// Write `answer`, which `ssh` took for the login that just succeeded, to the credential store
    /// as `machine`'s `kind` answer off the interface thread; it is wiped once written.
    Remember {
        machine: String,
        kind: SecretKind,
        answer: Answer,
    },
    /// Ask the hub to make the offered episode this client's.
    Claim(Id),
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
    /// `ssh` refused the remembered answer, which is deleted.
    RememberedRejected,
}

/// What an overlay shows of the open question, or of the last question of a login that ended
/// without the person. No member holds an answer: the field is a handle on the fixed buffer, which
/// every copy shares.
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
    /// How the login ended without the person's act. The overlay stays and shows it until the
    /// person closes it; the field was wiped and takes nothing.
    pub(crate) outcome: Option<Ending>,
    pub(crate) field: SecretField,
    /// The remember box beside the field.
    pub(crate) remember: RememberBox,
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
    /// The person's choice to remember, and the answers kept until the login succeeds.
    remember: Remember,
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

/// The last question shown of a login that ended without the person's act.
struct Concluded {
    token: Token,
    machine: String,
    target: String,
    kind: QuestionKind,
    prompt: Prompt,
    ending: Ending,
    /// Always empty: the overlay of an ended login takes no answer.
    field: SecretField,
}

/// The client's login episodes: at most one, claimed for the selection the person waits on or
/// accepted from its notice, the overlay of one that ended without them, how the last one of each
/// Machine ended, and the command logins offered here.
#[derive(Default)]
pub(crate) struct Logins {
    /// The Machine whose selection the person waits on; only its login, and one the person
    /// accepted from its notice, may ask.
    waiting: Option<String>,
    episode: Option<Episode>,
    concluded: Option<Concluded>,
    endings: Vec<(String, Ending)>,
    /// How long this platform's store keeps an answer, once the holder looked.
    remembering: Option<Remembering>,
    offers: Offers,
}

impl Logins {
    /// Applies `notice`; `target` names the `ssh` target of a Machine this client knows.
    pub(crate) fn notice(
        &mut self,
        notice: Notice,
        target: impl Fn(&str) -> Option<String>,
    ) -> Vec<Effect> {
        match notice {
            Notice::Offered {
                login,
                machine,
                waiters,
            } => {
                // A Machine this client cannot name is not offered here.
                if let Some(target) = target(&machine).filter(|_| !self.holds(login)) {
                    self.offers.offer(login, machine, target, waiters);
                }
                Vec::new()
            }
            Notice::Taken { login } => {
                self.offers.withdraw(login);
                Vec::new()
            }
            // The hub counted the waiters again: the episode is already this client's.
            Notice::Claimed { login, .. } if self.holds(login) => Vec::new(),
            Notice::Claimed { login, machine } => {
                // Only the selection the person waits on, or a login the person accepted from its
                // notice, may ask, and a shown overlay is never replaced: any other login is let
                // go before it asks anything here.
                let accepted = self.offers.accepted(login);
                if !accepted {
                    self.offers.withdraw(login);
                }
                let admitted = (accepted || self.waiting.as_deref() == Some(machine.as_str()))
                    && self.concluded.is_none()
                    && self
                        .episode
                        .as_ref()
                        .is_none_or(|episode| episode.open.is_none());
                // A Machine this client cannot name is not shown as anyone's: let it go too.
                let Some(target) = target(&machine).filter(|_| admitted) else {
                    self.offers.withdraw(login);
                    return vec![Effect::Dismiss(login)];
                };
                let mut effects = Vec::new();
                if let Some(previous) = self.episode.take() {
                    effects.push(Effect::Dismiss(previous.login));
                    self.end(previous, Ending::Lost);
                }
                self.episode = Some(Episode {
                    login,
                    machine,
                    target,
                    open: None,
                    remember: Remember::default(),
                });
                effects
            }
            Notice::Question(question) => {
                let Some(episode) = self
                    .episode
                    .as_mut()
                    .filter(|episode| episode.login == question.login)
                else {
                    return Vec::new();
                };
                // An accepted login's notice gives way to its question.
                self.offers.withdraw(question.login);
                // The client reads the kind again: only `ssh`'s own sentence naming the Machine's
                // host is a password or a new host; anything else is asked as `other`.
                let kind = attributed(question.kind, question.prompt.as_str(), &episode.target);
                if let Some(previous) = episode.open.take() {
                    previous.field.clear();
                }
                // `ssh` asking this kind again refused the answer kept for it.
                episode.remember.refused(&kind);
                let notice = if question.remembered_rejected {
                    Some(AskNotice::RememberedRejected)
                } else {
                    question.repeated.then_some(AskNotice::Repeated)
                };
                episode.open = Some(Open {
                    question: question.question,
                    kind,
                    prompt: question.prompt,
                    answer: question.answer,
                    notice,
                    sending: false,
                    field: SecretField::new(),
                });
                Vec::new()
            }
            Notice::Ended { login, end } => {
                self.offers.withdraw(login);
                let Some(mut episode) = self.episode.take_if(|episode| episode.login == login)
                else {
                    return Vec::new();
                };
                // Only a login that succeeded writes what was kept; any other end wipes it.
                let effects = if end == LoginEnd::Succeeded {
                    episode.remember.saved(&episode.machine)
                } else {
                    Vec::new()
                };
                self.conclude(episode, Ending::Hub(end));
                effects
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
                            self.conclude(episode, Ending::Undelivered);
                        }
                        vec![Effect::Dismiss(login)]
                    }
                }
            }
            Notice::Lost => {
                self.offers.clear();
                if let Some(episode) = self.episode.take() {
                    self.conclude(episode, Ending::Lost);
                }
                Vec::new()
            }
        }
    }

    /// Applies the person's `act` on the question `token` names. Cancelling acts on the episode
    /// whichever of its questions the overlay showed; submitting and trusting act only on the open
    /// question while it waits for an answer; any end of an ended login's overlay closes it.
    pub(crate) fn act(&mut self, token: Token, act: Act) -> Vec<Effect> {
        if self
            .concluded
            .as_ref()
            .is_some_and(|concluded| concluded.token == token)
        {
            if matches!(act, Act::Close | Act::Cancel) {
                self.concluded = None;
            }
            return Vec::new();
        }
        let Some(episode) = self.episode.as_mut() else {
            return Vec::new();
        };
        match act {
            Act::Close => return Vec::new(),
            Act::Cancel => {
                return if episode.login == token.login {
                    self.dismiss()
                } else {
                    Vec::new()
                };
            }
            Act::Submit | Act::Trust => {}
        }
        if episode.token() != Some(token) {
            return Vec::new();
        }
        let Some(open) = episode.open.as_mut().filter(|open| !open.sending) else {
            return Vec::new();
        };
        let remember = &mut episode.remember;
        let answer = match (act, &open.kind) {
            // The overlay trusts only once the field reads `yes`; what it held is not sent.
            (Act::Trust, QuestionKind::HostKey { .. }) => {
                open.field.clear();
                Answer::fixed(YES.as_bytes())
            }
            (Act::Submit, kind) if !matches!(kind, QuestionKind::HostKey { .. }) => {
                let answer = open.field.take();
                remember.keep(kind, &answer, self.remembering);
                answer
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

    /// The person ticked or cleared the remember box of the question `token` names; it holds for
    /// the episode's later questions.
    pub(crate) fn remember(&mut self, token: Token, ticked: bool) {
        if let Some(episode) = self
            .episode
            .as_mut()
            .filter(|episode| episode.token() == Some(token))
        {
            episode.remember.choose(ticked);
        }
    }

    /// How long this platform's store keeps an answer, once the holder said.
    pub(crate) const fn remembering(&self) -> Option<Remembering> {
        self.remembering
    }

    /// How long this platform's store keeps an answer, as the holder found it.
    pub(crate) const fn set_remembering(&mut self, remembering: Remembering) {
        self.remembering = Some(remembering);
    }

    /// The person's selection now waits on `machine`, or on no Machine. A selection that stops
    /// waiting abandons its Machine's login: one that asked nothing yet is dismissed, and a shown
    /// question's overlay stays with that end until the person closes it. A later question of an
    /// abandoned login is dismissed on arrival.
    pub(crate) fn wait_for(&mut self, machine: Option<&str>) -> Vec<Effect> {
        if self.waiting.as_deref() == machine {
            return Vec::new();
        }
        let effects = match self.waiting.take() {
            Some(previous) => self.abandon(&previous),
            None => Vec::new(),
        };
        self.waiting = machine.map(str::to_owned);
        effects
    }

    /// The person's selection of `machine` ended before its login did.
    pub(crate) fn abandon(&mut self, machine: &str) -> Vec<Effect> {
        if self.waiting.as_deref() == Some(machine) {
            self.waiting = None;
        }
        let Some(episode) = self.episode.take_if(|episode| episode.machine == machine) else {
            return Vec::new();
        };
        let login = episode.login;
        if episode.open.as_ref().is_some_and(|open| open.sending) {
            // An answered question refuses every key after its Enter, as success does: its
            // overlay closes with the selection, which may have ended because the Machine came
            // online before the hub's end arrived.
            self.end(episode, Ending::Dismissed);
        } else {
            self.conclude(episode, Ending::Dismissed);
        }
        vec![Effect::Dismiss(login)]
    }

    /// The open question, or the last one of a login that ended without the person, while its
    /// overlay is shown.
    pub(crate) fn presented(&self) -> Option<Presented> {
        if let Some(concluded) = &self.concluded {
            return Some(Presented {
                token: concluded.token,
                machine: concluded.machine.clone(),
                target: concluded.target.clone(),
                kind: concluded.kind.clone(),
                prompt: concluded.prompt.clone(),
                notice: None,
                sending: false,
                outcome: Some(concluded.ending),
                field: concluded.field.clone(),
                remember: RememberBox::Absent,
            });
        }
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
            outcome: None,
            field: open.field.clone(),
            remember: episode.remember.shown(&open.kind, self.remembering),
        })
    }

    /// The notice of a command login offered here, while no question or ended login's overlay is
    /// shown and no other login is this client's: the accepted login until its question opens,
    /// otherwise the lowest one offered.
    pub(crate) fn offered(&self) -> Option<Offered> {
        if self.concluded.is_some() {
            return None;
        }
        match &self.episode {
            Some(episode) if episode.open.is_some() => None,
            Some(episode) => self
                .offers
                .get(episode.login)
                .filter(|offer| offer.accepted)
                .cloned(),
            None => self.offers.first().cloned(),
        }
    }

    /// The person accepts the notice of `login`: this client claims it, and its question follows.
    pub(crate) fn accept(&mut self, login: Id) -> Vec<Effect> {
        if self.offered().is_some_and(|offer| offer.login == login) && self.offers.accept(login) {
            vec![Effect::Claim(login)]
        } else {
            Vec::new()
        }
    }

    /// The person declines the notice of `login`: the hub ends it for every client, fails its
    /// commands and stops offering that Machine's command logins.
    pub(crate) fn decline(&mut self, login: Id) -> Vec<Effect> {
        if self.offered().is_none_or(|offer| offer.login != login) {
            return Vec::new();
        }
        self.offers.withdraw(login);
        if self.holds(login) {
            return self.dismiss();
        }
        vec![Effect::Dismiss(login)]
    }

    /// Whether episode `login` is this client's.
    fn holds(&self, login: Id) -> bool {
        self.episode
            .as_ref()
            .is_some_and(|episode| episode.login == login)
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

    /// Ends the episode without the person's act. A shown question keeps its overlay, showing
    /// `ending`, until the person closes it; only success after their own answer closes it at
    /// once, since every key after that answer's Enter is already refused.
    fn conclude(&mut self, episode: Episode, ending: Ending) {
        let answered = ending == Ending::Hub(LoginEnd::Succeeded);
        if let Some(open) = episode
            .open
            .as_ref()
            .filter(|open| !(answered && open.sending))
        {
            self.concluded = Some(Concluded {
                token: Token {
                    login: episode.login,
                    question: open.question,
                },
                machine: episode.machine.clone(),
                target: episode.target.clone(),
                kind: open.kind.clone(),
                prompt: open.prompt.clone(),
                ending,
                field: SecretField::new(),
            });
        }
        self.end(episode, ending);
    }

    fn end(&mut self, episode: Episode, ending: Ending) {
        self.endings
            .retain(|(machine, _)| *machine != episode.machine);
        self.endings.push((episode.machine.clone(), ending));
        drop(episode);
    }
}
