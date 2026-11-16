//! Each provider's program and the exact text one resume types into a restored shell. It builds
//! input only; deciding whether to type it belongs to admission and the driver, and the argument
//! tokens come from the Session's closed conversation id table.
use crate::local_terminal_service::prompt_readiness::MarkedShell;
use crate::session_coordinator::resume_arguments;
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};

/// The provider's own program. Each resolves both in cmd and as a PowerShell application or
/// external script, and as a command zsh and bash find on `PATH`; none is a cmd internal command
/// or a generic name such as `agent`.
const fn resume_program(provider: AgentResumeProvider) -> &'static str {
    use AgentResumeProvider as P;
    match provider {
        P::ClaudeCode => "claude",
        P::Codex => "codex",
        P::Grok => "grok",
        P::OpenCode => "opencode",
        P::Kilo => "kilo",
        P::Mimo => "mimo",
        P::Cursor => "cursor-agent",
        P::QoderCli => "qodercli",
        P::Antigravity => "agy",
        P::Droid => "droid",
        P::Kimi => "kimi",
        P::Letta => "letta",
        P::Devin => "devin",
        P::Hermes => "hermes",
        P::Pi => "pi",
        P::Omp => "omp",
        P::Gemini => "gemini",
        P::GitHubCopilot => "copilot",
        P::QwenCode => "qwen",
        P::Cline => "cline",
    }
}

/// Whether `shell` strips a run prefix and runs the line with its token: managed PowerShell, zsh
/// and bash do; cmd takes the line bare.
pub(super) const fn takes_run_prefix(shell: MarkedShell) -> bool {
    match shell {
        MarkedShell::ManagedPowerShell | MarkedShell::ManagedZsh | MarkedShell::ManagedBash => true,
        MarkedShell::Cmd => false,
    }
}

/// The line typed for `session`, with the argument tokens of [`resume_arguments`], which shares
/// one closed table with the reader of a running Agent's command line. PowerShell resolves the
/// program as an application or script, so no alias, function or cmdlet of that name runs, and
/// single-quotes every argument token, so each reaches the program verbatim and none is read as a
/// parameter. cmd takes the tokens bare: each is inert there. zsh and bash take the words as an
/// Agent start types them, through the shared POSIX quoting, so the person's own alias or function
/// of the program's name runs, as for a line they typed; every token is a plain word there.
/// `token` is the run prefix's token and is given exactly when the shell
/// [takes one](takes_run_prefix): PowerShell's `<#orchbee-run:<token>#> ` comment, or zsh's and
/// bash's `: orchbee-run:<token>; ` no-op, which the managed shells strip. Nothing when a word
/// cannot be typed.
pub(super) fn text(
    session: &RetainedAgentSession,
    shell: MarkedShell,
    token: Option<&str>,
) -> Option<String> {
    let program = resume_program(session.provider());
    let arguments = resume_arguments(session);
    let line = match shell {
        MarkedShell::ManagedPowerShell => {
            let quoted: Vec<String> = arguments.iter().map(|a| format!("'{a}'")).collect();
            format!(
                "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1) {}",
                quoted.join(" ")
            )
        }
        MarkedShell::Cmd => format!("{program} {}", arguments.join(" ")),
        MarkedShell::ManagedZsh | MarkedShell::ManagedBash => {
            orchbee_local_runtime::posix_command_line(program, &arguments)?
        }
    };
    Some(match (token, shell) {
        (Some(token), MarkedShell::ManagedPowerShell) => format!("<#orchbee-run:{token}#> {line}"),
        (Some(token), MarkedShell::ManagedZsh | MarkedShell::ManagedBash) => {
            format!(": orchbee-run:{token}; {line}")
        }
        (_, MarkedShell::Cmd) | (None, _) => line,
    })
}

/// A fresh run token: 16 bytes of operating-system entropy as 32 lowercase hex digits. Nothing
/// when entropy is unavailable, which skips the resume rather than reusing a predictable token.
pub(super) fn run_token() -> Option<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(format!("{:032x}", u128::from_be_bytes(bytes)))
}
