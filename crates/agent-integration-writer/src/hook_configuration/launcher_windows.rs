//! The Windows hook command: a PowerShell EncodedCommand whose script names the Capsule root as
//! base64 data and runs the bounded Node observation runtime.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::{Path, PathBuf};

const PREFIX: &str = "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand ";
const MARKER: &str = "# orchbee-agent-observation:";
const ROOT: &str = "$env:ORCHBEE_AGENT_HOOK_ROOT='";
const NODE: &str = r#"(async()=>{try{const root=Buffer.from(process.env.ORCHBEE_AGENT_HOOK_ROOT||'','base64').toString('utf8');if(!require('node:path').isAbsolute(root))return;const file=require('node:path').join(root,'bundle','scripts','forward-hook.mjs');process.argv=[process.execPath,file,'cursor'];await import(require('node:url').pathToFileURL(file).href)}catch{}})()"#;

pub(super) fn render(id: &str, root: &Path) -> Option<String> {
    let node = NODE.replace("'cursor'", &format!("'{id}'"));
    let encoded_root = STANDARD.encode(root.to_str()?.as_bytes());
    let output = if id == "letta" {
        ""
    } else {
        "[Console]::Out.Write('{}')\n"
    };
    let script = format!(
        "{MARKER}{id}\n{ROOT}{encoded_root}';\ntry {{ & node -e \"{node}\" 2>$null | Out-Null }} catch {{}}\n{output}"
    );
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    Some(format!("{PREFIX}{}", STANDARD.encode(utf16)))
}

/// The root a command names: `Ok(None)` when Orchbee never writes such a command, `Err` when it
/// carries Orchbee's marker in another shape.
pub(super) fn named_root(id: &str, command: &str) -> Result<Option<PathBuf>, ()> {
    let Some(encoded) = command.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let bytes = match STANDARD.decode(encoded) {
        Ok(bytes) => bytes,
        Err(_) => return Err(()),
    };
    if !bytes.len().is_multiple_of(2) {
        return Err(());
    }
    let words: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let script = String::from_utf16(&words).map_err(|_| ())?;
    if !script.starts_with(MARKER) {
        return Ok(None);
    }
    let encoded_root = script
        .strip_prefix(&format!("{MARKER}{id}\n"))
        .and_then(|s| s.strip_prefix(ROOT))
        .and_then(|s| s.split_once("';\n"))
        .map(|(root, _)| root)
        .ok_or(())?;
    Ok(Some(PathBuf::from(
        String::from_utf8(STANDARD.decode(encoded_root).map_err(|_| ())?).map_err(|_| ())?,
    )))
}
