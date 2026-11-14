//! `machine connect --remember`: the password or passphrase typed at the console is kept, in a
//! buffer wiped when dropped, until the login ends; only a login that ended `succeeded` writes it
//! to the credential store. `ssh` asking the same kind again refused the kept answer, which goes.
//! When no store can be used here the command says so once and asks as without `--remember`.
//! `docs/machines.md#remembered-answers` owns the rules.

use orchbee_machine_protocol::{
    MachineId,
    hub::{LoginEnd, QuestionKind},
    wipe,
};
use orchbee_workbench_client::application::machine_credentials::{
    Item, Remembering, SecretKind, Store, readable_label,
};

/// The Machine whose answers `--remember` keeps, and the store they go to.
pub(super) struct Remember {
    store: Store,
    machine: MachineId,
    label: String,
    usable: bool,
}

impl Remember {
    /// Remembering for `machine`, saved as `label`; the store is asked now whether it can be used.
    pub(super) fn new(machine: MachineId, label: &str) -> Self {
        let store = Store::select();
        Self {
            usable: store.remembering() != Remembering::Unavailable,
            store,
            machine,
            label: readable_label(label),
        }
    }
}

/// One kept answer, wiped when dropped. It has no debug form.
struct Copy(Vec<u8>);

impl Drop for Copy {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// The answers one conversation keeps.
pub(super) struct Kept<'a> {
    remember: Option<&'a Remember>,
    answers: Vec<(SecretKind, Copy)>,
    /// The person was told this login cannot remember.
    told: bool,
}

impl<'a> Kept<'a> {
    pub(super) const fn new(remember: Option<&'a Remember>) -> Self {
        Self {
            remember,
            answers: Vec::new(),
            told: false,
        }
    }

    /// A question of `kind` was asked: a kept answer of that kind was refused. True once, at the
    /// first question that could be remembered, when no store can be used here.
    pub(super) fn asked(&mut self, kind: &QuestionKind) -> bool {
        let Some(kind) = SecretKind::of(kind) else {
            return false;
        };
        self.answers.retain(|(kept, _)| *kept != kind);
        let unusable = self.remember.is_some_and(|remember| !remember.usable);
        let tell = unusable && !self.told;
        self.told |= tell;
        tell
    }

    /// `answer` went to a question of `kind`: it is kept while remembering is asked for and usable.
    pub(super) fn delivered(&mut self, kind: &QuestionKind, answer: &[u8]) {
        let (Some(kind), Some(_)) = (
            SecretKind::of(kind),
            self.remember.filter(|remember| remember.usable),
        ) else {
            return;
        };
        self.answers.retain(|(kept, _)| *kept != kind);
        let mut copy = Copy(Vec::with_capacity(answer.len()));
        copy.0.extend_from_slice(answer);
        self.answers.push((kind, copy));
    }

    /// The login ended `end`: after `succeeded` every kept answer is written; each copy is wiped
    /// either way.
    pub(super) fn ended(&mut self, end: LoginEnd) {
        let answers = std::mem::take(&mut self.answers);
        let Some(remember) = self.remember.filter(|_| end == LoginEnd::Succeeded) else {
            return;
        };
        for (kind, answer) in answers {
            let item = Item::new(remember.machine.clone(), kind);
            let _written = remember.store.write(&item, &answer.0, &remember.label);
        }
    }
}
