//! What one uninstall run did: the `orchbee.uninstall` document, the step-a listing, the final
//! message, the diagnostic naming what was done and what remains, and the exit status.
use crate::{cli_text::CliText, localization::Locale, native_path};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The result of one removal or stop, as the document spells it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Outcome {
    NotReached,
    Absent,
    Removed,
    /// The store's last files go after this process exits.
    Scheduled,
    Failed,
}

impl Outcome {
    const fn token(self) -> &'static str {
        match self {
            Self::NotReached => "not-reached",
            Self::Absent => "absent",
            Self::Removed => "removed",
            Self::Scheduled => "scheduled",
            Self::Failed => "failed",
        }
    }

    pub(super) const fn settled(self) -> bool {
        matches!(self, Self::Absent | Self::Removed | Self::Scheduled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionOutcome {
    NotReached,
    Stopped,
    Failed,
}

pub(super) struct SessionLine {
    pub(super) name: String,
    pub(super) runtime: PathBuf,
    pub(super) terminals: Option<u32>,
    pub(super) agents: Option<u32>,
    /// The Server's product version when it is not this build's.
    pub(super) other_version: Option<String>,
    pub(super) pids: Vec<u32>,
    pub(super) outcome: SessionOutcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IntegrationOutcome {
    NotReached,
    Disabled,
    NotEnabled,
    /// The writer does not manage this provider on this platform; there is nothing to disable.
    NotManaged,
    /// The provider's program is not on `PATH`, so only that program could report an entry.
    ProgramNotFound,
    Failed,
}

pub(super) struct IntegrationLine {
    pub(super) provider: &'static str,
    /// Listed before confirming as enabled, or possibly enabled.
    pub(super) listed: bool,
    pub(super) outcome: IntegrationOutcome,
}

/// Why a run stopped; the closed `error` vocabulary of the document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stop {
    Location,
    InsideSession,
    Plan(&'static str),
    Inventory,
    TerminalsOpen,
    SessionUnstoppable,
    ProgramRunning,
    /// A program of a kind step b cannot end still runs from the store root.
    StillRunning,
    Sessions,
    Integrations,
    PathEntry,
    InstallerEntries,
    Data,
    Store,
    Interrupted,
}

impl Stop {
    const fn token(self) -> &'static str {
        match self {
            Self::Location => "location",
            Self::InsideSession => "inside-session",
            Self::Plan(_) => "plan",
            Self::Inventory => "inventory",
            Self::TerminalsOpen => "terminals-open",
            Self::SessionUnstoppable => "session-unstoppable",
            Self::ProgramRunning => "program-running",
            Self::StillRunning => "still-running",
            Self::Sessions => "sessions",
            Self::Integrations => "integrations",
            Self::PathEntry => "path-entry",
            Self::InstallerEntries => "installer-entries",
            Self::Data => "data",
            Self::Store => "store",
            Self::Interrupted => "interrupted",
        }
    }
}

pub(super) struct Report {
    pub(super) delete_data: bool,
    pub(super) installer: bool,
    pub(super) store_root: Option<PathBuf>,
    pub(super) data_root: Option<PathBuf>,
    pub(super) sessions: Vec<SessionLine>,
    pub(super) integrations: Vec<IntegrationLine>,
    pub(super) path_key: Option<String>,
    pub(super) path_entry: Outcome,
    pub(super) shortcut: Option<PathBuf>,
    pub(super) shortcut_outcome: Outcome,
    pub(super) registration: Option<String>,
    pub(super) registration_outcome: Outcome,
    pub(super) data: Vec<(PathBuf, Outcome)>,
    pub(super) store: Outcome,
    pub(super) remaining: Vec<PathBuf>,
    pub(super) npm_carrier: bool,
    pub(super) stop: Option<Stop>,
    /// The Session, provider, path or process the stop concerns, and a count, version or reason.
    pub(super) subject: String,
    pub(super) detail: String,
    /// Whether any step changed anything.
    pub(super) changed: bool,
    pub(super) interrupted: bool,
    /// The program a stop names to run again once `orchbee` may no longer be on `Path`.
    pub(super) again: Option<PathBuf>,
}

impl Report {
    pub(super) fn new(delete_data: bool, installer: bool) -> Self {
        Self {
            delete_data,
            installer,
            store_root: None,
            data_root: None,
            sessions: Vec::new(),
            integrations: Vec::new(),
            path_key: None,
            path_entry: Outcome::NotReached,
            shortcut: None,
            shortcut_outcome: Outcome::NotReached,
            registration: None,
            registration_outcome: Outcome::NotReached,
            data: Vec::new(),
            store: Outcome::NotReached,
            remaining: Vec::new(),
            npm_carrier: false,
            stop: None,
            subject: String::new(),
            detail: String::new(),
            changed: false,
            interrupted: false,
            again: None,
        }
    }

    /// Records why the run stopped and what the stop concerns.
    pub(super) fn stop(
        &mut self,
        stop: Stop,
        subject: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.stop = Some(stop);
        self.subject = subject.into();
        self.detail = detail.into();
    }

    /// The exit status by the output standard: 130 interrupted; 1 when the plan or the locations
    /// were refused; 3 when a person must act first and nothing changed; 9 when a step failed after
    /// the run had begun changing things; 0 otherwise.
    pub(super) fn status(&self) -> u8 {
        if self.interrupted {
            return 130;
        }
        match self.stop {
            None => 0,
            Some(Stop::Location | Stop::Plan(_) | Stop::Inventory) => 1,
            Some(
                Stop::InsideSession
                | Stop::TerminalsOpen
                | Stop::SessionUnstoppable
                | Stop::ProgramRunning,
            ) if !self.changed => 3,
            Some(_) => 9,
        }
    }

    pub(super) fn document(&self) -> Value {
        let path = |path: &Option<PathBuf>| path.as_deref().map(path_text);
        json!({
            "schema": "orchbee.uninstall",
            "version": 1,
            "result": {
                "mode": if self.delete_data { "delete-data" } else { "keep-data" },
                "caller": if self.installer { "installer" } else { "person" },
                "storeRoot": path(&self.store_root),
                "dataRoot": path(&self.data_root),
                "sessions": self.sessions.iter().map(|session| json!({
                    "name": session.name,
                    "terminals": session.terminals,
                    "agents": session.agents,
                    "otherVersion": session.other_version,
                    "result": match session.outcome {
                        SessionOutcome::NotReached => "not-reached",
                        SessionOutcome::Stopped => "stopped",
                        SessionOutcome::Failed => "failed",
                    },
                })).collect::<Vec<_>>(),
                "integrations": self.integrations.iter().map(|line| json!({
                    "provider": line.provider,
                    "listed": line.listed,
                    "result": match line.outcome {
                        IntegrationOutcome::NotReached => "not-reached",
                        IntegrationOutcome::Disabled => "disabled",
                        IntegrationOutcome::NotEnabled => "not-enabled",
                        IntegrationOutcome::NotManaged => "not-managed",
                        IntegrationOutcome::ProgramNotFound => "program-not-found",
                        IntegrationOutcome::Failed => "failed",
                    },
                })).collect::<Vec<_>>(),
                "pathEntry": {"key": self.path_key, "result": self.path_entry.token()},
                "installerEntries": [
                    {"entry": "start-menu-shortcut", "location": path(&self.shortcut),
                        "result": self.shortcut_outcome.token()},
                    {"entry": "uninstall-registration", "location": self.registration,
                        "result": self.registration_outcome.token()},
                ],
                "data": self.data.iter().map(|(path, outcome)| json!({
                    "path": path_text(path), "result": outcome.token(),
                })).collect::<Vec<_>>(),
                "store": self.store.token(),
                "remaining": self.remaining.iter().map(|path| path_text(path)).collect::<Vec<_>>(),
                "npmCarrier": self.npm_carrier,
                "error": self.stop.map(Stop::token),
                "reason": match self.stop {
                    Some(Stop::Plan(reason)) => Some(reason),
                    _ => None,
                },
                "interrupted": self.interrupted,
            },
        })
    }

    /// Step a: what goes and what stays, then the running Sessions with their open terminals and
    /// running Agents.
    pub(super) fn listing(&self, locale: Locale) -> String {
        let text = CliText::new(locale);
        let phrase = |key| text.cli_uninstall_phrase(key);
        let separator = phrase("separator");
        let mut lines = vec![phrase("listing").to_owned()];
        if let Some(store) = &self.store_root {
            lines.push(format!("  {}{}", phrase("program-folder"), display(store)));
        }
        if self.path_entry != Outcome::Absent
            && let Some(store) = &self.store_root
        {
            lines.push(format!(
                "  {}{}",
                phrase("path-entry"),
                display(&store.join("bin"))
            ));
        }
        let entries: Vec<&str> = [
            (self.shortcut_outcome, "start-menu-shortcut"),
            (self.registration_outcome, "uninstall-registration"),
        ]
        .into_iter()
        .filter(|(outcome, _)| *outcome != Outcome::Absent)
        .map(|(_, key)| phrase(key))
        .collect();
        if !entries.is_empty() {
            lines.push(format!(
                "  {}{}",
                phrase("installer-entries"),
                entries.join(separator)
            ));
        }
        let listed: Vec<&str> = self
            .integrations
            .iter()
            .filter(|line| line.listed)
            .map(|line| line.provider)
            .collect();
        lines.push(format!(
            "  {}{}",
            phrase("integrations"),
            if listed.is_empty() {
                phrase("integrations-none").to_owned()
            } else {
                listed.join(separator)
            }
        ));
        let data_root = self.data_root.as_deref().map(display).unwrap_or_default();
        if self.delete_data {
            lines.push(format!("  {}{data_root}", phrase("data-folder")));
        } else {
            let names: Vec<String> = super::plan::REMOVED_DATA
                .iter()
                .map(|parts| parts.join("\\"))
                .collect();
            lines.push(format!(
                "  {}{}",
                phrase("data-entries"),
                names.join(separator)
            ));
            lines.push(format!("{}{data_root}", phrase("keeps")));
        }
        if self.sessions.is_empty() {
            lines.push(phrase("no-sessions").to_owned());
        } else {
            lines.push(phrase("sessions").to_owned());
            for session in &self.sessions {
                lines.push(format!(
                    "  {}",
                    text.cli_uninstall_session(&session.name, session.terminals, session.agents)
                ));
            }
        }
        lines.join("\n") + "\n"
    }

    /// The final message after a complete run.
    pub(super) fn done(&self, locale: Locale) -> String {
        let text = CliText::new(locale);
        let phrase = |key| text.cli_uninstall_phrase(key);
        let separator = phrase("separator");
        let mut lines = vec![phrase("done").to_owned()];
        if let Some(store) = &self.store_root {
            match self.store {
                Outcome::Scheduled => {
                    lines.push(format!("{}{}", phrase("store-scheduled"), display(store)));
                }
                Outcome::Removed => {
                    lines.push(format!("{}{}", phrase("store-removed"), display(store)))
                }
                _ => {}
            }
        }
        let data_root = self.data_root.as_deref().map(display).unwrap_or_default();
        lines.push(if self.delete_data {
            format!("{}{data_root}", phrase("data-deleted"))
        } else {
            format!("{}{data_root}", phrase("keeps"))
        });
        for (outcome, key) in [
            (IntegrationOutcome::Disabled, "disabled"),
            (IntegrationOutcome::ProgramNotFound, "program-not-found"),
        ] {
            let providers: Vec<&str> = self
                .integrations
                .iter()
                .filter(|line| line.outcome == outcome)
                .map(|line| line.provider)
                .collect();
            if !providers.is_empty() {
                lines.push(format!("{}{}", phrase(key), providers.join(separator)));
            }
        }
        if self.path_entry == Outcome::Removed {
            lines.push(phrase("new-terminal").to_owned());
        }
        if self.npm_carrier {
            lines.push(phrase("npm").to_owned());
        }
        lines.join("\n") + "\n"
    }

    /// The diagnostic of a run that stopped: the cause, then what was done and what remains when
    /// anything changed, or that nothing was changed, and the way to continue.
    pub(super) fn diagnostic(&self, locale: Locale) -> String {
        let text = CliText::new(locale);
        let phrase = |key| text.cli_uninstall_phrase(key);
        let separator = phrase("separator");
        let Some(stop) = self.stop else {
            return String::new();
        };
        let cause_token = match stop {
            Stop::Plan(reason) => reason,
            other => other.token(),
        };
        let mut message = format!(
            "{}{}",
            phrase("stopped"),
            text.cli_uninstall_cause(cause_token, &self.subject, &self.detail)
        );
        if !self.changed {
            message.push_str(phrase("space"));
            message.push_str(phrase("nothing-changed"));
            return message + "\n";
        }
        let done: Vec<&str> = [
            (
                !self.sessions.is_empty()
                    && self
                        .sessions
                        .iter()
                        .all(|session| session.outcome == SessionOutcome::Stopped),
                "step-sessions",
            ),
            (
                !self.integrations.is_empty()
                    && self.integrations.iter().all(|line| {
                        !matches!(
                            line.outcome,
                            IntegrationOutcome::NotReached | IntegrationOutcome::Failed
                        )
                    }),
                "step-integrations",
            ),
            (
                self.path_entry.settled()
                    && self.shortcut_outcome.settled()
                    && self.registration_outcome.settled(),
                "step-entries",
            ),
            (
                !self.data.is_empty() && self.data.iter().all(|(_, outcome)| outcome.settled()),
                "step-data",
            ),
        ]
        .into_iter()
        .filter(|(done, _)| *done)
        .map(|(_, key)| phrase(key))
        .collect();
        let mut remains: Vec<String> = Vec::new();
        if self.integrations.iter().any(|line| {
            matches!(
                line.outcome,
                IntegrationOutcome::NotReached | IntegrationOutcome::Failed
            )
        }) {
            remains.push(phrase("remain-integrations").to_owned());
        }
        if !self.path_entry.settled() {
            remains.push(phrase("remain-path-entry").to_owned());
        }
        if !self.shortcut_outcome.settled() || !self.registration_outcome.settled() {
            remains.push(phrase("remain-installer-entries").to_owned());
        }
        // A path is named only when its removal failed; entries not yet reached are named by kind.
        remains.extend(
            self.data
                .iter()
                .filter(|(_, outcome)| *outcome == Outcome::Failed)
                .map(|(path, _)| display(path)),
        );
        if self
            .data
            .iter()
            .any(|(_, outcome)| *outcome == Outcome::NotReached)
        {
            remains.push(
                phrase(if self.delete_data {
                    "remain-data-folder"
                } else {
                    "remain-data"
                })
                .to_owned(),
            );
        }
        if !self.store.settled() {
            remains.push(phrase("remain-store").to_owned());
        }
        if !done.is_empty() {
            message.push_str(phrase("space"));
            message.push_str(phrase("was-done"));
            message.push_str(&done.join(separator));
            message.push_str(phrase("period"));
        }
        if !remains.is_empty() {
            message.push_str(phrase("space"));
            message.push_str(phrase("remains"));
            message.push_str(&remains.join(separator));
            message.push_str(phrase("period"));
        }
        // The full path matters once the store remains and the `Path` entry no longer leads to it.
        let program = (!self.store.settled() && self.path_entry.settled())
            .then_some(self.again.as_deref())
            .flatten()
            .map(display);
        message.push_str(phrase("space"));
        message.push_str(&text.cli_uninstall_again(program.as_deref()));
        message + "\n"
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(super) fn display(path: &Path) -> String {
    native_path::display(&path.to_string_lossy())
}
