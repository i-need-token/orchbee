//! Names that native chrome gives a launched program and its console default title.

use super::Locale;
use crate::terminal_title::{ChromeName, KnownShell, Program};
use std::borrow::Cow;

impl Locale {
    /// Program-set titles and Panel names stay as written. A launched program shows its name and
    /// keeps the command that cmd appends, in cmd's own `<title> - <command>` form.
    pub(crate) fn chrome_name<'a>(self, name: ChromeName<'a>) -> Cow<'a, str> {
        match name {
            ChromeName::Text(text) => Cow::Borrowed(text),
            ChromeName::Program {
                program,
                command: None,
            } => Cow::Borrowed(self.program_name(program)),
            ChromeName::Program {
                program,
                command: Some(command),
            } => Cow::Owned(format!("{} - {command}", self.program_name(program))),
        }
    }

    const fn program_name<'a>(self, program: Program<'a>) -> &'a str {
        match program {
            Program::Shell(shell) => self.shell_name(shell),
            Program::File(file) => file,
        }
    }

    /// Shell product names stay untranslated except Command Prompt, which Windows names in each
    /// display language.
    const fn shell_name(self, shell: KnownShell) -> &'static str {
        match (self, shell) {
            (Self::EnUs, KnownShell::PowerShell) => "PowerShell",
            (Self::ZhCn, KnownShell::PowerShell) => "PowerShell",
            (Self::EnUs, KnownShell::WindowsPowerShell) => "Windows PowerShell",
            (Self::ZhCn, KnownShell::WindowsPowerShell) => "Windows PowerShell",
            (Self::EnUs, KnownShell::CommandPrompt) => "Command Prompt",
            (Self::ZhCn, KnownShell::CommandPrompt) => "命令提示符",
            (Self::EnUs, KnownShell::Bash) => "Bash",
            (Self::ZhCn, KnownShell::Bash) => "Bash",
            (Self::EnUs, KnownShell::Zsh) => "Zsh",
            (Self::ZhCn, KnownShell::Zsh) => "Zsh",
        }
    }
}
