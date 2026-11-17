//! One creation's launch capture: the Session preferences read once before admission, the System
//! shell of the Worker generation that launches it and, for a configured command's Panel, the
//! program its launch presents. The dispatched launch, the launched program published with its
//! binding and whether that launch runs an interactive Shell all derive from this value; it
//! performs no I/O.

use orchbee_capability_node_protocol::{DefaultLaunchProgram, LaunchPreferences, LaunchSelection};
use orchbee_workbench_protocol::PresentationLaunchProgram;

#[derive(Clone, Debug)]
pub(crate) struct LaunchCapture {
    /// What an explicit selection carries: the confirmed scrollback and colors, never a program.
    explicit: LaunchPreferences,
    /// What a default launch carries, including the default profile's program.
    default: LaunchPreferences,
    system_shell: Option<PresentationLaunchProgram>,
    /// The program an explicit launch presents in place of the adapter it dispatches.
    presented: Option<PresentationLaunchProgram>,
}

impl LaunchCapture {
    pub(crate) const fn new(explicit: LaunchPreferences, default: LaunchPreferences) -> Self {
        Self {
            explicit,
            default,
            system_shell: None,
            presented: None,
        }
    }

    /// A capture whose every launch the Worker refuses, for requests that never create.
    pub(crate) const fn unavailable() -> Self {
        Self::new(
            LaunchPreferences::Unavailable,
            LaunchPreferences::Unavailable,
        )
    }

    /// The device defaults of an adapter without a Session preference owner.
    #[cfg(test)]
    pub(crate) const fn absent() -> Self {
        Self::new(LaunchPreferences::Absent, LaunchPreferences::Absent)
    }

    /// Adds the System shell file name of the Worker generation the creation targets; only
    /// [`CreateInputs`](super::CreateInputs) supplies it, from that generation's request handle.
    #[must_use]
    pub(super) fn with_system_shell(
        mut self,
        system_shell: Option<PresentationLaunchProgram>,
    ) -> Self {
        self.system_shell = system_shell;
        self
    }

    /// Names the program an explicit launch presents when its argv starts that program through an
    /// adapter that hides it, as a Windows configured command's PowerShell adapter does; only that
    /// configured-command Panel creation supplies it. A default launch never reads it.
    #[must_use]
    pub(crate) fn presenting(mut self, program: Option<PresentationLaunchProgram>) -> Self {
        self.presented = program;
        self
    }

    pub(crate) fn preferences(&self, selection: Option<&LaunchSelection>) -> LaunchPreferences {
        if selection.is_some() {
            self.explicit.clone()
        } else {
            self.default.clone()
        }
    }

    /// The file name of the program a launch with this selection requests: the program an explicit
    /// selection presents or starts, else the default profile's custom executable, else this
    /// generation's System shell. A default launch whose capture is unavailable, which the Worker
    /// refuses, requests no known program.
    pub(super) fn launch_program(
        &self,
        selection: Option<&LaunchSelection>,
    ) -> Option<PresentationLaunchProgram> {
        if let Some(selection) = selection {
            return self
                .presented
                .clone()
                .or_else(|| explicit_program(selection.executable(), selection.arguments()));
        }
        match &self.default {
            LaunchPreferences::Available(preferences) => match preferences.program() {
                Some(DefaultLaunchProgram::Custom(program)) => {
                    PresentationLaunchProgram::from_executable(program.executable())
                }
                Some(DefaultLaunchProgram::SystemShell { .. }) | None => self.system_shell.clone(),
            },
            LaunchPreferences::Absent => self.system_shell.clone(),
            LaunchPreferences::Unavailable => None,
        }
    }

    /// Whether a launch with this selection runs an interactive Shell, judged from the argv it
    /// dispatches: the explicit selection, else the default profile's custom program with its
    /// literal argv. A default launch of the System shell always qualifies.
    pub(super) fn launches_interactive_shell(&self, selection: Option<&LaunchSelection>) -> bool {
        if let Some(selection) = selection {
            return interactive_shell_launch(selection.executable(), selection.arguments());
        }
        match &self.default {
            LaunchPreferences::Available(preferences) => match preferences.program() {
                Some(DefaultLaunchProgram::Custom(program)) => {
                    interactive_shell_launch(program.executable(), program.arguments())
                }
                Some(DefaultLaunchProgram::SystemShell { .. }) | None => true,
            },
            // The Worker refuses an unavailable capture, so its launch never starts.
            LaunchPreferences::Absent | LaunchPreferences::Unavailable => true,
        }
    }
}

/// The program an explicit launch starts, by file name. `/usr/bin/env` only sets its leading
/// `NAME=value` operands and starts the next one, so the launch names that program, as a terminal
/// names the process it shows. A POSIX shell that env starts to run one `-c` command line, as a
/// macOS custom command runs, names the program that line starts when its first word is a plain
/// program name; any other line names the shell. Windows never launches `/usr/bin/env`.
fn explicit_program(executable: &str, arguments: &[String]) -> Option<PresentationLaunchProgram> {
    if executable != "/usr/bin/env" {
        return PresentationLaunchProgram::from_executable(executable);
    }
    let mut operands = arguments.iter().map(String::as_str);
    let Some(program) = operands.by_ref().find(|operand| !operand.contains('=')) else {
        return PresentationLaunchProgram::from_executable(executable);
    };
    if program.starts_with('-') {
        return PresentationLaunchProgram::from_executable(executable);
    }
    let shell = program.rsplit('/').next().unwrap_or(program);
    let command = match operands.collect::<Vec<_>>().as_slice() {
        ["-l", "-c", command] | ["-c", command]
            if ["sh", "bash", "zsh", "dash", "ksh", "csh", "tcsh", "fish"].contains(&shell) =>
        {
            command_program(command)
        }
        _ => None,
    };
    command.or_else(|| PresentationLaunchProgram::from_executable(program))
}

/// The program a shell command line starts with: its first word, when that word is a plain file
/// name or path rather than an assignment, a quoted or expanded word, a shell keyword or a builtin
/// that starts no program of its name.
fn command_program(command: &str) -> Option<PresentationLaunchProgram> {
    const NOT_PROGRAMS: [&str; 18] = [
        ".", "builtin", "case", "cd", "command", "eval", "exec", "export", "for", "function", "if",
        "select", "set", "source", "time", "unset", "until", "while",
    ];
    let word = command.split_whitespace().next()?;
    let plain = word.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'/' | b'@')
    });
    if !plain || NOT_PROGRAMS.contains(&word) || word.ends_with('/') {
        return None;
    }
    PresentationLaunchProgram::from_executable(word)
}

/// The Host's interactive Shell launch rule (`interactive_shell.zig`) applied to a dispatched argv:
/// PowerShell with only plain interactive flags, `cmd` with only `/D` or `/Q`, and POSIX shells
/// with only interactive or login flags. Commands, scripts and other programs return false. The
/// PowerShell startup that Orchbee prepares later is never part of the argv, so its exact
/// recognition stays with the Host.
fn interactive_shell_launch(executable: &str, arguments: &[String]) -> bool {
    let filename = executable.rsplit(['/', '\\']).next().unwrap_or(executable);
    let name = match filename.split_at_checked(filename.len().saturating_sub(4)) {
        Some((stem, extension)) if extension.eq_ignore_ascii_case(".exe") => stem,
        _ => filename,
    };
    let flags: &[&str] =
        if name.eq_ignore_ascii_case("pwsh") || name.eq_ignore_ascii_case("powershell") {
            &["-NoLogo", "-NoProfile", "-NoExit", "-Login", "-l"]
        } else if name.eq_ignore_ascii_case("cmd") {
            &["/D", "/Q"]
        } else if ["sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh"].contains(&name) {
            &["-i", "-l", "-il", "-li", "--login", "--noprofile", "--norc"]
        } else {
            return false;
        };
    arguments
        .iter()
        .all(|argument| flags.iter().any(|flag| argument.eq_ignore_ascii_case(flag)))
}

#[cfg(test)]
mod tests;
