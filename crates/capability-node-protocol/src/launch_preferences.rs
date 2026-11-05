//! Session-captured user launch intent for one authorized creation. The Server reads its frozen
//! preference store; the Worker resolves only device facts such as System shell discovery.
use crate::ProtocolError;

/// The preference domain admitted for new terminal scrollback.
pub const MAXIMUM_LAUNCH_SCROLLBACK_BYTES: u32 = 1_000_000_000;

/// The longest color-theme id a launch carries, in UTF-8 bytes.
pub const MAXIMUM_LAUNCH_THEME_ID_BYTES: usize = 64;

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

/// Effective scrollback, terminal default colors, the selected color themes and, for a default
/// launch, the selected program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunchPreferences {
    scrollback_bytes: u32,
    colors: Option<TerminalLaunchColors>,
    // Boxed, like the program, so launch operation enums stay small.
    themes: Box<TerminalLaunchThemes>,
    // Boxed so a launch carrying no default program stays small inside operation enums.
    program: Option<Box<DefaultLaunchProgram>>,
}

impl TerminalLaunchPreferences {
    /// # Errors
    /// Rejects scrollback outside the preference domain and colors outside 24-bit RGB.
    pub fn new(
        scrollback_bytes: u32,
        colors: Option<TerminalLaunchColors>,
        themes: TerminalLaunchThemes,
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
            themes: Box::new(themes),
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

    /// The slots whose theme supplies cursor and ANSI colors for the polarity of the terminal's
    /// default colors, whether explicit or followed from the Host.
    #[must_use]
    pub fn themes(&self) -> &TerminalLaunchThemes {
        &self.themes
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

/// The device's stored light and dark color-theme ids. Ids stay literal user intent: the Capability
/// Node resolves an id its build does not ship to its default theme of that polarity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalLaunchThemes {
    light: String,
    dark: String,
}

impl TerminalLaunchThemes {
    /// # Errors
    /// Rejects an empty id and an id longer than [`MAXIMUM_LAUNCH_THEME_ID_BYTES`].
    pub fn new(light: String, dark: String) -> Result<Self, ProtocolError> {
        if [&light, &dark]
            .iter()
            .any(|id| id.is_empty() || id.len() > MAXIMUM_LAUNCH_THEME_ID_BYTES)
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { light, dark })
    }

    #[must_use]
    pub fn light(&self) -> &str {
        &self.light
    }

    #[must_use]
    pub fn dark(&self) -> &str {
        &self.dark
    }
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
