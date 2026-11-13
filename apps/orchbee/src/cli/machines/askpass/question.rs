//! What a login question asks, read from `ssh`'s prompt alone, since `ssh` passes no kind: a
//! host-key confirmation with the offered key, a key passphrase, a password, or `other`. The
//! readings follow the OpenSSH 9.5 prompts M0 recorded; anything else is `other`, shown as `ssh`
//! worded it. Also the key a changed-host-key refusal names. Pure and platform-neutral.

use orchbee_machine_protocol::{HostKey, KeyFingerprint, Prompt, Token, hub::QuestionKind};

/// The most prompt bytes a question carries.
const PROMPT_BYTES: usize = 4096;

/// The prompt as a question carries it: cut at 4,096 bytes on a character boundary; `None` for an
/// empty prompt. Text that was not Unicode already reads U+FFFD.
pub(crate) fn prompt_text(text: &str) -> Option<Prompt> {
    let mut end = text.len().min(PROMPT_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Prompt::new(text.get(..end)?).ok()
}

/// The kind `prompt` asks.
pub(crate) fn classify(prompt: &str) -> QuestionKind {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("continue connecting") {
        return offered_key(prompt, " key fingerprint is ")
            .map_or(QuestionKind::Other, |host_key| QuestionKind::HostKey {
                host_key,
            });
    }
    let last = lower.trim_end();
    if lower.contains("passphrase for key") || lower.starts_with("enter passphrase") {
        QuestionKind::KeyPassphrase
    } else if last.ends_with("password:") {
        QuestionKind::Password
    } else {
        QuestionKind::Other
    }
}

/// The key a refusal of a changed host key names: OpenSSH prints "The fingerprint for the
/// <algorithm> key sent by the remote host is" and the fingerprint on the next line.
pub(crate) fn changed_key(stderr: &[u8]) -> Option<HostKey> {
    let text = String::from_utf8_lossy(stderr);
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim().strip_prefix("The fingerprint for the ") else {
            continue;
        };
        let algorithm = rest.strip_suffix(" key sent by the remote host is")?;
        return host_key(algorithm, lines.next()?);
    }
    None
}

/// The key a line such as "ED25519 key fingerprint is SHA256:…." offers, `marker` being the words
/// between the algorithm and the fingerprint.
fn offered_key(prompt: &str, marker: &str) -> Option<HostKey> {
    prompt.lines().find_map(|line| {
        let (before, fingerprint) = line.split_once(marker)?;
        host_key(before.split_whitespace().last()?, fingerprint)
    })
}

/// The algorithm as `ssh` names it, in lower case, and the fingerprint without its final period.
fn host_key(algorithm: &str, fingerprint: &str) -> Option<HostKey> {
    let fingerprint = fingerprint.trim();
    let fingerprint = fingerprint.strip_suffix('.').unwrap_or(fingerprint);
    Some(HostKey {
        algorithm: Token::new(algorithm.to_ascii_lowercase()).ok()?,
        fingerprint: KeyFingerprint::new(fingerprint).ok()?,
    })
}
