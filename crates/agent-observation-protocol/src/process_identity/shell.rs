//! Interactive shell recognition for the root of a terminal's process sample. Owners that decide
//! whether a terminal runs anything beyond its shell read the root through this one rule.
use super::native_agent_basename;

/// How a terminal's root process reads as an interactive shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellRoot {
    /// `cmd`, `powershell` or `pwsh` started with only interactive switches or the Worker's fixed
    /// initialization. Arbitrary command bodies and noninteractive launches do not qualify.
    Interactive,
    /// Another program, or a shell started to run something.
    Other,
    /// The root's image or arguments cannot be read.
    Unreadable,
}

/// Classifies a root from its actual image and its decoded arguments. `initialization` is the
/// Worker's fixed PowerShell initialization, which finishes before the interactive prompt.
#[must_use]
pub fn classify_shell_root(
    image: Option<&str>,
    argv: Option<&[String]>,
    initialization: &str,
) -> ShellRoot {
    let Some(image) = image else {
        return ShellRoot::Unreadable;
    };
    let shell = native_agent_basename(image);
    if !matches!(shell.as_str(), "cmd" | "powershell" | "pwsh") {
        return ShellRoot::Other;
    }
    let Some(args) = argv else {
        return ShellRoot::Unreadable;
    };
    let Some((first, switches)) = args.split_first() else {
        return ShellRoot::Other;
    };
    if native_agent_basename(first) != shell {
        return ShellRoot::Other;
    }
    let runtime_initialization = shell != "cmd"
        && switches
            .last()
            .is_some_and(|script| script == initialization);
    let flags = if switches.len() >= 3
        && runtime_initialization
        && switches[switches.len() - 2].eq_ignore_ascii_case("-Command")
        && switches[..switches.len() - 2]
            .iter()
            .any(|flag| flag.eq_ignore_ascii_case("-NoExit"))
    {
        &switches[..switches.len() - 2]
    } else {
        switches
    };
    let interactive = flags.iter().all(|arg| {
        let arg = arg.to_ascii_lowercase();
        if shell == "cmd" {
            matches!(
                arg.as_str(),
                "/d" | "/q" | "/a" | "/u" | "/v:on" | "/v:off" | "/e:on" | "/e:off"
            )
        } else {
            matches!(arg.as_str(), "-nologo" | "-noprofile" | "-noexit")
        }
    });
    if interactive {
        ShellRoot::Interactive
    } else {
        ShellRoot::Other
    }
}

/// The console host a Windows terminal's process tree carries beside its shell, never a program
/// the terminal runs.
#[must_use]
pub fn is_console_host(image: &str) -> bool {
    matches!(
        native_agent_basename(image).as_str(),
        "conhost" | "openconsole"
    )
}

/// The strict idle-shell rule that admits typing a command into a terminal: the sample is
/// complete, its root is an interactive shell, and every other row is a console host. A row whose
/// image cannot be read may be any program, so it refuses.
#[must_use]
pub fn idle_shell_only<'a>(
    root: ShellRoot,
    complete: bool,
    others: impl IntoIterator<Item = Option<&'a str>>,
) -> bool {
    complete
        && root == ShellRoot::Interactive
        && others
            .into_iter()
            .all(|image| image.is_some_and(is_console_host))
}
