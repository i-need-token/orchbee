//! Retains bounded controller reservations, committed capability revisions, and input
//! sequences for exact viewer and terminal identities. It admits local grant use without
//! granting Server authorization or performing Host I/O.

use orchbee_capability_node_protocol::{ControlRejection, HostTerminalId, OperationId};
use orchbee_terminal_protocol::ProjectionDemand;
use std::{collections::BTreeMap, num::NonZeroU64};

const MAXIMUM_CONTROLLERS: usize = 8;

#[derive(Clone, Copy)]
pub(crate) struct ControlGrant {
    pub(crate) viewer_id: OperationId,
    pub(crate) controller_id: OperationId,
    pub(crate) terminal_id: HostTerminalId,
    pub(crate) capability_revision: Option<NonZeroU64>,
    /// The demand the Host holds for this controller attachment. Cleanup brings it in line with
    /// its viewer's demand when a request had no room to.
    pub(crate) demand: ProjectionDemand,
    sequence: u64,
}
#[derive(Default)]
pub(crate) struct HostControllers {
    grants: BTreeMap<[u8; 16], ControlGrant>,
}
impl HostControllers {
    #[cfg(windows)]
    pub(crate) fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }
    pub(crate) fn contains(&self, id: OperationId) -> bool {
        self.grants.contains_key(&id.as_bytes())
    }
    pub(crate) fn reserve(
        &mut self,
        viewer_id: OperationId,
        controller_id: OperationId,
        terminal_id: HostTerminalId,
    ) -> Result<(), ControlRejection> {
        if self.grants.len() >= MAXIMUM_CONTROLLERS
            || self.contains(controller_id)
            || self
                .grants
                .values()
                .any(|grant| grant.terminal_id == terminal_id || grant.viewer_id == viewer_id)
        {
            return Err(ControlRejection::ControllerBusy);
        }
        self.grants.insert(
            controller_id.as_bytes(),
            ControlGrant {
                viewer_id,
                controller_id,
                terminal_id,
                capability_revision: None,
                // A new Host attachment starts paused.
                demand: ProjectionDemand::Paused,
                sequence: 0,
            },
        );
        Ok(())
    }
    pub(crate) fn commit(&mut self, id: OperationId, revision: NonZeroU64) {
        if let Some(grant) = self.grants.get_mut(&id.as_bytes()) {
            grant.capability_revision = Some(revision);
        }
    }
    /// Records the demand the Host accepted for a controller.
    pub(crate) fn set_demand(&mut self, id: OperationId, demand: ProjectionDemand) {
        if let Some(grant) = self.grants.get_mut(&id.as_bytes()) {
            grant.demand = demand;
        }
    }
    pub(crate) fn grants(&self) -> Vec<ControlGrant> {
        self.grants.values().copied().collect()
    }
    pub(crate) fn for_viewer(&self, id: OperationId) -> Option<ControlGrant> {
        self.grants
            .values()
            .find(|grant| grant.viewer_id == id)
            .copied()
    }
    /// Validate the entire replacement before revoking any Host attachment.
    pub(crate) fn takeover_incumbent(
        &self,
        viewer: OperationId,
        controller: OperationId,
        terminal: HostTerminalId,
        previous: OperationId,
    ) -> Result<Option<ControlGrant>, ControlRejection> {
        if self.contains(controller)
            || controller == previous
            || self
                .for_viewer(viewer)
                .is_some_and(|grant| grant.controller_id != previous)
        {
            return Err(ControlRejection::ControllerBusy);
        }
        match self.grants.get(&previous.as_bytes()).copied() {
            Some(grant) if grant.terminal_id != terminal => Err(ControlRejection::NotController),
            Some(grant) if grant.capability_revision.is_none() => {
                Err(ControlRejection::ControllerBusy)
            }
            Some(grant) => Ok(Some(grant)),
            None if self.for_terminal(terminal).is_some() => Err(ControlRejection::NotController),
            None if self.grants.len() >= MAXIMUM_CONTROLLERS => {
                Err(ControlRejection::ControllerBusy)
            }
            None => Ok(None),
        }
    }
    /// A Takeover naming one controller as both incumbent and replacement asks whether `viewer`
    /// still holds that committed grant on `terminal`, and changes nothing. A grant another viewer
    /// holds stays busy, as for any replacement reusing its incumbent's identity.
    pub(crate) fn confirm_own(
        &self,
        viewer: OperationId,
        controller: OperationId,
        terminal: HostTerminalId,
    ) -> Result<(), ControlRejection> {
        match self.grants.get(&controller.as_bytes()) {
            Some(grant) if grant.viewer_id != viewer => Err(ControlRejection::ControllerBusy),
            Some(grant) if grant.terminal_id == terminal && grant.capability_revision.is_some() => {
                Ok(())
            }
            _ => Err(ControlRejection::NotController),
        }
    }
    pub(crate) fn admit(
        &mut self,
        viewer: OperationId,
        controller: OperationId,
        sequence: NonZeroU64,
    ) -> Result<ControlGrant, ControlRejection> {
        let grant = self
            .grants
            .get_mut(&controller.as_bytes())
            .filter(|grant| grant.viewer_id == viewer && grant.capability_revision.is_some())
            .ok_or(ControlRejection::NotController)?;
        if sequence.get() <= grant.sequence {
            return Err(ControlRejection::SequenceMismatch);
        }
        grant.sequence = sequence.get();
        Ok(*grant)
    }
    pub(crate) fn remove(&mut self, id: OperationId) {
        self.grants.remove(&id.as_bytes());
    }

    pub(super) fn for_terminal(&self, terminal: HostTerminalId) -> Option<OperationId> {
        self.grants
            .values()
            .find(|grant| grant.terminal_id == terminal)
            .map(|grant| grant.controller_id)
    }
    /// The viewer a held controller attachment follows; `None` for one this Worker does not hold.
    pub(super) fn viewer_of(
        &self,
        attachment: orchbee_terminal_protocol::AttachmentId,
    ) -> Option<OperationId> {
        self.grants
            .get(attachment.as_bytes())
            .map(|grant| grant.viewer_id)
    }
    pub(crate) fn validate_lease_event(
        &self,
        event: orchbee_terminal_protocol::ResizeLeaseChanged,
    ) -> bool {
        self.grants
            .get(event.attachment_id.as_bytes())
            .is_some_and(|grant| {
                grant.terminal_id.as_bytes() == *event.terminal_id.as_bytes() && event.resize_lease
            })
    }
}

#[cfg(test)]
mod tests;
