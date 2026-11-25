//! Add Machine for the desktop page, as the TUI's form adds: the address reads as a machine named
//! by its host, the eight checks run on one draft connection through the hub, a failed check names
//! itself and gives the add up, and the record is saved once every check passed. The page keeps
//! what is typed; this keeps how the add stands and the SSH config hosts offered for the address.
//
// module-layer: workflow

use crate::{
    machine_add::{Proposal, Stage},
    machine_color::MachineColor,
    machine_discovery::{Adding, Discovery},
    machine_picker::ManageProblem,
    machine_ssh_config::HostRead,
    machine_status::Explanation,
    machines::{Catalog, Draft, ProfileId, Store, added_label, valid_target},
    ui_wakeup::UiWakeup,
};
use std::{sync::Arc, time::Instant};

/// The session a new machine's Workspaces run in, as the TUI's form saves it.
const SESSION: &str = "default";

/// How the add in progress stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Check {
    /// Asked for; discovery has not reported a check yet.
    Starting,
    Checking(Stage),
    Failed {
        stage: Option<Stage>,
        explanation: Explanation,
        /// Add asks again; the design offers no way past a changed host key or an unsupported
        /// system until the address changes.
        retry: bool,
    },
    /// Every check passed and the record is being saved.
    Saving,
}

/// The form the page shows while it is open.
pub(super) struct Add {
    /// The SSH config hosts the address can be chosen from, once read.
    pub(super) hosts: Arc<[String]>,
    reading: Option<HostRead>,
    pub(super) check: Option<Check>,
    pub(super) problem: Option<ManageProblem>,
    /// The host name the add in progress will be saved with, which its login and lines name.
    pub(super) label: String,
    /// The machine the add saved, until the page closes the form.
    pub(super) saved: Option<ProfileId>,
    /// The color the form offers first: the one the fewest saved machines use.
    pub(super) color: MachineColor,
}

impl Add {
    /// The form as it opens: the hosts are read once, off this thread. A unit test never reads
    /// the person's configuration.
    pub(super) fn open(now: Instant, wakeup: Option<&UiWakeup>, color: MachineColor) -> Self {
        #[cfg(not(test))]
        let home = crate::machine_ssh_config::ssh_home();
        #[cfg(test)]
        let home = None;
        Self {
            hosts: Arc::from([]),
            reading: Some(HostRead::start(home, now, wakeup.cloned())),
            check: None,
            problem: None,
            label: String::new(),
            saved: None,
            color,
        }
    }

    /// Whether the checks run, so the fields wait and Cancel gives the add up.
    pub(super) const fn running(&self) -> bool {
        matches!(
            self.check,
            Some(Check::Starting | Check::Checking(_) | Check::Saving)
        )
    }

    /// Starts adding `target` in `color`, named by its host, once it reads as a machine; a fresh
    /// id each time, so a retry never reuses the id of the add it replaces. The name is changed on
    /// the machine's page once it is saved.
    pub(super) fn start(
        &mut self,
        discovery: &mut Discovery,
        store: Option<&Store>,
        target: &str,
        color: Option<MachineColor>,
    ) {
        if self.running() || self.saved.is_some() {
            return;
        }
        self.check = None;
        self.problem = None;
        let target = target.trim();
        if !valid_target(target) {
            self.problem = Some(ManageProblem::Address);
            return;
        }
        let Some(label) = added_label(target) else {
            self.problem = Some(ManageProblem::Name);
            return;
        };
        let proposal = Draft::parse(&label, target, SESSION)
            .ok_or(ManageProblem::Address)
            .and_then(|draft| {
                let catalog = store
                    .ok_or(ManageProblem::Save)?
                    .read()
                    .map_err(|_| ManageProblem::Save)?;
                let mut proposal =
                    Proposal::new(draft, &catalog).map_err(|_| ManageProblem::Save)?;
                proposal.color = Some(color.unwrap_or_else(|| catalog.default_color()));
                Ok(proposal)
            });
        match proposal {
            Ok(proposal) => {
                self.label = label;
                self.check = Some(Check::Starting);
                discovery.add(proposal);
            }
            Err(problem) => self.problem = Some(problem),
        }
    }

    /// Follows the hosts' read and the add in progress; once every check passed, saves the
    /// record and returns the catalog it left.
    pub(super) fn follow(
        &mut self,
        discovery: &mut Discovery,
        store: Option<&Store>,
        now: Instant,
    ) -> Option<Catalog> {
        if let Some(hosts) = self.reading.as_ref().and_then(|read| read.poll(now)) {
            self.hosts = hosts;
            self.reading = None;
        }
        if !matches!(self.check, Some(Check::Starting | Check::Checking(_))) {
            return None;
        }
        match discovery.adding()? {
            Adding::Checking(stage) => {
                self.check = Some(Check::Checking(stage));
                None
            }
            Adding::Failed {
                stage, explanation, ..
            } => {
                let retry =
                    explanation.action.is_some() || matches!(stage, None | Some(Stage::Reach));
                self.check = Some(Check::Failed {
                    stage,
                    explanation,
                    retry,
                });
                // A failed add is given up at once: its draft, link, login and remembered
                // answers go.
                discovery.discard_add();
                None
            }
            Adding::Passed => {
                self.check = Some(Check::Saving);
                let proposal = discovery.proposal()?.clone();
                if let Some(Ok((catalog, _))) = store.map(|store| proposal.save(store)) {
                    discovery.add_saved(catalog.revision);
                    self.saved = Some(proposal.id);
                    Some(catalog)
                } else {
                    discovery.discard_add();
                    self.check = None;
                    self.problem = Some(ManageProblem::Save);
                    None
                }
            }
        }
    }
}
