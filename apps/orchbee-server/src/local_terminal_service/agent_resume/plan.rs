//! Each provider's program and the exact text and records one resume types into a restored shell.
//! It builds input only; deciding whether to type it belongs to admission and the driver, and the
//! argument tokens come from the Session's closed conversation id table.
use crate::session_coordinator::resume_arguments;
use orchbee_client_server_protocol::{
    SendKey, SendKeyStroke, SendPasteContent, SendRecord, SendRecords, SendStrokeKey,
};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};

/// The restored shell a resume line is written for: one the Host launched with prompt marks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResumeShell {
    /// `powershell` or `pwsh` running the Worker's fixed initialization, which strips the run
    /// prefix and echoes its token in the command's start and finish markers.
    ManagedPowerShell,
    /// An interactive `cmd` whose prompt the Host marks.
    Cmd,
}

impl ResumeShell {
    pub(super) const fn powershell(self) -> bool {
        matches!(self, Self::ManagedPowerShell)
    }
}

/// The provider's own program. Each resolves both in cmd and as a PowerShell application or
/// external script, and none is a cmd internal command or a generic name such as `agent`.
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

/// The line typed for `session`, with the argument tokens of [`resume_arguments`], which shares
/// one closed table with the reader of a running Agent's command line. PowerShell resolves the
/// program as an application or script, so no alias, function or cmdlet of that name runs, and
/// single-quotes every argument token, so each reaches the program verbatim and none is read as a
/// parameter. cmd takes the tokens bare: each
/// is inert there. `token` is the run prefix's token and is given only for managed PowerShell.
pub(super) fn text(
    session: &RetainedAgentSession,
    shell: ResumeShell,
    token: Option<&str>,
) -> String {
    let program = resume_program(session.provider());
    let arguments = resume_arguments(session);
    let line = if shell.powershell() {
        let quoted: Vec<String> = arguments.iter().map(|a| format!("'{a}'")).collect();
        format!(
            "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1) {}",
            quoted.join(" ")
        )
    } else {
        format!("{program} {}", arguments.join(" "))
    };
    match token {
        Some(token) => format!("<#orchbee-run:{token}#> {line}"),
        None => line,
    }
}

/// A fresh run token: 16 bytes of operating-system entropy as 32 lowercase hex digits. Nothing
/// when entropy is unavailable, which skips the resume rather than reusing a predictable token.
pub(super) fn run_token() -> Option<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(format!("{:032x}", u128::from_be_bytes(bytes)))
}

/// Paste, then Right for PowerShell, then Enter, in one submission: the one prompt line the Host's
/// gate admits, and the same records an Agent start types into the same shells.
pub(super) fn records(text: &str, powershell: bool) -> Option<SendRecords> {
    let mut records = vec![SendRecord::Paste {
        content: SendPasteContent::new(text.as_bytes().to_vec()).ok()?,
    }];
    if powershell {
        records.push(SendRecord::Stroke(
            SendKeyStroke::new(SendStrokeKey::Named(SendKey::Right), 0).ok()?,
        ));
    }
    records.push(SendRecord::Stroke(
        SendKeyStroke::new(SendStrokeKey::Named(SendKey::Enter), 0).ok()?,
    ));
    SendRecords::new(records).ok()
}
