//! Composition policy for optional host effects. Shared state and protocol operations stay available.
use crate::client_view::ClientAction;

pub(crate) use crate::host_capabilities::{Capability, Unsupported};

/// Select the host effect required by this view intent; availability is a separate input.
pub(crate) fn for_action(action: &ClientAction) -> Option<Capability> {
    match action {
        ClientAction::OpenMachines => Some(Capability::SavedMachines),
        ClientAction::ReloadConfiguration => Some(Capability::ConfigurationFile),
        ClientAction::RequestQuickWorkspace
        | ClientAction::RequestWorkspaceCreate
        | ClientAction::RequestWorkspaceAtDirectory
        | ClientAction::RequestProjectQuickWorkspace
        | ClientAction::RequestWorktreeCreate
        | ClientAction::RequestWorktreeOpen
        | ClientAction::RequestWorkspaceDirectory => Some(Capability::DirectoryInput),
        _ => None,
    }
}

/// The existing default-shell adapter admits POSIX login mode only.
pub(crate) const fn login_shell_supported() -> bool {
    cfg!(unix)
}

/// Immutable composition input. Pure UI traces can exercise any capability set on any host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Capabilities {
    effects: u16,
    pub(crate) login_shell: bool,
}
impl Capabilities {
    pub(crate) const fn none() -> Self {
        Self {
            effects: 0,
            login_shell: false,
        }
    }
    pub(crate) const fn all() -> Self {
        Self {
            effects: (1 << 10) - 1,
            login_shell: true,
        }
    }
    pub(crate) const fn for_host() -> Self {
        Self {
            effects: if cfg!(windows) {
                Self::all().effects
            } else {
                Self::none().effects
            },
            login_shell: login_shell_supported(),
        }
    }
    pub(crate) const fn supports(self, capability: Capability) -> bool {
        self.effects & (1 << capability as u8) != 0
    }
}
