//! The POSIX hook command, which the providers run through `/bin/sh -c`, `bash -c` or `zsh -c`.
//! Node evaluates a fixed script that reads the Capsule root as base64 data from its only
//! argument, so no path ever becomes shell or JavaScript source, and the line parses the same in
//! sh, bash, zsh and fish. A missing Node or Capsule stays silent and never fails the hook.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::{Path, PathBuf};

/// `node -e "` and the script's leading comment, which carries the marker and the provider.
const PREFIX: &str = "node -e \"/*orchbee-agent-observation:";
/// The script after the marker comment. It avoids `$`, `` ` ``, `\`, `"` and `!`, the characters
/// a double-quoted shell word would treat specially.
const SCRIPT: &str = "*/(async()=>{try{const p=require('node:path');const r=Buffer.from(process.argv[1]||'','base64').toString('utf8');if(p.isAbsolute(r)){const f=p.join(r,'bundle','scripts','forward-hook.mjs');process.argv=[process.execPath,f,'cursor'];await import(require('node:url').pathToFileURL(f).href)}}catch{}})()";

pub(super) fn render(id: &str, root: &Path) -> Option<String> {
    let script = SCRIPT.replace("'cursor'", &format!("'{id}'"));
    let encoded_root = STANDARD.encode(root.to_str()?.as_bytes());
    // Letta reads hook stdout as prompt context, so its hook prints nothing; every other provider
    // expects an empty JSON object.
    let output = if id == "letta" {
        " || true"
    } else {
        "; printf '{}'"
    };
    Some(format!(
        "{PREFIX}{id}{script}\" '{encoded_root}' >/dev/null 2>&1{output}"
    ))
}

/// The root a command names: `Ok(None)` when Orchbee never writes such a command, `Err` when it
/// carries Orchbee's marker in another shape.
pub(super) fn named_root(id: &str, command: &str) -> Result<Option<PathBuf>, ()> {
    let Some(rest) = command.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let (_, operand) = rest
        .strip_prefix(id)
        .filter(|rest| rest.starts_with("*/"))
        .and_then(|rest| rest.split_once("\" '"))
        .ok_or(())?;
    let (encoded_root, _) = operand.split_once('\'').ok_or(())?;
    Ok(Some(PathBuf::from(
        String::from_utf8(STANDARD.decode(encoded_root).map_err(|_| ())?).map_err(|_| ())?,
    )))
}
