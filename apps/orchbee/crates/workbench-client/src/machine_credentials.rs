//! Remembered login answers in the operating system's credential store: the one port every
//! caller uses, the item names the remembered-answer contract freezes, and the rule that selects
//! this platform's store. A store that cannot be used answers [`Remembering::Unavailable`], and
//! nothing ever falls back to a file. `apps/orchbee/docs/machines.md#remembered-answers` owns the
//! rules.
//!
//! An answer read here lives only in a [`Secret`], wiped when dropped; one written here is read
//! from the caller's buffer and leaves no copy behind. No failure carries the store's own error,
//! which may hold an answer's bytes, and nothing here logs.

#[cfg(windows)]
mod windows;

use orchbee_machine_protocol::{MachineId, hub::QuestionKind, wipe};
use std::fmt;

/// The longest readable label a store item carries, in characters.
const LABEL_CHARACTERS: usize = 120;

/// What a remembered answer answers. Only these two kinds are ever remembered.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecretKind {
    Password,
    KeyPassphrase,
}

impl SecretKind {
    /// Both kinds, in the order a forget or a removal deletes them.
    pub const ALL: [Self; 2] = [Self::Password, Self::KeyPassphrase];

    /// The kind's name in an item name: `password` or `key-passphrase`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::KeyPassphrase => "key-passphrase",
        }
    }

    /// The kind a question's answer is remembered as; `None` for a question whose answer is never
    /// remembered.
    pub const fn of(kind: &QuestionKind) -> Option<Self> {
        match kind {
            QuestionKind::Password => Some(Self::Password),
            QuestionKind::KeyPassphrase => Some(Self::KeyPassphrase),
            _ => None,
        }
    }
}

/// One remembered answer's place: this build's environment, the Machine's id and the kind. The
/// environment keeps a development or test build's items apart from a release's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    environment: String,
    machine: MachineId,
    kind: SecretKind,
}

impl Item {
    /// The item of `machine`'s `kind` answer in this build's environment.
    pub fn new(machine: MachineId, kind: SecretKind) -> Self {
        Self {
            environment: orchbee_local_runtime::environment_stem().to_owned(),
            machine,
            kind,
        }
    }

    /// The item in the named environment, for a test whose items must not be any build's.
    #[cfg(all(test, windows))]
    pub(crate) fn in_environment(environment: &str, machine: MachineId, kind: SecretKind) -> Self {
        Self {
            environment: environment.to_owned(),
            machine,
            kind,
        }
    }

    /// The Windows Credential Manager target name:
    /// `Orchbee/<environment>/machine/<machine id>/<kind>`.
    pub fn target_name(&self) -> String {
        format!(
            "Orchbee/{}/machine/{}/{}",
            self.environment,
            self.machine.as_str(),
            self.kind.name()
        )
    }
}

/// The readable label an item carries for a person who browses the store, written once when the
/// answer is saved: `Orchbee: <machine label>`, without control characters and bounded.
pub fn readable_label(machine_label: &str) -> String {
    let mut label = String::from("Orchbee: ");
    label.extend(
        machine_label
            .chars()
            .filter(|character| !character.is_control())
            .take(LABEL_CHARACTERS),
    );
    label
}

/// How long this platform's store keeps an answer, or that it cannot keep one here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Remembering {
    /// Until the person forgets it or removes the Machine.
    Lasting,
    /// Until this computer restarts: a store held only in memory.
    UntilRestart,
    /// No store can be used in this login, such as a process in an SSH logon.
    Unavailable,
}

/// Why a store call failed. It never carries the store's own error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// The store cannot be used in this login.
    Unavailable,
    /// The store refused the call or failed.
    Failed,
}

/// What a deletion found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Deleted {
    Removed,
    Absent,
}

/// An answer read from the store, wiped when dropped. It cannot be cloned or printed.
pub struct Secret(Vec<u8>);

impl Secret {
    #[cfg_attr(
        not(windows),
        expect(dead_code, reason = "only Windows has an adapter yet")
    )]
    fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret")
    }
}

/// One store's calls. Each platform's adapter implements it; callers reach it only through
/// [`Store`], so a new adapter changes no caller.
trait Adapter: Sync {
    /// How long the store keeps an answer; `probe` is a name no item has.
    fn remembering(&self, probe: &str) -> Remembering;
    fn read(&self, item: &Item) -> Result<Option<Secret>, Failure>;
    fn write(&self, item: &Item, secret: &[u8], label: &str) -> Result<(), Failure>;
    fn delete(&self, item: &Item) -> Result<Deleted, Failure>;
}

/// This platform's credential store, or none.
#[derive(Clone, Copy)]
pub struct Store {
    adapter: Option<&'static dyn Adapter>,
}

impl fmt::Debug for Store {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(if self.adapter.is_some() {
            "Store"
        } else {
            "Store(unavailable)"
        })
    }
}

impl Store {
    /// The selection rule: Windows Credential Manager on Windows. macOS and Linux have no adapter
    /// yet and answer unavailable; theirs join here (the login Keychain on macOS; on Linux the
    /// Secret Service, otherwise the kernel's per-user keyring) without changing any caller.
    pub fn select() -> Self {
        #[cfg(windows)]
        {
            Self {
                adapter: Some(&windows::CredentialManager),
            }
        }
        #[cfg(not(windows))]
        {
            Self::unavailable()
        }
    }

    /// A store that keeps nothing.
    pub const fn unavailable() -> Self {
        Self { adapter: None }
    }

    /// How long this store keeps an answer in this login. It reads one name in this build's
    /// environment that no item has; a store that cannot be reached answers unavailable.
    pub fn remembering(&self) -> Remembering {
        let Some(adapter) = self.adapter else {
            return Remembering::Unavailable;
        };
        let probe = format!(
            "Orchbee/{}/probe",
            orchbee_local_runtime::environment_stem()
        );
        adapter.remembering(&probe)
    }

    /// The answer remembered at `item`, if any.
    pub fn read(&self, item: &Item) -> Result<Option<Secret>, Failure> {
        self.adapter.ok_or(Failure::Unavailable)?.read(item)
    }

    /// Remembers `secret` at `item`, with `label` for a person who browses the store. It replaces
    /// what the item held.
    pub fn write(&self, item: &Item, secret: &[u8], label: &str) -> Result<(), Failure> {
        self.adapter
            .ok_or(Failure::Unavailable)?
            .write(item, secret, label)
    }

    /// Deletes what `item` holds; nothing is read first.
    pub fn delete(&self, item: &Item) -> Result<Deleted, Failure> {
        self.adapter.ok_or(Failure::Unavailable)?.delete(item)
    }

    /// Deletes both kinds of `machine`'s remembered answers; `Removed` when either was there.
    pub fn forget(&self, machine: &MachineId) -> Result<Deleted, Failure> {
        let mut found = Deleted::Absent;
        for kind in SecretKind::ALL {
            if self.delete(&Item::new(machine.clone(), kind))? == Deleted::Removed {
                found = Deleted::Removed;
            }
        }
        Ok(found)
    }
}

#[cfg(all(test, windows))]
mod tests;
