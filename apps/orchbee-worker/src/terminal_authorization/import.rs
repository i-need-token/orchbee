//! The one transition that gives a replacement Worker's kernel the bindings of the Worker it
//! replaces, once the adopted Host's complete inventory has been read. It matches each binding to
//! its Host terminal without asking the Host anything, and leaves verification and reading to the
//! adoption that calls it.

use std::collections::{BTreeMap, BTreeSet};

use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptBindingState, HostTerminalIdentity, TerminalId,
};

use super::{
    AuthorizationKernelError, AuthorizedHostOperationKernel, BindingAuthority,
    MAXIMUM_RETAINED_TERMINAL_BINDINGS, TerminalAuthorizationBinding, lifecycle::LifecycleReceipts,
};

/// One terminal of the adopted Host's inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InventoryTerminal {
    pub(crate) identity: HostTerminalIdentity,
    /// Its program has started: any phase but awaiting start, an exited one included.
    pub(crate) started: bool,
}

/// What an import took in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportOutcome {
    pub(crate) imported: u32,
    /// The Host terminals no binding matched, which no request addresses from now on.
    pub(crate) quarantined: Vec<HostTerminalIdentity>,
    /// The bindings whose Host terminal the inventory lacks or contradicts, in binding order.
    pub(crate) missing: Vec<TerminalId>,
}

impl AuthorizedHostOperationKernel {
    /// Imports `bindings` against the Host's complete `inventory` into a kernel that has admitted
    /// nothing. A binding matches the inventory terminal with its whole Host identity and the same
    /// started state; it is imported with its fence as a floor and its Close's receipts. A binding
    /// without a match is missing, and a Host terminal without one is quarantined, so a terminal
    /// whose state contradicts its binding is both.
    ///
    /// # Errors
    ///
    /// Returns [`AuthorizationKernelError::InconsistentBinding`], changing nothing, when this
    /// kernel already holds a binding, an operation or an import, when `bindings` exceed the
    /// retained-binding bound, name one terminal twice or match one Host terminal twice, or when
    /// `inventory` names one Host terminal twice.
    pub(crate) fn import(
        &mut self,
        bindings: &[AdoptBinding],
        inventory: &[InventoryTerminal],
    ) -> Result<ImportOutcome, AuthorizationKernelError> {
        let terminals: BTreeSet<_> = bindings.iter().map(|binding| binding.terminal_id).collect();
        if !self.bindings.is_empty()
            || self.slot.is_some()
            || !self.quarantined.is_empty()
            || bindings.len() > MAXIMUM_RETAINED_TERMINAL_BINDINGS
            || terminals.len() != bindings.len()
        {
            return Err(AuthorizationKernelError::InconsistentBinding);
        }
        let mut hosted = BTreeMap::new();
        for terminal in inventory {
            if hosted
                .insert(terminal.identity.terminal_id(), *terminal)
                .is_some()
            {
                return Err(AuthorizationKernelError::InconsistentBinding);
            }
        }
        let mut imported = BTreeMap::new();
        let mut claimed = BTreeSet::new();
        let mut missing = Vec::new();
        for binding in bindings {
            let started = binding.state == AdoptBindingState::Started;
            let matched = hosted
                .get(&binding.identity.terminal_id())
                .is_some_and(|terminal| {
                    terminal.identity == binding.identity && terminal.started == started
                });
            if !matched {
                missing.push(binding.terminal_id);
                continue;
            }
            if !claimed.insert(binding.identity.terminal_id()) {
                return Err(AuthorizationKernelError::InconsistentBinding);
            }
            imported.insert(
                binding.terminal_id,
                TerminalAuthorizationBinding {
                    cached_result: None,
                    authority: BindingAuthority::Imported {
                        fence: binding.fence,
                        identity: binding.identity,
                    },
                    lifecycle: binding.close.map(LifecycleReceipts::imported),
                },
            );
        }
        let quarantined: Vec<_> = hosted
            .values()
            .filter(|terminal| !claimed.contains(&terminal.identity.terminal_id()))
            .map(|terminal| terminal.identity)
            .collect();
        let count = u32::try_from(imported.len())
            .map_err(|_| AuthorizationKernelError::InconsistentBinding)?;
        self.quarantined = quarantined
            .iter()
            .map(|identity| identity.terminal_id())
            .collect();
        self.bindings = imported;
        Ok(ImportOutcome {
            imported: count,
            quarantined,
            missing,
        })
    }
}

#[cfg(test)]
mod tests;
