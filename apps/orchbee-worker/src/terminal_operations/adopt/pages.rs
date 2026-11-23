//! Collects one adoption's AdoptHost pages until its last arrives. A page that does not continue
//! the pages before it is refused and changes nothing; it does not verify, dial or read a Host.

use std::{collections::BTreeSet, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptHostRequest, AdoptHostTarget, HostCreationId, HostEpoch, HostTerminalId,
    MAXIMUM_ADOPT_HOST_BINDINGS, OperationId, TerminalId,
};
use orchbee_local_worker_protocol::WorkerId;

/// The pages of the adoption in progress, if one has begun.
#[derive(Default)]
pub(super) struct Pages {
    pending: Option<Pending>,
}

struct Pending {
    adoption_id: OperationId,
    target: AdoptHostTarget,
    next: u16,
    bindings: Vec<AdoptBinding>,
    terminals: BTreeSet<TerminalId>,
    hosted: BTreeSet<(HostEpoch, HostTerminalId)>,
    creations: BTreeSet<(HostEpoch, HostCreationId)>,
}

/// A page that continued its adoption.
pub(super) enum Accepted {
    /// A page before the last, kept for the import.
    Page,
    /// The last page: the adoption's target and every page's bindings, in page order.
    Complete {
        target: AdoptHostTarget,
        bindings: Vec<AdoptBinding>,
    },
}

/// A page this Worker declines; the pages before it stay as they were.
pub(super) struct Refused;

impl Pages {
    /// Takes one page. Page 0 begins a new adoption, discarding one in progress; every later page
    /// must name the same adoption and target with the next index. Every page must name this
    /// Worker and generation, and all pages together stay within [`MAXIMUM_ADOPT_HOST_BINDINGS`]
    /// without naming a terminal twice, or a Host terminal or creation of one epoch twice.
    pub(super) fn accept(
        &mut self,
        request: AdoptHostRequest,
        worker_id: WorkerId,
        generation: NonZeroU64,
    ) -> Result<Accepted, Refused> {
        let key = request.key();
        if key.worker_id.as_bytes() != worker_id.as_bytes()
            || key.worker_generation.get() != generation.get()
        {
            return Err(Refused);
        }
        let next = if key.page.last {
            None
        } else {
            Some(key.page.index.checked_add(1).ok_or(Refused)?)
        };
        if key.page.index == 0 {
            let fresh = Pending {
                adoption_id: key.adoption_id,
                target: request.target().clone(),
                next: 0,
                bindings: Vec::new(),
                terminals: BTreeSet::new(),
                hosted: BTreeSet::new(),
                creations: BTreeSet::new(),
            };
            if !fresh.admits(request.bindings()) {
                return Err(Refused);
            }
            self.pending = Some(fresh);
        } else if !self.pending.as_ref().is_some_and(|pending| {
            pending.adoption_id == key.adoption_id
                && pending.target == *request.target()
                && pending.next == key.page.index
                && pending.admits(request.bindings())
        }) {
            return Err(Refused);
        }
        let Some(pending) = self.pending.as_mut() else {
            return Err(Refused);
        };
        for binding in request.bindings() {
            let epoch = binding.identity.host_epoch();
            pending.terminals.insert(binding.terminal_id);
            pending
                .hosted
                .insert((epoch, binding.identity.terminal_id()));
            pending
                .creations
                .insert((epoch, binding.identity.creation_id()));
        }
        pending.bindings.extend_from_slice(request.bindings());
        match next {
            Some(next) => {
                pending.next = next;
                Ok(Accepted::Page)
            }
            None => {
                let target = std::mem::take(&mut pending.target);
                let bindings = std::mem::take(&mut pending.bindings);
                self.pending = None;
                Ok(Accepted::Complete { target, bindings })
            }
        }
    }
}

impl Pending {
    fn admits(&self, bindings: &[AdoptBinding]) -> bool {
        self.bindings.len().saturating_add(bindings.len()) <= MAXIMUM_ADOPT_HOST_BINDINGS
            && bindings.iter().all(|binding| {
                let epoch = binding.identity.host_epoch();
                !self.terminals.contains(&binding.terminal_id)
                    && !self
                        .hosted
                        .contains(&(epoch, binding.identity.terminal_id()))
                    && !self
                        .creations
                        .contains(&(epoch, binding.identity.creation_id()))
            })
    }
}
