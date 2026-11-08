//! Keeps each terminal's recent output beside the checkpoint so a restart can show it again. It
//! owns the payload format, the line and byte caps, the file number a checkpoint names, and an
//! in-memory index of what it last wrote per terminal, so a save writes a file only when the
//! content changed. Numbers count up from the highest kept one, so a file name never carries a
//! digest. Storage effects go through the narrow store port; capture and launch belong to the
//! terminal service.

use orchbee_workbench_protocol::{PresentationBinding, TerminalPresentationId};
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
};

/// The most logical lines kept per terminal.
pub(crate) const LINES: u16 = 2000;
/// The most output bytes kept per terminal; the oldest lines go first. The Host admits at most
/// 1 MiB including the divider line, which this reserve covers.
pub(crate) const MAXIMUM_OUTPUT_BYTES: usize = 1024 * 1024 - 1024;
const MAGIC: &[u8] = b"orchbee.terminal-output 1\n";

/// Where kept output persists: complete payloads named by a nonzero number.
pub(crate) trait TerminalOutputStore: Send + Sync {
    fn tokens(&self) -> io::Result<Vec<u64>>;
    fn read(&self, token: u64) -> io::Result<Option<Vec<u8>>>;
    fn write(&self, token: u64, payload: &[u8]) -> io::Result<()>;
    fn remove(&self, token: u64) -> io::Result<()>;
}

#[cfg(windows)]
impl TerminalOutputStore for Mutex<orchbee_local_runtime::SessionStore> {
    fn tokens(&self) -> io::Result<Vec<u64>> {
        lock(self)?.terminal_output_tokens()
    }
    fn read(&self, token: u64) -> io::Result<Option<Vec<u8>>> {
        lock(self)?.read_terminal_output(token)
    }
    fn write(&self, token: u64, payload: &[u8]) -> io::Result<()> {
        lock(self)?.write_terminal_output(token, payload)
    }
    fn remove(&self, token: u64) -> io::Result<()> {
        lock(self)?.remove_terminal_output(token)
    }
}

#[cfg(windows)]
fn lock(
    store: &Mutex<orchbee_local_runtime::SessionStore>,
) -> io::Result<std::sync::MutexGuard<'_, orchbee_local_runtime::SessionStore>> {
    store
        .lock()
        .map_err(|_| io::Error::other("checkpoint store failed"))
}

/// What was last written for one live terminal.
struct Kept {
    binding: PresentationBinding,
    /// The terminal content generation and revision the file reflects.
    version: (u64, u64),
    token: u64,
    /// The written Presentation and content, compared to skip an unchanged rewrite.
    fingerprint: u64,
}

/// Every live terminal's last write, and the next free file number once one was needed.
#[derive(Default)]
struct Index {
    kept: BTreeMap<(String, String), Kept>,
    next: Option<u64>,
}

#[cfg(test)]
mod tests;

pub(crate) struct TerminalOutput {
    store: Arc<dyn TerminalOutputStore>,
    index: Mutex<Index>,
}

fn key(binding: &PresentationBinding) -> (String, String) {
    (
        binding.worker_id.as_str().to_owned(),
        binding.terminal_id.as_str().to_owned(),
    )
}

impl TerminalOutput {
    pub(crate) fn new(store: Arc<dyn TerminalOutputStore>) -> Self {
        Self {
            store,
            index: Mutex::new(Index::default()),
        }
    }

    fn index(&self) -> io::Result<std::sync::MutexGuard<'_, Index>> {
        self.index
            .lock()
            .map_err(|_| io::Error::other("terminal output index failed"))
    }

    /// A number no kept file and no earlier write in this Session uses.
    fn next_number(&self) -> io::Result<u64> {
        let mut index = self.index()?;
        let number = match index.next {
            Some(next) => next,
            // The first write continues after the highest number a kept file uses.
            None => self
                .store
                .tokens()?
                .into_iter()
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(io::ErrorKind::InvalidData)?,
        };
        index.next = Some(number.checked_add(1).ok_or(io::ErrorKind::InvalidData)?);
        Ok(number)
    }

    /// The content version last written for `binding`, so an unchanged terminal is not reread.
    pub(crate) fn known(&self, binding: &PresentationBinding) -> Option<(u64, u64)> {
        self.index()
            .ok()?
            .kept
            .get(&key(binding))
            .filter(|kept| &kept.binding == binding)
            .map(|kept| kept.version)
    }

    /// Keeps `lines` for the terminal `binding` shows in `presentation`. The file is rewritten
    /// only when the capped content differs from what that terminal last kept; blank content
    /// keeps nothing.
    pub(crate) fn record(
        &self,
        binding: &PresentationBinding,
        presentation: &TerminalPresentationId,
        version: (u64, u64),
        lines: &[String],
        saved_at: u64,
    ) -> io::Result<()> {
        let Some(content) = content(lines) else {
            self.index()?.kept.remove(&key(binding));
            return Ok(());
        };
        let fingerprint = fingerprint(presentation, &content);
        let unchanged = self
            .index()?
            .kept
            .get(&key(binding))
            .filter(|kept| &kept.binding == binding && kept.fingerprint == fingerprint)
            .map(|kept| kept.token);
        let token = match unchanged {
            Some(token) => token,
            None => {
                let token = self.next_number()?;
                self.store
                    .write(token, &encode(presentation, saved_at, &content))?;
                token
            }
        };
        self.index()?.kept.insert(
            key(binding),
            Kept {
                binding: binding.clone(),
                version,
                token,
                fingerprint,
            },
        );
        Ok(())
    }

    /// The token each of `bindings` currently keeps, for the checkpoint to name.
    pub(crate) fn tokens_for(
        &self,
        bindings: &[PresentationBinding],
    ) -> Vec<(PresentationBinding, u64)> {
        let Ok(index) = self.index() else {
            return Vec::new();
        };
        bindings
            .iter()
            .filter_map(|binding| {
                let kept = index
                    .kept
                    .get(&key(binding))
                    .filter(|k| &k.binding == binding)?;
                Some((binding.clone(), kept.token))
            })
            .collect()
    }

    /// After a committed save, removes every kept file the checkpoint does not name and forgets
    /// terminals whose output it no longer names.
    pub(crate) fn after_publish(&self, named: &[u64]) -> io::Result<()> {
        self.index()?
            .kept
            .retain(|_, kept| named.contains(&kept.token));
        for token in self.store.tokens()? {
            if !named.contains(&token) {
                self.store.remove(token)?;
            }
        }
        Ok(())
    }

    /// Removes every kept file and forgets every terminal, when replay is turned off.
    pub(crate) fn remove_all(&self) -> io::Result<()> {
        self.after_publish(&[])
    }

    /// The output kept under `token` for `presentation`, and when it was kept. A damaged file or
    /// one that belongs to another Presentation is removed and never replayed; a failed read
    /// leaves the file for the next save to judge.
    pub(crate) fn restored(
        &self,
        presentation: &TerminalPresentationId,
        token: u64,
    ) -> Option<(Vec<u8>, u64)> {
        let payload = self.store.read(token).ok()?;
        match payload.as_deref().and_then(decode) {
            Some((owner, saved_at, content)) if owner == presentation.as_str() => {
                Some((content.to_vec(), saved_at))
            }
            _ => {
                let _ = self.store.remove(token);
                None
            }
        }
    }
}

/// The bytes the Host draws before the shell starts: the kept output, then the divider, dim.
pub(crate) fn seed(content: &[u8], divider: &str) -> Vec<u8> {
    let mut seed = Vec::with_capacity(content.len() + divider.len() + 16);
    seed.extend_from_slice(content);
    seed.extend_from_slice(b"\r\n\x1b[0;2m");
    seed.extend_from_slice(divider.as_bytes());
    seed.extend_from_slice(b"\x1b[0m");
    seed
}

/// The last lines joined by CR LF within the byte cap, oldest dropped first. Characters the Host
/// would refuse are dropped; the only escapes left are the style sequences the capture wrote.
fn content(lines: &[String]) -> Option<Vec<u8>> {
    let mut kept: Vec<String> = Vec::new();
    let mut bytes = 0usize;
    for line in lines.iter().rev().take(usize::from(LINES)) {
        let line: String = line
            .chars()
            .filter(|c| *c == '\x1b' || !c.is_control())
            .collect();
        let added = line.len() + if kept.is_empty() { 0 } else { 2 };
        if bytes + added > MAXIMUM_OUTPUT_BYTES {
            break;
        }
        bytes += added;
        kept.push(line);
    }
    kept.reverse();
    let first = kept.iter().position(|line| !visible(line).is_empty())?;
    Some(kept[first..].join("\r\n").into_bytes())
}

/// A line's text without its style sequences.
fn visible(line: &str) -> String {
    let mut text = String::new();
    let mut escape = false;
    for c in line.chars() {
        match (escape, c) {
            (false, '\x1b') => escape = true,
            (true, 'm') => escape = false,
            (true, _) => {}
            (false, c) => text.push(c),
        }
    }
    text.trim().to_owned()
}

/// FNV-1a over the Presentation and content, kept in memory only: equal content keeps its file.
fn fingerprint(presentation: &TerminalPresentationId, content: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in presentation
        .as_str()
        .as_bytes()
        .iter()
        .chain(b"\n")
        .chain(content)
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn encode(presentation: &TerminalPresentationId, saved_at: u64, content: &[u8]) -> Vec<u8> {
    let mut payload = MAGIC.to_vec();
    payload.extend_from_slice(presentation.as_str().as_bytes());
    payload.push(b'\n');
    payload.extend_from_slice(saved_at.to_string().as_bytes());
    payload.push(b'\n');
    payload.extend_from_slice(content);
    payload
}

fn decode(payload: &[u8]) -> Option<(&str, u64, &[u8])> {
    let rest = payload.strip_prefix(MAGIC)?;
    let end = rest.iter().position(|byte| *byte == b'\n')?;
    let owner = std::str::from_utf8(&rest[..end]).ok()?;
    let rest = &rest[end + 1..];
    let end = rest.iter().position(|byte| *byte == b'\n')?;
    let saved_at = std::str::from_utf8(&rest[..end]).ok()?.parse().ok()?;
    let content = &rest[end + 1..];
    (!content.is_empty() && content.len() <= MAXIMUM_OUTPUT_BYTES)
        .then_some((owner, saved_at, content))
}
