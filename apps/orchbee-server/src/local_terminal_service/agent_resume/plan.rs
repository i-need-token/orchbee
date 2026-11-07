//! The closed provider table and the exact text and records one resume types into a restored
//! shell. It builds input only; deciding whether to type it belongs to admission and the driver.
use orchbee_client_server_protocol::{
    SendKey, SendKeyStroke, SendPasteContent, SendRecord, SendRecords, SendStrokeKey,
};
use orchbee_server::{AgentResumeProvider, RetainedAgentSession};

/// The restored shell a resume line is written for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ResumeShell {
    /// `powershell` or `pwsh` running the Worker's fixed initialization, which strips the run
    /// prefix and echoes its token in the command's start and finish markers.
    ManagedPowerShell,
    /// Any other interactive `powershell` or `pwsh`.
    PowerShell,
    /// An interactive `cmd`.
    Cmd,
}

impl ResumeShell {
    pub(super) const fn powershell(self) -> bool {
        matches!(self, Self::ManagedPowerShell | Self::PowerShell)
    }
}

/// The provider's own program and the arguments before the id. The table is closed: a provider
/// joins it only with a witness that its hook session key is the id this command resumes.
const fn command(provider: AgentResumeProvider) -> (&'static str, &'static str) {
    match provider {
        AgentResumeProvider::ClaudeCode => ("claude", "--resume"),
        AgentResumeProvider::Codex => ("codex", "resume"),
    }
}

/// The line typed for `session`. PowerShell resolves the program as an application or script, so
/// no alias, function or cmdlet of that name runs. `token` is the run prefix's token and is given
/// only for managed PowerShell. The id is a hyphenated UUID, inert in both shells and never an
/// option, so it is typed bare.
pub(super) fn text(
    session: &RetainedAgentSession,
    shell: ResumeShell,
    token: Option<&str>,
) -> String {
    let (program, verb) = command(session.provider());
    let id = session.session_id();
    let line = if shell.powershell() {
        format!(
            "& (Get-Command '{program}' -CommandType Application,ExternalScript -ErrorAction Stop | Select-Object -First 1) {verb} {id}"
        )
    } else {
        format!("{program} {verb} {id}")
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

/// Paste, then Right for PowerShell, then Enter, in one submission: the same records an Agent
/// start types into the same shells.
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
