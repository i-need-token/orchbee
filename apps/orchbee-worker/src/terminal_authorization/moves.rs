//! The kernel's side of a move between Hosts (`proposals/platform/host-handover.md`, H3 design):
//! which bindings may move, and the rebinding that follows a claim. It asks no Host anything.

use orchbee_capability_node_protocol::{HostEpoch, HostTerminalIdentity, TerminalId};

use super::{AuthorizedHostOperationKernel, BindingAuthority};

impl AuthorizedHostOperationKernel {
    /// The bindings whose Host terminal runs in the Host of `epoch` and that no Close has reached,
    /// in product-terminal order.
    pub(crate) fn movable_on(&self, epoch: HostEpoch) -> Vec<(TerminalId, HostTerminalIdentity)> {
        self.bindings
            .iter()
            .filter(|(_, binding)| binding.lifecycle.is_none())
            .filter_map(|(terminal, binding)| Some((*terminal, binding.lifecycle_identity()?)))
            .filter(|(_, identity)| identity.host_epoch() == epoch)
            .collect()
    }

    /// Rebinds `terminal`, whose Host terminal `from` moved and runs as `to`: the binding keeps its
    /// fence as an imported one, and its cached Launch or Start result goes, since it named `from`.
    /// Returns whether the binding still named `from`.
    pub(crate) fn rebind(
        &mut self,
        terminal: TerminalId,
        from: HostTerminalIdentity,
        to: HostTerminalIdentity,
    ) -> bool {
        let Some(binding) = self.bindings.get_mut(&terminal) else {
            return false;
        };
        if binding.lifecycle_identity() != Some(from) {
            return false;
        }
        binding.authority = BindingAuthority::Imported {
            fence: binding.fence(),
            identity: to,
        };
        binding.cached_result = None;
        true
    }
}
