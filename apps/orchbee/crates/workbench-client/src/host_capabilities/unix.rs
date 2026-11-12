//! Frozen Linux and macOS session facts behind the session-dependent host effects. Each route is
//! detected once from the declared terminal capability variables and the executables present, and
//! a missing route is a refusal, never an assumption.
use super::Capability;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::OnceLock,
};

mod tool;
pub(crate) use tool::{Exit, Overrun, run};

/// One external program and the fixed leading arguments it always receives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Program {
    pub(crate) path: PathBuf,
    pub(crate) arguments: &'static [&'static str],
}

/// Where a copied text goes: a desktop clipboard program, or an OSC 52 sequence that the outer
/// terminal applies to the clipboard of the machine it runs on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClipboardRoute {
    Program(Program),
    Terminal,
}

/// The desktop notification program; each message supplies its own trailing arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Notifier {
    AppleScript(PathBuf),
    NotifySend(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Facts {
    pub(crate) clipboard: Option<ClipboardRoute>,
    pub(crate) browser: Option<Program>,
    pub(crate) notifier: Option<Notifier>,
    /// The outer terminal is one whose notification sequence the terminal port can frame.
    pub(crate) terminal_notifications: bool,
    pub(crate) window_title: bool,
}

/// The facts of this session, detected on first use and fixed for the process lifetime.
pub(crate) fn facts() -> &'static Facts {
    static FACTS: OnceLock<Facts> = OnceLock::new();
    FACTS.get_or_init(|| {
        let env = |name: &str| std::env::var_os(name);
        let search = env("PATH");
        Facts::detect(env, cfg!(target_os = "macos"), |name| {
            locate(search.as_deref(), name)
        })
    })
}

pub(super) fn admits(capability: Capability) -> bool {
    let facts = facts();
    match capability {
        Capability::Clipboard => facts.clipboard.is_some(),
        Capability::Browser => facts.browser.is_some(),
        Capability::SystemNotification => facts.terminal_notifications || facts.notifier.is_some(),
        Capability::WindowTitle => facts.window_title,
        Capability::SavedMachines
        | Capability::DirectoryInput
        | Capability::AgentLaunch
        | Capability::PositionStorage
        | Capability::ConfigurationFile
        | Capability::Graphics => false,
    }
}

const PBCOPY: &str = "/usr/bin/pbcopy";
const OPEN: &str = "/usr/bin/open";
const OSASCRIPT: &str = "/usr/bin/osascript";

impl Facts {
    /// `locate` resolves a program name to an executable; macOS programs use fixed system paths.
    pub(crate) fn detect(
        env: impl Fn(&str) -> Option<OsString>,
        macos: bool,
        locate: impl Fn(&str) -> Option<PathBuf>,
    ) -> Self {
        let set = |name: &str| env(name).is_some_and(|value| !value.is_empty());
        let text = |name: &str| env(name).and_then(|value| value.into_string().ok());
        // A remote login's desktop belongs to another machine, so only X forwarding and the
        // outer terminal can reach the user's own screen.
        let remote = set("SSH_CONNECTION") || set("SSH_CLIENT") || set("SSH_TTY");
        let wayland = !macos && !remote && set("WAYLAND_DISPLAY");
        let x11 = !macos && set("DISPLAY");
        let term = text("TERM");
        let program = text("TERM_PROGRAM");
        let fixed = |path: &str| {
            let path = Path::new(path);
            executable(path).then(|| path.to_path_buf())
        };
        let program_at = |path: Option<PathBuf>, arguments: &'static [&'static str]| {
            path.map(|path| Program { path, arguments })
        };

        let clipboard_program = if macos {
            program_at((!remote).then(|| fixed(PBCOPY)).flatten(), &[])
        } else if wayland && let Some(path) = locate("wl-copy") {
            program_at(Some(path), &["--type", "text/plain;charset=utf-8"])
        } else if x11 {
            program_at(locate("xclip"), &["-selection", "clipboard", "-in"])
                .or_else(|| program_at(locate("xsel"), &["--clipboard", "--input"]))
        } else {
            None
        };
        let osc52 = osc52_terminal(&set, term.as_deref(), program.as_deref());
        let clipboard = match (remote, clipboard_program, osc52) {
            (true, _, true) => Some(ClipboardRoute::Terminal),
            (_, Some(program), _) => Some(ClipboardRoute::Program(program)),
            (false, None, true) => Some(ClipboardRoute::Terminal),
            (_, None, false) => None,
        };

        let desktop = !remote && (macos || wayland || x11);
        let browser = if !desktop {
            None
        } else if macos {
            program_at(fixed(OPEN), &[])
        } else {
            program_at(locate("xdg-open"), &[])
        };
        let notifier = if !desktop {
            None
        } else if macos {
            fixed(OSASCRIPT).map(Notifier::AppleScript)
        } else {
            locate("notify-send").map(Notifier::NotifySend)
        };
        let terminal_notifications = crate::notifications::terminal::admits(
            &crate::notifications::terminal::Host::detect(&env),
        );
        // The Linux virtual console prints an OSC title as text, and a dumb terminal has none.
        let window_title = term
            .as_deref()
            .is_some_and(|term| term != "dumb" && !term.starts_with("linux"));
        Self {
            clipboard,
            browser,
            notifier,
            terminal_notifications,
            window_title,
        }
    }
}

/// Terminals that apply an application's OSC 52 clipboard write under their default settings.
/// A multiplexer or a host pane in between consumes or drops the sequence, so neither admits it.
fn osc52_terminal(set: &impl Fn(&str) -> bool, term: Option<&str>, program: Option<&str>) -> bool {
    if ["TMUX", "ZELLIJ", "STY", "HERDR_ENV", "ORCHBEE_PANEL_ID"]
        .into_iter()
        .any(set)
    {
        return false;
    }
    matches!(program, Some("ghostty" | "WezTerm"))
        || ["KITTY_WINDOW_ID", "ALACRITTY_WINDOW_ID", "WT_SESSION"]
            .into_iter()
            .any(set)
        || term.is_some_and(|term| {
            matches!(
                term,
                "xterm-ghostty" | "xterm-kitty" | "foot" | "foot-extra" | "alacritty"
            ) || term.contains("wezterm")
        })
}

/// The first executable regular file of that name in an absolute `PATH` entry. Relative and empty
/// entries are skipped, so the current directory never supplies a program.
pub(crate) fn locate(search: Option<&OsStr>, name: &str) -> Option<PathBuf> {
    std::env::split_paths(search?)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| executable(candidate))
}

fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests;
