//! Names optional host effects and their current platform admission.
//! View action selection and feature state stay with the composition owner.

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
        match self {
            Self::Clipboard
            | Self::Browser
            | Self::SavedMachines
            | Self::DirectoryInput
            | Self::AgentLaunch
            | Self::PositionStorage
            | Self::ConfigurationFile
            | Self::SystemNotification
            | Self::WindowTitle
            | Self::Graphics => cfg!(windows),
        }
    }

    pub(crate) fn require(self) -> Result<(), Unsupported> {
        self.available().then_some(()).ok_or(Unsupported(self))
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
