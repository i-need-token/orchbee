//! Session-captured user launch intent for one authorized creation. The Server reads its frozen
//! preference store; the Worker resolves only device facts such as System shell discovery.
use crate::ProtocolError;

/// The preference domain admitted for new terminal scrollback.
pub const MAXIMUM_LAUNCH_SCROLLBACK_BYTES: u32 = 1_000_000_000;

/// The Server's preference capture for one launch. Servers always send `Unavailable` or
/// `Available`; `Absent` is reserved for adapters without a Session preference owner.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum LaunchPreferences {
    /// The Worker keeps its device defaults: System shell, Host colors and Host scrollback.
    #[default]
    Absent,
    /// The Session could not read or bound its confirmed snapshot; the Worker refuses creation.
    Unavailable,
    Available(TerminalLaunchPreferences),
}

/// Effective scrollback, terminal default colors and, for a default launch, the selected program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunchPreferences {
    scrollback_bytes: u32,
    colors: Option<TerminalLaunchColors>,
    // Boxed so a launch carrying no default program stays small inside operation enums.
    program: Option<Box<DefaultLaunchProgram>>,
}

impl TerminalLaunchPreferences {
    /// # Errors
    /// Rejects scrollback outside the preference domain and colors outside 24-bit RGB.
    pub fn new(
        scrollback_bytes: u32,
        colors: Option<TerminalLaunchColors>,
        program: Option<DefaultLaunchProgram>,
    ) -> Result<Self, ProtocolError> {
        if scrollback_bytes > MAXIMUM_LAUNCH_SCROLLBACK_BYTES
            || colors.is_some_and(|colors| {
                colors.foreground > 0xff_ffff || colors.background > 0xff_ffff
            })
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            scrollback_bytes,
            colors,
            program: program.map(Box::new),
        })
    }

    #[must_use]
    pub const fn scrollback_bytes(&self) -> u32 {
        self.scrollback_bytes
    }

    /// Absent only when the terminal display follows the Host's own colors.
    #[must_use]
    pub const fn colors(&self) -> Option<TerminalLaunchColors> {
        self.colors
    }

    /// Present exactly when the launch has no explicit selection.
    #[must_use]
    pub fn program(&self) -> Option<&DefaultLaunchProgram> {
        self.program.as_deref()
    }
}

/// Foreground and background defaults with an explicit color scheme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalLaunchColors {
    pub foreground: u32,
    pub background: u32,
    pub dark: bool,
}

/// The selected default launch profile as user intent. A custom program stays literal because the
/// preference store owns its path admission; it shares the explicit selection budget because a
/// launch carries only one of them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefaultLaunchProgram {
    SystemShell { login: bool },
    Custom(DefaultCustomProgram),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultCustomProgram {
    executable: String,
    arguments: Vec<String>,
}

impl DefaultCustomProgram {
    /// # Errors
    /// Rejects an empty executable and any program outside the launch selection bounds.
    pub fn new(executable: String, arguments: Vec<String>) -> Result<Self, ProtocolError> {
        if executable.is_empty()
            || !crate::terminal_launch::program_within_bounds(&executable, &arguments)
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            executable,
            arguments,
        })
    }

    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}
