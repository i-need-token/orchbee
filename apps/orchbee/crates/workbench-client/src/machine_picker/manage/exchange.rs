//! Carries what the person asked of a saved Machine to discovery and the machines store, once per
//! shell tick: the add and its checks, rename, enable and disable, disconnect, remove, and the
//! stop of a remote Session before its record goes. A record changes only through
//! `Store::commit`; the catalog a commit returns goes back to the shell, which refreshes the
//! picker from it at once.

use super::{AddForm, Check, Notice, Page, Problem, Work};
use crate::machine_add::{Proposal, Stage};
use crate::machine_discovery::{Adding, Discovery, SessionStop};
use crate::machine_picker::Picker;
use crate::machines::{Catalog, Draft, Edit, ProfileId, Store};
use std::{sync::Arc, time::Instant};

/// What one exchange changed.
#[derive(Debug, Default)]
pub struct Managed {
    pub redraw: bool,
    /// The catalog a commit left, for the shell's catalog refresh.
    pub catalog: Option<Catalog>,
}

impl Picker {
    /// Whether the next exchange may write the store, so the shell hands it one.
    pub fn manage_wants_store(&self) -> bool {
        !self.manage.work.is_empty()
            || self.manage.stopping.is_some()
            || matches!(&self.manage.page, Page::Add(form) if form.check.is_some())
    }

    /// Carries the person's requests to `discovery` and `store`, and shows how they went.
    pub fn exchange_manage(
        &mut self,
        discovery: &mut Discovery,
        store: Option<&Store>,
        now: Instant,
    ) -> Managed {
        let mut managed = Managed::default();
        let explanations = discovery.explanations(now);
        if explanations != self.manage.explanations {
            self.manage.explanations = explanations;
            managed.redraw = true;
        }
        if let Page::Add(form) = &mut self.manage.page
            && !form.suggestions_read
        {
            form.suggestions = suggestions();
            form.suggestions_read = true;
            managed.redraw = true;
        }
        // A stop asked earlier settles before anything newer is asked.
        if let Some(machine) = self.manage.stopping.clone()
            && let Some(stop) = discovery.stopped(&machine)
        {
            self.manage.stopping = None;
            let label = self.manage.label(&machine);
            match stop {
                SessionStop::Stopped => {
                    if let Some(catalog) = self.remove(discovery, store, &machine) {
                        managed.catalog = Some(catalog);
                        self.manage.notice = Some(Notice::RemovedStopped(label));
                    }
                }
                SessionStop::NotOnline => self.manage.notice = Some(Notice::NotOnline(label)),
                SessionStop::Failed => self.manage.notice = Some(Notice::StopFailed(label)),
            }
            if matches!(&self.manage.page, Page::Confirm { machine: shown } if *shown == machine) {
                self.manage.set_page(Page::List);
            }
            managed.redraw = true;
        }
        for work in std::mem::take(&mut self.manage.work) {
            managed.redraw = true;
            match work {
                Work::Add { label, target } => self.start_add(discovery, store, &label, &target),
                Work::Discard => discovery.discard_add(),
                Work::Rename { machine, label } => {
                    managed.catalog = self
                        .commit(store, &Edit::Rename { id: machine, label })
                        .or(managed.catalog);
                }
                Work::Enable { machine, enabled } => {
                    // Disabling also ends the link, so nothing connects it again.
                    if !enabled {
                        discovery.disconnect(&machine);
                    }
                    managed.catalog = self
                        .commit(
                            store,
                            &Edit::Enable {
                                id: machine,
                                enabled,
                            },
                        )
                        .or(managed.catalog);
                }
                Work::Disconnect(machine) => discovery.disconnect(&machine),
                Work::Remove(machine) => {
                    let label = self.manage.label(&machine);
                    if let Some(catalog) = self.remove(discovery, store, &machine) {
                        managed.catalog = Some(catalog);
                        self.manage.notice = Some(Notice::Removed(label));
                    }
                }
                Work::Stop(machine) => discovery.stop_session(&machine),
            }
        }
        if let Some(catalog) = self.follow_add(discovery, store) {
            managed.catalog = Some(catalog);
            managed.redraw = true;
        } else if matches!(&self.manage.page, Page::Add(form) if form.check.is_some()) {
            managed.redraw = true;
        }
        managed
    }

    fn commit(&mut self, store: Option<&Store>, edit: &Edit) -> Option<Catalog> {
        match store.map(|store| store.commit(edit)) {
            Some(Ok((catalog, _))) => Some(catalog),
            _ => {
                self.manage.notice = Some(Notice::Failed(Problem::Save));
                None
            }
        }
    }

    /// Removes `machine`'s record and forgets its remembered answers with it.
    fn remove(
        &mut self,
        discovery: &Discovery,
        store: Option<&Store>,
        machine: &ProfileId,
    ) -> Option<Catalog> {
        let catalog = self.commit(store, &Edit::Remove(machine.clone()))?;
        discovery.forget_removed(machine);
        Some(catalog)
    }

    fn start_add(
        &mut self,
        discovery: &mut Discovery,
        store: Option<&Store>,
        label: &str,
        target: &str,
    ) {
        let proposal = Draft::parse(label, target, super::SESSION)
            .ok_or(None)
            .and_then(|draft| {
                let catalog = store
                    .ok_or(Some(Problem::Save))?
                    .read()
                    .map_err(|_| Some(Problem::Save))?;
                Proposal::new(draft, &catalog).map_err(|_| Some(Problem::Save))
            });
        let Page::Add(form) = &mut self.manage.page else {
            return;
        };
        match proposal {
            // A fresh id each time: a retry never reuses the id of the add it replaces.
            Ok(proposal) => discovery.add(proposal),
            Err(problem) => {
                form.check = None;
                form.problem = Some(problem.unwrap_or(Problem::Address));
            }
        }
    }

    /// Follows the add the form started; once every check passed, saves its record and returns
    /// the catalog, with the new Machine selected in the list.
    fn follow_add(&mut self, discovery: &mut Discovery, store: Option<&Store>) -> Option<Catalog> {
        let Page::Add(form) = &mut self.manage.page else {
            return None;
        };
        if !matches!(form.check, Some(Check::Starting | Check::Checking(_))) {
            return None;
        }
        match discovery.adding()? {
            Adding::Checking(stage) => {
                form.check = Some(Check::Checking(stage));
                None
            }
            Adding::Failed {
                stage, explanation, ..
            } => {
                form.check = Some(failed(stage, explanation));
                None
            }
            Adding::Passed => {
                form.check = Some(Check::Saving);
                let proposal = discovery.proposal()?.clone();
                match store.map(|store| proposal.save(store)) {
                    Some(Ok((catalog, _))) => {
                        discovery.add_saved();
                        self.manage.select = Some(proposal.id);
                        self.manage.add_focused = false;
                        self.manage.set_page(Page::List);
                        Some(catalog)
                    }
                    _ => {
                        discovery.discard_add();
                        if let Page::Add(form) = &mut self.manage.page {
                            form.check = None;
                            form.problem = Some(Problem::Save);
                        }
                        None
                    }
                }
            }
        }
    }
}

/// A failed add as the form shows it: Enter retries unless the design offers no way on, as for
/// a changed host key or a system Orchbee cannot run on.
fn failed(stage: Option<Stage>, explanation: crate::machine_status::Explanation) -> Check {
    let retry = explanation.action.is_some() || matches!(stage, None | Some(Stage::Reach));
    Check::Failed {
        stage,
        explanation,
        retry,
    }
}

/// The SSH config hosts the address can be chosen from, read once as the form opens: the bounded
/// read of aliases only. A unit test never reads the person's configuration; no application test
/// opens the form.
fn suggestions() -> Arc<[String]> {
    #[cfg(not(test))]
    {
        crate::machine_ssh_config::ssh_home()
            .map(|home| crate::machine_ssh_config::host_suggestions(&home))
            .unwrap_or_default()
            .into()
    }
    #[cfg(test)]
    {
        Arc::from([])
    }
}

impl AddForm {
    /// The suggestions a test offers in place of the person's configuration.
    #[cfg(test)]
    pub(crate) fn offer(&mut self, hosts: &[&str]) {
        self.suggestions = hosts.iter().map(|host| (*host).to_owned()).collect();
        self.suggestions_read = true;
    }
}
