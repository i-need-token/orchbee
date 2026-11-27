//! OBFL, the private link between one Server and the `orchbee-files` process it admitted: a
//! 12-byte header (`OBFL`, version, kind, length) and one typed body per kind. Both ends come
//! from one build, so the link has no compatibility window: a hello from another build is
//! refused and any malformed frame ends the link. This codec frames and validates; endpoint
//! admission, token comparison, relay and scope remain with the two processes.

use std::{
    fmt,
    io::{self, Read, Write},
};

use crate::{CallId, ConnectionId, LaneId, LinkToken, WorkspaceId};

/// The link's frame magic.
pub const LINK_MAGIC: [u8; 4] = *b"OBFL";
/// The only link version; both ends come from one build.
pub const LINK_VERSION: u16 = 1;
/// The largest body one frame carries.
pub const MAXIMUM_LINK_PAYLOAD_BYTES: usize = 256 * 1024;
/// The longest environment name a hello carries, as the handshake's name grammar bounds it.
pub const MAXIMUM_ENVIRONMENT_BYTES: usize = 64;
/// The longest product version a hello carries, as the handshake bounds it.
pub const MAXIMUM_VERSION_BYTES: usize = 64;
/// The longest Workspace root a scope update carries, in UTF-8 bytes.
pub const MAXIMUM_ROOT_BYTES: usize = 4096;

const HEADER_BYTES: usize = 12;
const COMMIT_BYTES: usize = 20;
const FINGERPRINT_BYTES: usize = 32;
const TOKEN_BYTES: usize = 32;
const WORKSPACE_BYTES: usize = 16;

const HELLO: u16 = 1;
const ADMITTED: u16 = 2;
const REFUSED: u16 = 3;
const REQUEST: u16 = 4;
const FRAME: u16 = 5;
const CLOSE: u16 = 6;
const CANCEL: u16 = 7;
const SCOPE: u16 = 8;
const CONNECTION_LOST: u16 = 9;
const LANE_CLOSED: u16 = 10;
const AGENT_SETTLED: u16 = 11;
const DRAIN: u16 = 12;
const ACTIVITY: u16 = 13;

/// Which end sent a frame. Each kind has one sender; a frame from the other end is malformed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkSide {
    /// The admitting Server.
    Server,
    /// The `orchbee-files` process.
    Files,
}

/// The build a hello describes. Two ends match only when every member is equal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildIdentity {
    version: String,
    commit: Option<[u8; COMMIT_BYTES]>,
    environment: String,
    fingerprint: [u8; FINGERPRINT_BYTES],
}

impl BuildIdentity {
    /// The identity of product version `version` (1–64 visible ASCII bytes) compiled from
    /// `commit`, when release packaging named one, for the environment `environment` (1–64 bytes
    /// of `a`–`z`, `0`–`9` and `-`), whose protocol source has the development fingerprint
    /// `fingerprint`.
    ///
    /// # Errors
    ///
    /// [`LinkError::InvalidBuild`] for a version or environment outside its grammar.
    pub fn new(
        version: &str,
        commit: Option<[u8; COMMIT_BYTES]>,
        environment: &str,
        fingerprint: [u8; FINGERPRINT_BYTES],
    ) -> Result<Self, LinkError> {
        if !valid_version(version.as_bytes()) || !valid_environment(environment.as_bytes()) {
            return Err(LinkError::InvalidBuild);
        }
        Ok(Self {
            version: version.to_owned(),
            commit,
            environment: environment.to_owned(),
            fingerprint,
        })
    }

    /// The product version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The source commit, when release packaging named one.
    #[must_use]
    pub const fn commit(&self) -> Option<&[u8; COMMIT_BYTES]> {
        self.commit.as_ref()
    }

    /// The compiled environment name.
    #[must_use]
    pub fn environment(&self) -> &str {
        &self.environment
    }

    /// The development protocol fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &[u8; FINGERPRINT_BYTES] {
        &self.fingerprint
    }
}

/// The first frame a started process sends: its build and the token its start received.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkHello {
    /// The sender's build.
    pub build: BuildIdentity,
    /// The token the sender's start passed on its standard input.
    pub token: LinkToken,
}

/// Why a Server refused a hello whose token it admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkRefusal {
    /// The process comes from another build; the link has no compatibility window.
    BuildMismatch,
}

/// The Session grants a requesting Client connection holds, beyond reading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrantClass {
    /// Terminal creation only: reads and subscriptions.
    TerminalCreate,
    /// Terminal creation and terminal lifecycle: also mutations and relay answers.
    Lifecycle,
}

/// One Client record the Server relays, in the envelope that admitted it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkRequest {
    /// The Server's number for this call; every reply names it.
    pub call: CallId,
    /// The Client connection that sent the record.
    pub connection: ConnectionId,
    /// The connection's lane the record arrived on.
    pub lane: LaneId,
    /// The Workspace the request names, which the Server read from it; a Put names none.
    pub workspace: Option<WorkspaceId>,
    /// What the sending connection's Session grants allow.
    pub grant: GrantClass,
    /// The complete OBFB record: a Request or a Put, header included.
    pub record: Vec<u8>,
}

/// How a reply frame relates to its call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameKind {
    /// A request call's one answer.
    Result,
    /// A subscription's complete state.
    Snapshot,
    /// A subscription's change since the previous frame.
    Update,
}

/// One reply record for a relayed call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkFrame {
    /// The call the record answers.
    pub call: CallId,
    /// How the record relates to the call.
    pub kind: FrameKind,
    /// The complete OBFB record: a Result, Notice or Chunk, header included.
    pub record: Vec<u8>,
}

/// How a relayed call ended, as its Client stream closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallClose {
    /// Every reply was sent.
    Complete,
    /// The call stopped at its Client's cancel.
    Cancelled,
    /// The operation is unknown or unserved.
    Unsupported,
    /// The record did not decode or validate.
    InvalidRequest,
    /// A bound refused the call before it ran.
    Capacity,
    /// A drain refused the call before it ran.
    Stopping,
    /// The call ran, and how it ended cannot be reported.
    OutcomeUnknown,
}

/// What a Workspace's scope is after an update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceScope {
    /// The Workspace is open and its root is this absolute native path.
    Root(String),
    /// The Workspace is open and has no root.
    NoRoot,
    /// The Workspace closed; only grants its connections still hold remain.
    Closed,
}

/// A Workspace's scope, which precedes every request it authorizes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeUpdate {
    /// The Workspace.
    pub workspace: WorkspaceId,
    /// Its scope from now on.
    pub scope: WorkspaceScope,
}

/// Why a Server asks the process to end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrainReason {
    /// Nothing used the process for the idle period.
    Idle,
    /// The Session stops.
    Session,
}

/// One OBFL frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkMessage {
    /// Files → Server, first: the build and the start's token.
    Hello(LinkHello),
    /// Server → Files: the hello was admitted; relayed traffic follows.
    Admitted,
    /// Server → Files: the hello was refused; the link ends.
    Refused(LinkRefusal),
    /// Server → Files: one relayed Client record.
    Request(LinkRequest),
    /// Files → Server: one reply record.
    Frame(LinkFrame),
    /// Files → Server: a call ended; no frame for it follows.
    Close {
        /// The call.
        call: CallId,
        /// How it ended.
        reason: CallClose,
    },
    /// Server → Files: the call's Client cancelled it.
    Cancel {
        /// The call.
        call: CallId,
    },
    /// Server → Files: a Workspace's scope changed.
    Scope(ScopeUpdate),
    /// Server → Files: a Client connection ended, with everything it held.
    ConnectionLost {
        /// The connection.
        connection: ConnectionId,
    },
    /// Server → Files: a connection closed one lane, with every transfer on it.
    LaneClosed {
        /// The connection.
        connection: ConnectionId,
        /// The lane.
        lane: LaneId,
    },
    /// Server → Files: an Agent in this Workspace settled.
    AgentSettled {
        /// The Workspace.
        workspace: WorkspaceId,
    },
    /// Server → Files: end; refuse new work and end children first.
    Drain(DrainReason),
    /// Files → Server: whether the process holds work no open call covers, such as a transfer
    /// or a prompt.
    Activity {
        /// True while such work exists.
        holds: bool,
    },
}

impl LinkMessage {
    /// The end that sends this kind.
    #[must_use]
    pub const fn sender(&self) -> LinkSide {
        match self {
            Self::Hello(_) | Self::Frame(_) | Self::Close { .. } | Self::Activity { .. } => {
                LinkSide::Files
            }
            Self::Admitted
            | Self::Refused(_)
            | Self::Request(_)
            | Self::Cancel { .. }
            | Self::Scope(_)
            | Self::ConnectionLost { .. }
            | Self::LaneClosed { .. }
            | Self::AgentSettled { .. }
            | Self::Drain(_) => LinkSide::Server,
        }
    }
}

/// A frame this codec cannot write or does not admit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkError {
    /// The magic is not `OBFL`.
    InvalidMagic,
    /// The version is not [`LINK_VERSION`].
    UnsupportedVersion,
    /// The kind is unknown, or the other end sends it.
    UnexpectedKind,
    /// The length is outside the kind's bounds.
    InvalidLength,
    /// The body does not decode as the kind's body.
    InvalidBody,
    /// A product version or environment name outside its grammar.
    InvalidBuild,
}

impl fmt::Display for LinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMagic => "invalid OBFL magic",
            Self::UnsupportedVersion => "unsupported OBFL version",
            Self::UnexpectedKind => "unexpected OBFL kind",
            Self::InvalidLength => "invalid OBFL length",
            Self::InvalidBody => "invalid OBFL body",
            Self::InvalidBuild => "invalid build identity",
        })
    }
}

impl std::error::Error for LinkError {}

impl From<LinkError> for io::Error {
    fn from(error: LinkError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}

/// The complete frame of `message`.
///
/// # Errors
///
/// [`LinkError::InvalidLength`] for a record that is empty or does not fit one frame, or a root
/// that is empty or longer than [`MAXIMUM_ROOT_BYTES`].
pub fn encode_link_message(message: &LinkMessage) -> Result<Vec<u8>, LinkError> {
    let mut body = Vec::new();
    let kind = match message {
        LinkMessage::Hello(hello) => {
            body.extend_from_slice(hello.token.as_bytes());
            body.extend_from_slice(hello.build.fingerprint());
            match hello.build.commit() {
                Some(commit) => {
                    body.push(1);
                    body.extend_from_slice(commit);
                }
                None => body.push(0),
            }
            for text in [hello.build.version(), hello.build.environment()] {
                body.push(u8::try_from(text.len()).map_err(|_| LinkError::InvalidLength)?);
                body.extend_from_slice(text.as_bytes());
            }
            HELLO
        }
        LinkMessage::Admitted => ADMITTED,
        LinkMessage::Refused(LinkRefusal::BuildMismatch) => {
            body.push(0);
            REFUSED
        }
        LinkMessage::Request(request) => {
            nonempty_record(&request.record)?;
            body.extend_from_slice(&request.call.get().to_be_bytes());
            body.extend_from_slice(&request.connection.get().to_be_bytes());
            body.extend_from_slice(&request.lane.get().to_be_bytes());
            // A Put names no Workspace; no Workspace has the all-zero identity.
            body.extend_from_slice(
                request
                    .workspace
                    .map_or([0; WORKSPACE_BYTES], |workspace| *workspace.as_bytes())
                    .as_slice(),
            );
            body.push(match request.grant {
                GrantClass::TerminalCreate => 0,
                GrantClass::Lifecycle => 1,
            });
            body.extend_from_slice(&request.record);
            REQUEST
        }
        LinkMessage::Frame(frame) => {
            nonempty_record(&frame.record)?;
            body.extend_from_slice(&frame.call.get().to_be_bytes());
            body.push(match frame.kind {
                FrameKind::Result => 0,
                FrameKind::Snapshot => 1,
                FrameKind::Update => 2,
            });
            body.extend_from_slice(&frame.record);
            FRAME
        }
        LinkMessage::Close { call, reason } => {
            body.extend_from_slice(&call.get().to_be_bytes());
            body.push(match reason {
                CallClose::Complete => 0,
                CallClose::Cancelled => 1,
                CallClose::Unsupported => 2,
                CallClose::InvalidRequest => 3,
                CallClose::Capacity => 4,
                CallClose::Stopping => 5,
                CallClose::OutcomeUnknown => 6,
            });
            CLOSE
        }
        LinkMessage::Cancel { call } => {
            body.extend_from_slice(&call.get().to_be_bytes());
            CANCEL
        }
        LinkMessage::Scope(update) => {
            body.extend_from_slice(update.workspace.as_bytes());
            match &update.scope {
                WorkspaceScope::NoRoot => body.push(0),
                WorkspaceScope::Root(root) => {
                    if root.is_empty() || root.len() > MAXIMUM_ROOT_BYTES {
                        return Err(LinkError::InvalidLength);
                    }
                    body.push(1);
                    body.extend_from_slice(root.as_bytes());
                }
                WorkspaceScope::Closed => body.push(2),
            }
            SCOPE
        }
        LinkMessage::ConnectionLost { connection } => {
            body.extend_from_slice(&connection.get().to_be_bytes());
            CONNECTION_LOST
        }
        LinkMessage::LaneClosed { connection, lane } => {
            body.extend_from_slice(&connection.get().to_be_bytes());
            body.extend_from_slice(&lane.get().to_be_bytes());
            LANE_CLOSED
        }
        LinkMessage::AgentSettled { workspace } => {
            body.extend_from_slice(workspace.as_bytes());
            AGENT_SETTLED
        }
        LinkMessage::Drain(reason) => {
            body.push(match reason {
                DrainReason::Idle => 0,
                DrainReason::Session => 1,
            });
            DRAIN
        }
        LinkMessage::Activity { holds } => {
            body.push(u8::from(*holds));
            ACTIVITY
        }
    };
    if body.len() > MAXIMUM_LINK_PAYLOAD_BYTES {
        return Err(LinkError::InvalidLength);
    }
    let mut frame = Vec::with_capacity(HEADER_BYTES + body.len());
    frame.extend_from_slice(&LINK_MAGIC);
    frame.extend_from_slice(&LINK_VERSION.to_be_bytes());
    frame.extend_from_slice(&kind.to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(body.len())
            .map_err(|_| LinkError::InvalidLength)?
            .to_be_bytes(),
    );
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Writes `message` as one frame and flushes it.
///
/// # Errors
///
/// `InvalidData` for a message [`encode_link_message`] refuses; write errors pass through.
pub fn write_link_message(writer: &mut impl Write, message: &LinkMessage) -> io::Result<()> {
    writer.write_all(&encode_link_message(message)?)?;
    writer.flush()
}

/// Reads one frame `sender` sent.
///
/// # Errors
///
/// `UnexpectedEof` when the stream ends before a frame starts or inside one; `InvalidData`
/// carrying a [`LinkError`] for a frame this codec does not admit from `sender`; other read
/// errors pass through.
pub fn read_link_message(reader: &mut impl Read, sender: LinkSide) -> io::Result<LinkMessage> {
    let mut header = [0; HEADER_BYTES];
    reader.read_exact(&mut header)?;
    let (kind, length) = admit_header(&header, sender)?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(decode_body(kind, &body)?)
}

/// Decodes one complete frame `sender` sent.
///
/// # Errors
///
/// A [`LinkError`] for a frame this codec does not admit from `sender`, trailing bytes included.
pub fn decode_link_message(frame: &[u8], sender: LinkSide) -> Result<LinkMessage, LinkError> {
    let header = frame.get(..HEADER_BYTES).ok_or(LinkError::InvalidLength)?;
    let (kind, length) = admit_header(header, sender)?;
    let body = &frame[HEADER_BYTES..];
    if body.len() != length {
        return Err(LinkError::InvalidLength);
    }
    decode_body(kind, body)
}

fn admit_header(header: &[u8], sender: LinkSide) -> Result<(u16, usize), LinkError> {
    if header[..4] != LINK_MAGIC {
        return Err(LinkError::InvalidMagic);
    }
    if u16::from_be_bytes([header[4], header[5]]) != LINK_VERSION {
        return Err(LinkError::UnsupportedVersion);
    }
    let kind = u16::from_be_bytes([header[6], header[7]]);
    let length = usize::try_from(u32::from_be_bytes([
        header[8], header[9], header[10], header[11],
    ]))
    .map_err(|_| LinkError::InvalidLength)?;
    let (expected, bounds) = match kind {
        HELLO => (
            LinkSide::Files,
            TOKEN_BYTES + FINGERPRINT_BYTES + 1 + 2 + 2
                ..=TOKEN_BYTES
                    + FINGERPRINT_BYTES
                    + 1
                    + COMMIT_BYTES
                    + 1
                    + MAXIMUM_VERSION_BYTES
                    + 1
                    + MAXIMUM_ENVIRONMENT_BYTES,
        ),
        ADMITTED => (LinkSide::Server, 0..=0),
        REFUSED | DRAIN => (LinkSide::Server, 1..=1),
        REQUEST => (
            LinkSide::Server,
            8 + 8 + 8 + WORKSPACE_BYTES + 1 + 1..=MAXIMUM_LINK_PAYLOAD_BYTES,
        ),
        FRAME => (LinkSide::Files, 8 + 1 + 1..=MAXIMUM_LINK_PAYLOAD_BYTES),
        CLOSE => (LinkSide::Files, 9..=9),
        CANCEL | CONNECTION_LOST => (LinkSide::Server, 8..=8),
        SCOPE => (
            LinkSide::Server,
            WORKSPACE_BYTES + 1..=WORKSPACE_BYTES + 1 + MAXIMUM_ROOT_BYTES,
        ),
        LANE_CLOSED => (LinkSide::Server, 16..=16),
        AGENT_SETTLED => (LinkSide::Server, WORKSPACE_BYTES..=WORKSPACE_BYTES),
        ACTIVITY => (LinkSide::Files, 1..=1),
        _ => return Err(LinkError::UnexpectedKind),
    };
    if expected != sender {
        return Err(LinkError::UnexpectedKind);
    }
    if !bounds.contains(&length) {
        return Err(LinkError::InvalidLength);
    }
    Ok((kind, length))
}

fn decode_body(kind: u16, body: &[u8]) -> Result<LinkMessage, LinkError> {
    let mut reader = Body(body);
    let message = match kind {
        HELLO => {
            let token = LinkToken::from_bytes(reader.array()?);
            let fingerprint = reader.array()?;
            let commit = match reader.byte()? {
                0 => None,
                1 => Some(reader.array()?),
                _ => return Err(LinkError::InvalidBody),
            };
            let version = reader.short_text()?;
            let environment = reader.short_text()?;
            let build = BuildIdentity::new(version, commit, environment, fingerprint)
                .map_err(|_| LinkError::InvalidBody)?;
            LinkMessage::Hello(LinkHello { build, token })
        }
        ADMITTED => LinkMessage::Admitted,
        REFUSED => match reader.byte()? {
            0 => LinkMessage::Refused(LinkRefusal::BuildMismatch),
            _ => return Err(LinkError::InvalidBody),
        },
        REQUEST => {
            let call = CallId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?;
            let connection = ConnectionId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?;
            let lane = LaneId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?;
            let workspace = WorkspaceId::from_bytes(reader.array()?);
            let grant = match reader.byte()? {
                0 => GrantClass::TerminalCreate,
                1 => GrantClass::Lifecycle,
                _ => return Err(LinkError::InvalidBody),
            };
            LinkMessage::Request(LinkRequest {
                call,
                connection,
                lane,
                workspace,
                grant,
                record: reader.rest().to_vec(),
            })
        }
        FRAME => {
            let call = CallId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?;
            let kind = match reader.byte()? {
                0 => FrameKind::Result,
                1 => FrameKind::Snapshot,
                2 => FrameKind::Update,
                _ => return Err(LinkError::InvalidBody),
            };
            LinkMessage::Frame(LinkFrame {
                call,
                kind,
                record: reader.rest().to_vec(),
            })
        }
        CLOSE => {
            let call = CallId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?;
            let reason = match reader.byte()? {
                0 => CallClose::Complete,
                1 => CallClose::Cancelled,
                2 => CallClose::Unsupported,
                3 => CallClose::InvalidRequest,
                4 => CallClose::Capacity,
                5 => CallClose::Stopping,
                6 => CallClose::OutcomeUnknown,
                _ => return Err(LinkError::InvalidBody),
            };
            LinkMessage::Close { call, reason }
        }
        CANCEL => LinkMessage::Cancel {
            call: CallId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?,
        },
        SCOPE => {
            let workspace =
                WorkspaceId::from_bytes(reader.array()?).ok_or(LinkError::InvalidBody)?;
            let scope = match reader.byte()? {
                0 => WorkspaceScope::NoRoot,
                1 => {
                    let root = reader.rest();
                    if root.is_empty() {
                        return Err(LinkError::InvalidBody);
                    }
                    WorkspaceScope::Root(
                        std::str::from_utf8(root)
                            .map_err(|_| LinkError::InvalidBody)?
                            .to_owned(),
                    )
                }
                2 => WorkspaceScope::Closed,
                _ => return Err(LinkError::InvalidBody),
            };
            LinkMessage::Scope(ScopeUpdate { workspace, scope })
        }
        CONNECTION_LOST => LinkMessage::ConnectionLost {
            connection: ConnectionId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?,
        },
        LANE_CLOSED => LinkMessage::LaneClosed {
            connection: ConnectionId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?,
            lane: LaneId::new(reader.u64()?).ok_or(LinkError::InvalidBody)?,
        },
        AGENT_SETTLED => LinkMessage::AgentSettled {
            workspace: WorkspaceId::from_bytes(reader.array()?).ok_or(LinkError::InvalidBody)?,
        },
        DRAIN => LinkMessage::Drain(match reader.byte()? {
            0 => DrainReason::Idle,
            1 => DrainReason::Session,
            _ => return Err(LinkError::InvalidBody),
        }),
        ACTIVITY => LinkMessage::Activity {
            holds: match reader.byte()? {
                0 => false,
                1 => true,
                _ => return Err(LinkError::InvalidBody),
            },
        },
        _ => return Err(LinkError::UnexpectedKind),
    };
    if reader.0.is_empty() {
        Ok(message)
    } else {
        Err(LinkError::InvalidBody)
    }
}

fn nonempty_record(record: &[u8]) -> Result<(), LinkError> {
    if record.is_empty() {
        Err(LinkError::InvalidLength)
    } else {
        Ok(())
    }
}

fn valid_version(version: &[u8]) -> bool {
    (1..=MAXIMUM_VERSION_BYTES).contains(&version.len()) && version.iter().all(u8::is_ascii_graphic)
}

fn valid_environment(name: &[u8]) -> bool {
    (1..=MAXIMUM_ENVIRONMENT_BYTES).contains(&name.len())
        && name
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

/// The unread rest of one frame's body.
struct Body<'a>(&'a [u8]);

impl<'a> Body<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], LinkError> {
        if self.0.len() < length {
            return Err(LinkError::InvalidBody);
        }
        let (taken, rest) = self.0.split_at(length);
        self.0 = rest;
        Ok(taken)
    }

    fn array<const LENGTH: usize>(&mut self) -> Result<[u8; LENGTH], LinkError> {
        self.take(LENGTH)?
            .try_into()
            .map_err(|_| LinkError::InvalidBody)
    }

    fn byte(&mut self) -> Result<u8, LinkError> {
        Ok(self.array::<1>()?[0])
    }

    fn u64(&mut self) -> Result<u64, LinkError> {
        self.array().map(u64::from_be_bytes)
    }

    /// One length byte and that many UTF-8 bytes.
    fn short_text(&mut self) -> Result<&'a str, LinkError> {
        let length = usize::from(self.byte()?);
        std::str::from_utf8(self.take(length)?).map_err(|_| LinkError::InvalidBody)
    }

    fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.0)
    }
}

#[cfg(test)]
mod tests;
