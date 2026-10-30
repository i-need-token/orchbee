//! Composition policy for optional host effects. Shared state and protocol operations stay available.
use crate::client_actions::ClientAction;

pub use crate::host_capabilities::Capability;

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
    crate::host_capabilities::login_shell_available()
}

/// Immutable composition input. Pure UI traces can exercise any capability set on any host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
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
    /// Admits exactly the effects whose host adapters exist on this build.
    pub const fn for_host() -> Self {
        let mut effects = Self::none().effects;
        let mut index = 0;
        while index < Capability::ALL.len() {
            let capability = Capability::ALL[index];
            if capability.available() {
                effects |= 1 << capability as u8;
            }
            index += 1;
        }
        Self {
            effects,
            login_shell: login_shell_supported(),
        }
    }
    pub const fn supports(self, capability: Capability) -> bool {
        self.effects & (1 << capability as u8) != 0
    }
}
