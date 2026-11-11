//! Composition policy for optional host effects. Shared state and protocol operations stay available.
use crate::client_actions::ClientAction;

pub use crate::host_capabilities::Capability;

/// Select the host effect required by this view intent; availability is a separate input.
pub(crate) fn for_action(action: &ClientAction) -> Option<Capability> {
    match action {
        ClientAction::OpenMachines => Some(Capability::SavedMachines),
        ClientAction::ReloadConfiguration => Some(Capability::ConfigurationFile),
        ClientAction::StartAgent => Some(Capability::AgentLaunch),
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
    /// Whether sidebar search is offered: the fixed Search row and its key. No host build offers
    /// them yet; the machine picker they open stays reachable through the Machines action. This is
    /// a build policy, independent of the host effects above, and composed traces still offer it.
    search: bool,
}
impl Capabilities {
    /// No optional host effect; build policy still offers every entry point.
    pub(crate) const fn none() -> Self {
        Self {
            effects: 0,
            login_shell: false,
            search: true,
        }
    }
    pub(crate) const fn all() -> Self {
        Self {
            effects: (1 << 10) - 1,
            login_shell: true,
            search: true,
        }
    }
    /// Admits exactly the effects whose host adapters exist on this build, and the entry points
    /// every build offers.
    pub fn for_host() -> Self {
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
            // Search is withheld until it is finished (ORC-424); its code stays in place.
            search: false,
        }
    }
    pub const fn supports(self, capability: Capability) -> bool {
        self.effects & (1 << capability as u8) != 0
    }
    /// Whether the sidebar Search row and its key are offered.
    pub(crate) const fn offers_search(self) -> bool {
        self.search
    }
}
