//! What a login question asks, read from `ssh`'s prompt alone, since `ssh` passes no kind. Only
//! `ssh`'s own local sentences, whole and from their first word, read as a kind: the key
//! passphrase sentence, the `user@host's password:` form, and the first-time host sentence with
//! the offered key. Anything else, a keyboard-interactive prompt with its `(user@host) ` prefix
//! included, is `other`. A password or new host that names another host than the Machine's is
//! `other` too, so it is never shown as the Machine's own. Also the key a changed-host-key refusal
//! names. Pure and platform-neutral.

use crate::machines::default_label;
use orchbee_machine_protocol::{HostKey, KeyFingerprint, Prompt, Token, hub::QuestionKind};

/// The most prompt bytes a question carries.
const PROMPT_BYTES: usize = 4096;
/// The first-time host sentence's first words, the host it names, and its last words.
const AUTHENTICITY: &str = "The authenticity of host '";
const NOT_ESTABLISHED: &str = "' can't be established";
const CONTINUE: &str = "Are you sure you want to continue connecting (yes/no";
/// The words between a fingerprint line's algorithm and its fingerprint.
const FINGERPRINT: &str = " key fingerprint is ";
/// The key passphrase sentence's first words and its end.
const PASSPHRASE: &str = "Enter passphrase for key '";
const PASSPHRASE_END: &str = "':";
/// The password sentence's end, after `user@host`.
const PASSWORD_END: &str = "'s password:";

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
    if let Some((_, host_key)) = new_host(prompt) {
        QuestionKind::HostKey { host_key }
    } else if passphrase(prompt) {
        QuestionKind::KeyPassphrase
    } else if password_host(prompt).is_some() {
        QuestionKind::Password
    } else {
        QuestionKind::Other
    }
}

/// The kind the hub puts to a person for the Machine whose `ssh` target is `target`: `kind` when
/// `prompt` reads as that kind and, for a password or a new host, names the target's host; `other`
/// otherwise, such as for a jump host's question, which must never read as the Machine's own nor
/// mark it untrusted.
pub(crate) fn attributed(kind: QuestionKind, prompt: &str, target: &str) -> QuestionKind {
    if classify(prompt) != kind {
        return QuestionKind::Other;
    }
    let named = match kind {
        QuestionKind::Password => password_host(prompt),
        QuestionKind::HostKey { .. } => new_host(prompt).map(|(host, _)| host),
        QuestionKind::KeyPassphrase | QuestionKind::Other => return kind,
    };
    let saved = default_label(target);
    if named.is_some_and(|host| bare(host).eq_ignore_ascii_case(&saved)) {
        kind
    } else {
        QuestionKind::Other
    }
}

/// The host and the offered key of OpenSSH's first-time host sentence: its first line names the
/// host, a later line is exactly `<algorithm> key fingerprint is <fingerprint>.`, and its last line
/// asks to continue connecting.
fn new_host(prompt: &str) -> Option<(&str, HostKey)> {
    let mut lines = prompt.trim_end().lines();
    let (host, _) = lines
        .next()?
        .strip_prefix(AUTHENTICITY)?
        .split_once(NOT_ESTABLISHED)?;
    let rest: Vec<&str> = lines.collect();
    let (last, middle) = rest.split_last()?;
    if !last.starts_with(CONTINUE) {
        return None;
    }
    let host_key = middle.iter().find_map(|line| {
        let (algorithm, fingerprint) = line.split_once(FINGERPRINT)?;
        let fingerprint = fingerprint.strip_suffix('.')?;
        let plain = |text: &str| !text.is_empty() && !text.contains(char::is_whitespace);
        (plain(algorithm) && plain(fingerprint))
            .then(|| host_key(algorithm, fingerprint))
            .flatten()
    })?;
    Some((host, host_key))
}

/// Whether `prompt` is OpenSSH's one-line key passphrase sentence.
fn passphrase(prompt: &str) -> bool {
    let line = prompt.trim_end();
    one_line(line) && line.starts_with(PASSPHRASE) && line.ends_with(PASSPHRASE_END)
}

/// The host of OpenSSH's one-line `user@host's password:` sentence.
fn password_host(prompt: &str) -> Option<&str> {
    let line = prompt.trim_end();
    if !one_line(line) {
        return None;
    }
    let (user, host) = line.strip_suffix(PASSWORD_END)?.rsplit_once('@')?;
    let plain = |text: &str, also: &[char]| {
        !text.is_empty()
            && text
                .chars()
                .all(|character| character.is_ascii_graphic() && !also.contains(&character))
    };
    (plain(user, &['(']) && plain(host, &['\'', '@', '('])).then_some(host)
}

fn one_line(line: &str) -> bool {
    !line.contains(['\n', '\r'])
}

/// The host a sentence names, without the address after it in parentheses or the port of a
/// bracketed `[host]:port`.
fn bare(named: &str) -> &str {
    let named = named.split_once(" (").map_or(named, |(host, _)| host);
    match named.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map_or(rest, |(host, _)| host),
        None => named,
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

/// The algorithm as `ssh` names it, in lower case, and the fingerprint without its final period.
fn host_key(algorithm: &str, fingerprint: &str) -> Option<HostKey> {
    let fingerprint = fingerprint.trim();
    let fingerprint = fingerprint.strip_suffix('.').unwrap_or(fingerprint);
    Some(HostKey {
        algorithm: Token::new(algorithm.to_ascii_lowercase()).ok()?,
        fingerprint: KeyFingerprint::new(fingerprint).ok()?,
    })
}
