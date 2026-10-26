//! Composition policy for optional host effects. Shared state and protocol operations stay available.
use crate::client_view::ClientAction;

/// An optional operating-system effect; this is independent of Server action availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Capability {
    Clipboard,
    Browser,
    SavedMachines,
    DirectoryInput,
    AgentLaunch,
    PositionStorage,
    ConfigurationFile,
    SystemNotification,
    WindowTitle,
    Graphics,
}

/// Refusal before any external effect; it never denotes a failed or unknown commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Unsupported(pub(crate) Capability);

impl Capability {
    pub(crate) const fn available(self) -> bool {
        // These capabilities currently have Windows adapters. The terminal, local transport,
        // shared Workbench operations and client-local interaction are separate capabilities.
        cfg!(windows)
    }

    pub(crate) fn require(self) -> Result<(), Unsupported> {
        self.available().then_some(()).ok_or(Unsupported(self))
    }

    pub(crate) fn for_action(action: &ClientAction) -> Option<Self> {
        match action {
            ClientAction::OpenMachines => Some(Self::SavedMachines),
            ClientAction::ReloadConfiguration => Some(Self::ConfigurationFile),
            ClientAction::RequestQuickWorkspace
            | ClientAction::RequestWorkspaceCreate
            | ClientAction::RequestWorkspaceAtDirectory(_)
            | ClientAction::RequestProjectQuickWorkspace(_)
            | ClientAction::RequestWorktreeCreate
            | ClientAction::RequestWorktreeOpen => Some(Self::DirectoryInput),
            _ => None,
        }
    }
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported host capability: {:?}", self.0)
    }
}
impl std::error::Error for Unsupported {}

impl From<Unsupported> for std::io::Error {
    fn from(value: Unsupported) -> Self {
        Self::new(std::io::ErrorKind::Unsupported, value)
    }
}

/// The existing default-shell adapter admits POSIX login mode only.
pub(crate) const fn login_shell_supported() -> bool {
    cfg!(unix)
}
