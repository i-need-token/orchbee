use orchbee_capability_node_protocol::{ControlRejection, HostTerminalId, OperationId};
use std::{collections::BTreeMap, num::NonZeroU64};

const MAXIMUM_CONTROLLERS: usize = 8;

#[derive(Clone, Copy)]
pub(crate) struct ControlGrant {
    pub(crate) viewer_id: OperationId,
    pub(crate) controller_id: OperationId,
    pub(crate) terminal_id: HostTerminalId,
    pub(crate) capability_revision: Option<NonZeroU64>,
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
