use super::{
    AttachmentKey, CarrierFailure, MailboxMembership, MailboxState, PaneKey, PaneRef,
    UiReplyTransaction,
};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};

pub(in crate::workbench_client) enum DesiredInstall {
    Preserve,
    ReuseReserved {
        key: AttachmentKey,
        panel: WorkbenchPanelId,
    },
    RestoreSwitch(crate::workbench_client::state::SwitchRestoreKey),
    Fresh {
        tab_id: WorkbenchTabId,
        panel: WorkbenchPanelId,
    },
    Empty,
}

/// Holding this plan keeps the mailbox mutex's state exclusively borrowed until commit or drop.
pub(in crate::workbench_client) struct PreparedDesiredInstall<'a> {
    state: &'a mut MailboxState,
    desired: Option<PaneKey>,
    generation: u64,
    panes: Vec<PaneRef>,
}

impl PreparedDesiredInstall<'_> {
    /// The focus pane the caller-side state install should resolve against.
    pub(in crate::workbench_client) const fn desired(&self) -> Option<&PaneKey> {
        self.desired.as_ref()
    }

    pub(in crate::workbench_client) fn commit(self) {
        self.state.desired = MailboxMembership::Live {
            desired: self.desired,
            panes: self.panes,
        };
        self.state.attachment_generation = self.generation;
    }
}

impl UiReplyTransaction<'_> {
    pub(in crate::workbench_client) fn prepare_desired_install(
        &mut self,
        install: DesiredInstall,
    ) -> Result<PreparedDesiredInstall<'_>, CarrierFailure> {
        if self.state.desired.awaiting_bootstrap() || self.state.closed.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        let mut generation = self.state.attachment_generation;
        // Preserve the current manifest or restore only the incarnations proven
        // by the exclusively borrowed Client. Fresh epochs wait for reconcile.
        let panes = match &install {
            DesiredInstall::Preserve => match &self.state.desired {
                MailboxMembership::Live { panes, .. } => panes.clone(),
                MailboxMembership::AwaitingBootstrap => Vec::new(),
            },
            DesiredInstall::RestoreSwitch(proof) => proof.panes().to_vec(),
            _ => Vec::new(),
        };
        let desired = match install {
            DesiredInstall::Preserve => self.state.desired.key().cloned(),
            DesiredInstall::ReuseReserved { key, panel } => {
                if key.generation() == 0 || key.generation() != generation {
                    return Err(CarrierFailure::Protocol);
                }
                Some(PaneKey::new(key, panel))
            }
            DesiredInstall::RestoreSwitch(proof) => {
                if self.state.desired.key() != Some(proof.marker())
                    || generation != proof.marker().anchor().generation()
                    || proof.restored().anchor().generation() >= generation
                {
                    return Err(CarrierFailure::Protocol);
                }
                Some(proof.restored().clone())
            }
            DesiredInstall::Fresh { tab_id, panel } => {
                generation = generation
                    .checked_add(1)
                    .ok_or(CarrierFailure::IdentityExhausted)?;
                Some(PaneKey::new(AttachmentKey::new(generation, tab_id), panel))
            }
            DesiredInstall::Empty => None,
        };
        Ok(PreparedDesiredInstall {
            state: self.state,
            desired,
            generation,
            panes,
        })
    }
}
