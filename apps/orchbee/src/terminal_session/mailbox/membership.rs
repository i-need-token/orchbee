use super::{AttachmentKey, CarrierFailure, MailboxMembership, MailboxState, UiReplyTransaction};
use orchbee_workbench_protocol::WorkbenchTabId;

pub(in crate::terminal_session) enum DesiredInstall {
    Preserve,
    ReuseReserved(AttachmentKey),
    RestoreSwitch(crate::terminal_session::state::SwitchRestoreKey),
    Fresh(WorkbenchTabId),
    Empty,
}

/// Holding this plan keeps the mailbox mutex's state exclusively borrowed until commit or drop.
pub(in crate::terminal_session) struct PreparedDesiredInstall<'a> {
    state: &'a mut MailboxState,
    desired: Option<AttachmentKey>,
    generation: u64,
}

impl PreparedDesiredInstall<'_> {
    pub(in crate::terminal_session) const fn desired(&self) -> Option<&AttachmentKey> {
        self.desired.as_ref()
    }

    pub(in crate::terminal_session) fn commit(self) {
        self.state.desired = MailboxMembership::Live {
            desired: self.desired,
        };
        self.state.attachment_generation = self.generation;
    }
}

impl UiReplyTransaction<'_> {
    pub(in crate::terminal_session) fn prepare_desired_install(
        &mut self,
        install: DesiredInstall,
    ) -> Result<PreparedDesiredInstall<'_>, CarrierFailure> {
        if self.state.desired.awaiting_bootstrap() || self.state.closed.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        let mut generation = self.state.attachment_generation;
        let desired = match install {
            DesiredInstall::Preserve => self.state.desired.key().cloned(),
            DesiredInstall::ReuseReserved(key) => {
                if key.generation() == 0 || key.generation() != generation {
                    return Err(CarrierFailure::Protocol);
                }
                Some(key)
            }
            DesiredInstall::RestoreSwitch(proof) => {
                if self.state.desired.key() != Some(proof.marker())
                    || generation != proof.marker().generation()
                    || proof.restored().generation() >= generation
                {
                    return Err(CarrierFailure::Protocol);
                }
                Some(proof.restored().clone())
            }
            DesiredInstall::Fresh(tab_id) => {
                generation = generation
                    .checked_add(1)
                    .ok_or(CarrierFailure::IdentityExhausted)?;
                Some(AttachmentKey::new(generation, tab_id))
            }
            DesiredInstall::Empty => None,
        };
        Ok(PreparedDesiredInstall {
            state: self.state,
            desired,
            generation,
        })
    }
}
