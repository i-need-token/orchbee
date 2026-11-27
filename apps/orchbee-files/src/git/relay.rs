//! The credential and editor relay's endpoint. Git starts this executable as its askpass program
//! and its editor; that helper connects to the endpoint of the process whose operation started
//! Git, each side proves the other runs as the same user before any content, and the helper sends
//! the operation's token with what Git asked. The engine routes the prompt and answers through
//! the connection; an answer is written to the helper and dropped, never logged or kept.
//!
//! The engine binds an endpoint only while an operation that may prompt runs. Its thread looks for
//! a waiting helper every 50 ms rather than blocking, so dropping the relay stops it without a
//! connection of its own: on Windows a connection that closes before it is accepted leaves a
//! blocking accept waiting.
//!
//! The wire is private to one build: the helper is this executable, started by the Git this
//! process started. A request is `OBFR`, a version byte, how Git asked, the 32-byte token and the
//! text with its length; an answer is its kind and the value with its length.

mod endpoint;

use endpoint::Endpoint;
use orchbee_files_protocol::PromptKind;
use orchbee_local_ipc::{LocalListener, LocalStream};
use orchbee_local_peer_credentials::require_same_effective_user;
use std::{
    io::{self, Read, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

const MAGIC: [u8; 4] = *b"OBFR";
const VERSION: u8 = 1;
/// The longest text a request or an answer carries.
const TEXT_LIMIT: usize = 16 * 1024;
/// The prompt text a Client receives at most, as the asking program wrote it.
const PROMPT_TEXT_LIMIT: usize = 4096;
/// How long a connected helper may take to send its request, and the relay its answer.
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// Helper connections served at once; more are refused, which fails their Git command.
const CONNECTION_LIMIT: usize = 32;
/// How often the endpoint looks for a waiting helper and for its stop.
const ACCEPT_POLL: Duration = Duration::from_millis(50);

/// One operation's token, which binds its helpers' prompts to it.
pub(super) type Token = [u8; 32];

/// How Git asked the helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Asked {
    /// As `GIT_ASKPASS` or `SSH_ASKPASS`, with the prompt as the argument.
    Askpass,
    /// As `SSH_ASKPASS` with `SSH_ASKPASS_PROMPT=confirm`.
    Confirm,
    /// As `GIT_EDITOR`, with the message file.
    Editor,
}

impl Asked {
    const fn byte(self) -> u8 {
        match self {
            Self::Askpass => 1,
            Self::Confirm => 2,
            Self::Editor => 3,
        }
    }

    const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::Askpass),
            2 => Some(Self::Confirm),
            3 => Some(Self::Editor),
            _ => None,
        }
    }
}

/// What a prompt asks the user for, from how Git asked and its text as VS Code's askpass reads it:
/// a passphrase, a host key to accept, a username, else a password.
fn prompt_kind(asked: Asked, text: &str) -> PromptKind {
    let lower = text.to_lowercase();
    match asked {
        Asked::Editor => PromptKind::Editor,
        _ if lower.contains("passphrase") => PromptKind::Passphrase,
        Asked::Confirm => PromptKind::HostKey,
        Asked::Askpass
            if lower.contains("authenticity of host") || lower.contains("continue connecting") =>
        {
            PromptKind::HostKey
        }
        Asked::Askpass if lower.starts_with("username") => PromptKind::Username,
        Asked::Askpass => PromptKind::Password,
    }
}

/// The user's answer as the helper writes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Answer {
    Value(String),
    /// An empty credential, or the editor's tab closed.
    Empty,
    /// The user cancelled the operation; the helper fails.
    Cancelled,
}

impl Answer {
    fn encode(&self) -> Vec<u8> {
        let (kind, value) = match self {
            Self::Value(value) => (1, value.as_bytes()),
            Self::Empty => (2, &[][..]),
            Self::Cancelled => (3, &[][..]),
        };
        let mut frame = vec![kind];
        frame.extend_from_slice(&length(value.len()));
        frame.extend_from_slice(value);
        frame
    }
}

/// One prompt a helper brought: the operation's token, what it asks, and where its answer goes.
/// Dropping `answer` unanswered fails the helper.
pub(super) struct Ask {
    pub(super) token: Token,
    pub(super) kind: PromptKind,
    /// The asking program's text, at most 4 KiB.
    pub(super) text: String,
    /// The message file of an editor prompt.
    pub(super) path: Option<String>,
    pub(super) answer: mpsc::Sender<Answer>,
}

/// The process's relay endpoint and the thread that admits its helpers.
pub(super) struct Relay {
    endpoint: Endpoint,
    stop: Arc<AtomicBool>,
}

impl Relay {
    /// Binds a fresh endpoint whose helpers' prompts reach `asks`.
    pub(super) fn bind(asks: impl Fn(Ask) + Send + Sync + 'static) -> io::Result<Self> {
        let endpoint = Endpoint::create(&hex(&random::<8>()?))?;
        let listener = endpoint.bind()?;
        let stop = Arc::new(AtomicBool::new(false));
        let relay = Self {
            endpoint,
            stop: Arc::clone(&stop),
        };
        let asks: Arc<dyn Fn(Ask) + Send + Sync> = Arc::new(asks);
        thread::Builder::new()
            .name("orchbee-files-relay".into())
            .spawn(move || accept(&listener, &stop, &asks))?;
        Ok(relay)
    }

    /// The address a helper connects to.
    pub(super) fn endpoint(&self) -> &Path {
        self.endpoint.path()
    }
}

impl Drop for Relay {
    /// Stops admitting helpers: within 50 ms the accept thread sees the stop, ends and closes the
    /// endpoint. Helpers already admitted keep their connections.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn accept(listener: &LocalListener, stop: &AtomicBool, asks: &Arc<dyn Fn(Ask) + Send + Sync>) {
    let serving = Arc::new(AtomicUsize::new(0));
    while !stop.load(Ordering::Acquire) {
        // No helper waiting, a peer that failed admission, or an endpoint that failed: look again
        // shortly rather than spin.
        let Ok((stream, _)) = listener.accept() else {
            thread::sleep(ACCEPT_POLL);
            continue;
        };
        if endpoint::blocking(&stream).is_err() {
            continue;
        }
        if serving.fetch_add(1, Ordering::AcqRel) >= CONNECTION_LIMIT {
            serving.fetch_sub(1, Ordering::AcqRel);
            continue;
        }
        let asks = Arc::clone(asks);
        let done = Arc::clone(&serving);
        let spawned = thread::Builder::new()
            .name("orchbee-files-relay-helper".into())
            .spawn(move || {
                let _ = serve(&stream, &*asks);
                done.fetch_sub(1, Ordering::AcqRel);
            });
        if spawned.is_err() {
            serving.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Serves one helper: proves its user, reads its request, hands the prompt to the engine and
/// writes the answer, or a cancellation once the engine drops the prompt.
fn serve(stream: &LocalStream, asks: &(dyn Fn(Ask) + Send + Sync)) -> io::Result<()> {
    require_same_effective_user(stream)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = stream;
    let (asked, token, text) = read_request(&mut reader)?;
    let (answer, answered) = mpsc::channel();
    let (text, path) = match asked {
        Asked::Editor => (String::new(), Some(text)),
        Asked::Askpass | Asked::Confirm => (truncated(text, PROMPT_TEXT_LIMIT), None),
    };
    asks(Ask {
        token,
        kind: prompt_kind(asked, &text),
        text,
        path,
        answer,
    });
    let answer = answered.recv().unwrap_or(Answer::Cancelled);
    let mut writer = stream;
    writer.write_all(&answer.encode())?;
    writer.flush()
}

/// The request a helper writes.
pub(super) fn encode_request(asked: Asked, token: &Token, text: &str) -> Vec<u8> {
    let text = text.as_bytes();
    let text = text.get(..TEXT_LIMIT).unwrap_or(text);
    let mut frame = Vec::with_capacity(4 + 2 + token.len() + 4 + text.len());
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&[VERSION, asked.byte()]);
    frame.extend_from_slice(token);
    frame.extend_from_slice(&length(text.len()));
    frame.extend_from_slice(text);
    frame
}

fn read_request(reader: &mut impl Read) -> io::Result<(Asked, Token, String)> {
    let mut header = [0; 4 + 2];
    reader.read_exact(&mut header)?;
    let asked = header
        .strip_prefix(&MAGIC)
        .and_then(|rest| match rest {
            [VERSION, asked] => Asked::from_byte(*asked),
            _ => None,
        })
        .ok_or(io::ErrorKind::InvalidData)?;
    let mut token = [0; 32];
    reader.read_exact(&mut token)?;
    let text = read_text(reader)?;
    Ok((asked, token, text))
}

/// Reads the answer the relay writes back.
pub(super) fn read_answer(reader: &mut impl Read) -> io::Result<Answer> {
    let mut kind = [0];
    reader.read_exact(&mut kind)?;
    let value = read_text(reader)?;
    match kind[0] {
        1 => Ok(Answer::Value(value)),
        2 => Ok(Answer::Empty),
        3 => Ok(Answer::Cancelled),
        _ => Err(io::ErrorKind::InvalidData.into()),
    }
}

fn read_text(reader: &mut impl Read) -> io::Result<String> {
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let length = usize::try_from(u32::from_le_bytes(length))
        .ok()
        .filter(|length| *length <= TEXT_LIMIT)
        .ok_or(io::ErrorKind::InvalidData)?;
    let mut text = vec![0; length];
    reader.read_exact(&mut text)?;
    String::from_utf8(text).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn length(length: usize) -> [u8; 4] {
    u32::try_from(length).unwrap_or(u32::MAX).to_le_bytes()
}

/// `text` cut to at most `limit` bytes, on a character boundary.
fn truncated(mut text: String, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

/// `N` random bytes from the operating system.
pub(super) fn random<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::from(io::ErrorKind::Other))?;
    Ok(bytes)
}

/// Lowercase hexadecimal digits of `bytes`.
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The bytes 64 hexadecimal digits spell.
pub(super) fn token_from_hex(text: &str) -> Option<Token> {
    let mut token = [0; 32];
    if text.len() != token.len() * 2 {
        return None;
    }
    for (index, byte) in token.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(token)
}
