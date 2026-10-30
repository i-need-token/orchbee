//! Names optional host effects and their current platform admission.
//! View action selection and feature state stay with the composition owner.

/// An optional operating-system effect; this is independent of Server action availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
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
pub struct Unsupported(pub Capability);

impl Capability {
    /// Every optional effect, in declaration order.
    pub(crate) const ALL: [Self; 10] = [
        Self::Clipboard,
        Self::Browser,
        Self::SavedMachines,
        Self::DirectoryInput,
        Self::AgentLaunch,
        Self::PositionStorage,
        Self::ConfigurationFile,
        Self::SystemNotification,
        Self::WindowTitle,
        Self::Graphics,
    ];

    pub const fn available(self) -> bool {
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

    pub fn require(self) -> Result<(), Unsupported> {
        self.available().then_some(()).ok_or(Unsupported(self))
    }
}

/// The existing default-shell adapter admits POSIX login mode only.
pub(crate) const fn login_shell_available() -> bool {
    cfg!(unix)
}

/// Only the Windows local activation adapter retains a diagnosable startup checkpoint when
/// recovery is required; other hosts refuse without starting a replacement process.
pub(crate) const fn startup_recovery_checkpoint() -> bool {
    cfg!(windows)
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
