//! Encodes the literal remote executable and validated tunnel operands into a PowerShell
//! process launch that inherits binary streams directly. It performs no SSH connection or
//! process launch and never reparses protocol bytes as text.

use base64::{Engine as _, engine::general_purpose::STANDARD};
/// ProcessStartInfo inherits binary handles directly; the PowerShell pipeline must
/// never decode/re-encode protocol bytes as native-command text.
pub(crate) fn command(executable: &str, session: &str, cohort: &str) -> String {
    let executable = format!("'{}'", executable.replace('\'', "''"));
    let script = format!(
        "$ErrorActionPreference='Stop';try{{$s=New-Object System.Diagnostics.ProcessStartInfo;$s.FileName={executable};$s.Arguments='--machine-tunnel-v1 {session} {cohort}';$s.UseShellExecute=$false;$p=[System.Diagnostics.Process]::Start($s);$p.WaitForExit();exit $p.ExitCode}}catch{{exit 125}}"
    );
    let encoded = STANDARD.encode(
        script
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    format!("powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {encoded}")
}
