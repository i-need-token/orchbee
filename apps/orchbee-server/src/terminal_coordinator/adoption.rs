//! Moves the settled bindings of a lost Worker into the generation of the replacement that adopted
//! their Host, and a binding whose terminal that replacement moved to another Host onto it. A
//! binding with an operation in flight at the loss is never moved: that operation settles against
//! the generation it was issued to, as `LocalTerminalCoordinator`'s `ImportBinding` requires.

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptBindingState, HostTerminalIdentity, TerminalId,
    TerminalOperationAuthorization,
};

use super::{TerminalCoordinator, TerminalPhase, WorkerPlacement, matches_placement};

/// How an adoption changed the bindings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AdoptionImport {
    pub(crate) imported: usize,
    pub(crate) missing: usize,
}

impl TerminalCoordinator {
    /// The settled bindings `lost` held, each awaiting its start or started, with its Host identity
    /// and the highest fence the Server issued for it. A binding whose launch or start outcome is
    /// unknown, or still in flight, is left out; none carries a Close yet.
    pub(crate) fn adoption_bindings(&self, lost: WorkerPlacement) -> Vec<AdoptBinding> {
        self.bindings
            .values()
            .filter(|binding| matches_placement(binding.authorization, lost))
            .filter_map(|binding| {
                let (identity, state) = match binding.phase {
                    TerminalPhase::AwaitingStart(identity) => {
                        (identity, AdoptBindingState::AwaitingStart)
                    }
                    TerminalPhase::Started { identity, .. } => {
                        (identity, AdoptBindingState::Started)
                    }
                    _ => return None,
                };
                Some(AdoptBinding {
                    terminal_id: binding.authorization.terminal_id(),
                    identity,
                    state,
                    fence: binding.authorization.fence(),
                    close: None,
                })
            })
            .collect()
    }

    /// Moves each binding of `lost` named in `sent` into `replacement`'s generation, keeping its
    /// operation, fence and lease, once the replacement adopted their Host. A binding the complete
    /// Host inventory lacked, named in `missing`, becomes unavailable instead: its program is gone.
    /// A binding that is no longer settled, or no longer `lost`'s, is left alone.
    pub(crate) fn import_adopted(
        &mut self,
        lost: WorkerPlacement,
        replacement: WorkerPlacement,
        sent: &[TerminalId],
        missing: &[TerminalId],
    ) -> AdoptionImport {
        let mut import = AdoptionImport::default();
        for binding in self.bindings.values_mut() {
            let terminal = binding.authorization.terminal_id();
            if !matches_placement(binding.authorization, lost) || !sent.contains(&terminal) {
                continue;
            }
            let identity = match binding.phase {
                TerminalPhase::AwaitingStart(identity)
                | TerminalPhase::Started { identity, .. } => identity,
                _ => continue,
            };
            if missing.contains(&terminal) {
                binding.phase = TerminalPhase::Unavailable(Some(identity));
                import.missing += 1;
                continue;
            }
            binding.authorization = moved(binding.authorization, replacement);
            import.imported += 1;
        }
        import
    }

    /// Rebinds the binding of `terminal` that `worker` holds, whose Host terminal `from` moved and
    /// runs as `to` (`proposals/platform/host-handover.md`, Rebinding): it keeps its operation,
    /// fence and lease, and every fact read through it follows. A binding that no longer names
    /// `from`, closed or rebound meanwhile, is left alone. Returns whether one was rebound.
    pub(crate) fn rebind_moved(
        &mut self,
        worker: WorkerPlacement,
        terminal: TerminalId,
        from: HostTerminalIdentity,
        to: HostTerminalIdentity,
    ) -> bool {
        let Some(binding) = self.bindings.values_mut().find(|binding| {
            binding.authorization.terminal_id() == terminal
                && matches_placement(binding.authorization, worker)
        }) else {
            return false;
        };
        match &mut binding.phase {
            TerminalPhase::AwaitingStart(identity) | TerminalPhase::Started { identity, .. }
                if *identity == from =>
            {
                *identity = to;
                true
            }
            _ => false,
        }
    }
}

fn moved(
    authorization: TerminalOperationAuthorization,
    replacement: WorkerPlacement,
) -> TerminalOperationAuthorization {
    TerminalOperationAuthorization::new(
        authorization.terminal_id(),
        authorization.operation_id(),
        replacement.worker_id,
        replacement.generation,
        authorization.fence(),
        authorization.lease(),
        authorization.capability_version(),
    )
}
