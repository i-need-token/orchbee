//! Names optional host effects and their current platform admission.
//! View action selection and feature state stay with the composition owner.

// module-layer: values

#[cfg(not(windows))]
pub(crate) mod unix;

/// An optional operating-system effect; this is independent of Server action availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    Clipboard,
    Browser,
    SavedMachines,
    DirectoryInput,
    Worktrees,
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
    pub(crate) const ALL: [Self; 11] = [
        Self::Clipboard,
        Self::Browser,
        Self::SavedMachines,
        Self::DirectoryInput,
        Self::Worktrees,
        Self::AgentLaunch,
        Self::PositionStorage,
        Self::ConfigurationFile,
        Self::SystemNotification,
        Self::WindowTitle,
        Self::Graphics,
    ];

    pub fn available(self) -> bool {
        // The terminal, local transport, shared Workbench operations and client-local interaction
        // are separate capabilities. The macOS and Linux adapters of the session-dependent effects
        // admit what this session's frozen facts support.
        match self {
            Self::Clipboard | Self::Browser | Self::SystemNotification | Self::WindowTitle => {
                session_admits(self)
            }
            // Every platform's shells get the managed prompt and run prefix; every platform keeps
            // the position record and `config.toml` under its data root, resolves a typed folder in
            // its own grammar from its launch folder, and its Server serves Worktree operations and
            // Panel graphics. Graphics admits only the outer terminal's probe; its replies, never
            // the terminal's name, select the renderer. The machine hub, its carriers and the far
            // end run over the system `ssh`.
            Self::AgentLaunch
            | Self::PositionStorage
            | Self::ConfigurationFile
            | Self::DirectoryInput
            | Self::Worktrees
            | Self::Graphics
            | Self::SavedMachines => true,
        }
    }

    pub fn require(self) -> Result<(), Unsupported> {
        self.available().then_some(()).ok_or(Unsupported(self))
    }

    /// Whether this build's adapter depends on the session (a desktop, a remote login or the
    /// outer terminal), so a refusal describes the session rather than the platform.
    pub(crate) const fn session_dependent(self) -> bool {
        !cfg!(windows)
            && matches!(
                self,
                Self::Clipboard | Self::Browser | Self::SystemNotification | Self::WindowTitle
            )
    }
}

#[cfg(windows)]
const fn session_admits(_capability: Capability) -> bool {
    true
}
#[cfg(not(windows))]
fn session_admits(capability: Capability) -> bool {
    unix::admits(capability)
}

/// Whether the outer terminal is one known to print a capability query it cannot parse. Such a
/// host receives no graphics probe and no DECRQM query; each capability keeps the value an
/// unanswered probe settles to.
pub fn host_prints_unparsed_queries() -> bool {
    #[cfg(windows)]
    {
        false
    }
    #[cfg(not(windows))]
    {
        unix::facts().query_echo
    }
}

/// The existing default-shell adapter admits POSIX login mode only.
pub(crate) const fn login_shell_available() -> bool {
    cfg!(unix)
}

/// What a startup refused for required recovery can say about the Session's checkpoint. Every
/// host refuses without starting a replacement process, so no Server touches the checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryCheckpoint {
    /// The checkpoint is retained, and `orchbee doctor` and another named Session are admitted.
    Diagnosable,
    /// The checkpoint is retained; neither command is admitted on this host yet.
    Retained,
}

/// This host's [`RecoveryCheckpoint`].
pub(crate) const fn startup_recovery_checkpoint() -> RecoveryCheckpoint {
    if cfg!(windows) {
        RecoveryCheckpoint::Diagnosable
    } else {
        RecoveryCheckpoint::Retained
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
