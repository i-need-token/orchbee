//! `machine connect --remember`: the password or passphrase typed at the console is kept, in a
//! buffer wiped when dropped, until the login ends; only a login that ended `succeeded` writes it
//! to the credential store. `ssh` asking the same kind again refused the kept answer, which goes.
//! When no store can be used here, or the local `ssh` is older than OpenSSH 8.5, the command says
//! why once and asks as without `--remember`. `docs/machines.md#remembered-answers` owns the rules.

use crate::cli_text::RememberedLine;
use orchbee_machine_protocol::{
    MachineId,
    hub::{LoginEnd, QuestionKind},
    wipe,
};
use orchbee_workbench_client::application::{
    machine_credentials::{Item, Remembering, SecretKind, Store, readable_label},
    machine_ssh,
};

/// The Machine whose answers `--remember` keeps, and the store they go to.
pub(super) struct Remember {
    store: Store,
    machine: MachineId,
    label: String,
    /// Why nothing can be remembered here; none when answers can be.
    unusable: Option<RememberedLine>,
}

impl Remember {
    /// Remembering for `machine`, saved as `label`; the store and the local `ssh` are asked now
    /// whether answers can be remembered.
    pub(super) fn new(machine: MachineId, label: &str) -> Self {
        let store = Store::select();
        Self {
            unusable: match machine_ssh::remembering(&store) {
                Remembering::Lasting | Remembering::UntilRestart => None,
                Remembering::Unavailable => Some(RememberedLine::Unavailable),
                Remembering::NeedsNewerSsh => Some(RememberedLine::NeedsNewerSsh),
            },
            store,
            machine,
            label: readable_label(label),
        }
    }

    const fn usable(&self) -> bool {
        self.unusable.is_none()
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

    /// A question of `kind` was asked: a kept answer of that kind was refused. Once, at the first
    /// question that could be remembered, why nothing can be remembered here, if so.
    pub(super) fn asked(&mut self, kind: &QuestionKind) -> Option<RememberedLine> {
        let kind = SecretKind::of(kind)?;
        self.answers.retain(|(kept, _)| *kept != kind);
        let line = self.remember.and_then(|remember| remember.unusable)?;
        if self.told {
            return None;
        }
        self.told = true;
        Some(line)
    }

    /// `answer` went to a question of `kind`: it is kept while remembering is asked for and usable.
    pub(super) fn delivered(&mut self, kind: &QuestionKind, answer: &[u8]) {
        let (Some(kind), Some(_)) = (
            SecretKind::of(kind),
            self.remember.filter(|remember| remember.usable()),
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
