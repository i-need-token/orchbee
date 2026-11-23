//! The Worker's connection to the Session's Keeper (`schemas/keeper/README.md`): the `keeper`
//! handshake, the hello as the Worker, and the Worker's requests, each answered within the
//! Keeper's reply bound or failed. A frame is `OBKP`, a big-endian payload length of 1 to
//! 1,048,576, and one JSON object. A frame without an `id` is an event, which generation 1 does not
//! define and this client skips. Nothing here ends a Keeper or a program it holds.

use std::{
    io::{self, Read, Write},
    path::Path,
    time::{Duration, Instant},
};

use orchbee_connection_handshake::{
    InitiatorMessage, Offer, RECOVERY, ReadError, conclude_offer, encode_initiator_frame,
    read_acceptor_message,
};
use orchbee_local_ipc::LocalStream;
use serde_json::{Map, Value};

/// How long the Keeper may take to answer one request.
pub const KEEPER_REPLY_BOUND: Duration = Duration::from_secs(2);
const MAGIC: [u8; 4] = *b"OBKP";
const MAXIMUM_PAYLOAD: usize = 1_048_576;

/// A process as the Keeper names it: its ID and its creation time, a FILETIME on Windows and the
/// start time in microseconds since 1970 on macOS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeeperProcess {
    pub pid: u32,
    pub created: u64,
}

/// The terminal a holding last ran as, under the Host epoch that served it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeldTerminal {
    pub creation_id: [u8; 16],
    pub host_epoch: [u8; 16],
    pub terminal_id: [u8; 16],
}

/// A holding's state as the Keeper names it; a name this build does not know reads as unknown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HoldingState {
    Held,
    Quiesced,
    Orphaned,
    Unknown,
}

/// One terminal holding the Keeper reported, with the program and OpenConsole processes it holds;
/// a macOS PTY holding names its leader as the program and has no console.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Holding {
    pub holding: [u8; 16],
    pub owner: KeeperProcess,
    pub state: HoldingState,
    pub terminal: HeldTerminal,
    pub program: KeeperProcess,
    pub console: Option<KeeperProcess>,
}

/// One background program a Host holds in the Keeper, under `programs`: the program's process and
/// the id its Worker gave it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramHolding {
    pub holding: [u8; 16],
    pub owner: KeeperProcess,
    pub state: HoldingState,
    pub process: KeeperProcess,
    pub program_id: [u8; 16],
}

/// What `status` reported: every terminal holding, every program holding, and every registered
/// Host with whether it ended.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct KeeperStatus {
    pub holdings: Vec<Holding>,
    pub programs: Vec<ProgramHolding>,
    pub hosts: Vec<(KeeperProcess, bool)>,
}

/// A request the Keeper answered without applying it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeeperRefusal {
    /// `refused`, with the reason the Keeper named.
    Refused(String),
    /// `unsupported`: this Keeper does not know the request.
    Unsupported,
}

/// One admitted Worker connection to the Keeper.
pub struct KeeperClient {
    stream: LocalStream,
    next_id: u32,
}

impl KeeperClient {
    /// Dials the Keeper at `endpoint` before `deadline`, sends `offer`, a `keeper` offer, and says
    /// hello as the Worker. Also answers whether the connection negotiated `recovery`.
    ///
    /// # Errors
    ///
    /// Returns an error when the Keeper cannot be reached by `deadline`, refuses the offer or the
    /// hello, or answers outside the contract.
    pub fn connect(endpoint: &Path, offer: &Offer, deadline: Instant) -> io::Result<(Self, bool)> {
        #[cfg(windows)]
        let mut stream = LocalStream::connect_before(endpoint, deadline)?;
        // A Unix socket's connect completes or fails at once.
        #[cfg(unix)]
        let mut stream = LocalStream::connect(endpoint)?;
        let remaining = remaining(deadline)?;
        stream.set_read_timeout(Some(remaining))?;
        stream.set_write_timeout(Some(remaining))?;
        let frame = encode_initiator_frame(&InitiatorMessage::Offer(offer.clone()))
            .map_err(|_| invalid("this build's Keeper offer did not encode"))?;
        stream.write_all(&frame)?;
        let reply = read_acceptor_message(&mut stream).map_err(|error| match error {
            ReadError::Io(error) => error,
            ReadError::Closed => io::ErrorKind::UnexpectedEof.into(),
            _ => invalid("the Keeper sent no valid handshake reply"),
        })?;
        let answer =
            conclude_offer(offer, reply).map_err(|_| invalid("the Keeper refused this Worker"))?;
        let recovery = answer.capabilities.contains(RECOVERY);
        let mut client = Self { stream, next_id: 1 };
        client
            .send("hello", vec![("role", Value::from("worker"))])?
            .map_err(refusal_error)?;
        Ok((client, recovery))
    }

    /// Registers the Host `host` with the Keeper, before that Host attaches it.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection fails or the Keeper answers outside the contract.
    pub fn register_host(&mut self, host: KeeperProcess) -> io::Result<Result<(), KeeperRefusal>> {
        self.send("register-host", vec![("process", process_value(host))])
            .map(|reply| reply.map(drop))
    }

    /// Removes the registration of `host`, which owns no holding any more.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection fails or the Keeper answers outside the contract.
    pub fn unregister_host(
        &mut self,
        host: KeeperProcess,
    ) -> io::Result<Result<(), KeeperRefusal>> {
        self.send("unregister-host", vec![("process", process_value(host))])
            .map(|reply| reply.map(drop))
    }

    /// Every holding and registered Host, read page by page.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection fails, the Keeper refuses, or a page breaks the
    /// contract.
    pub fn status(&mut self) -> io::Result<KeeperStatus> {
        let mut status = KeeperStatus::default();
        let mut after: Option<Value> = None;
        loop {
            let members = after
                .take()
                .map(|after| ("after", after))
                .into_iter()
                .collect();
            let reply = self.send("status", members)?.map_err(refusal_error)?;
            let holdings = reply
                .get("holdings")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("a status page has no holdings"))?;
            for entry in holdings {
                match entry.get("kind").map(Value::as_str) {
                    None => status.holdings.push(holding(entry, false)?),
                    Some(Some("pty")) => status.holdings.push(holding(entry, true)?),
                    Some(Some("program")) => status.programs.push(program_holding(entry)?),
                    Some(_) => return Err(invalid("a holding names an unknown kind")),
                }
            }
            for entry in reply
                .get("hosts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let ended = entry
                    .get("ended")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("a status host has no ended flag"))?;
                status.hosts.push((process(entry.get("process"))?, ended));
            }
            match reply.get("next") {
                Some(next) if next.is_string() => after = Some(next.clone()),
                Some(_) => return Err(invalid("a status page names an invalid next holding")),
                None => return Ok(status),
            }
        }
    }

    /// Assigns the orphaned holding `holding` to the registered Host `target` under `mark`, which
    /// that Host then claims. The same assignment again succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection fails or the Keeper answers outside the contract.
    pub fn assign(
        &mut self,
        holding: [u8; 16],
        target: KeeperProcess,
        mark: [u8; 16],
    ) -> io::Result<Result<(), KeeperRefusal>> {
        self.send(
            "assign",
            vec![
                ("holding", Value::from(hex(&holding))),
                ("for", process_value(target)),
                ("mark", Value::from(hex(&mark))),
            ],
        )
        .map(|reply| reply.map(drop))
    }

    /// Discards the orphaned holding `holding`, which ends its program.
    ///
    /// # Errors
    ///
    /// Returns an error when the connection fails or the Keeper answers outside the contract.
    pub fn discard(&mut self, holding: [u8; 16]) -> io::Result<Result<(), KeeperRefusal>> {
        self.send("discard", vec![("holding", Value::from(hex(&holding)))])
            .map(|reply| reply.map(drop))
    }

    /// Sends one request of `kind` with `members` and reads its reply within the reply bound,
    /// skipping events.
    fn send(
        &mut self,
        kind: &str,
        members: Vec<(&str, Value)>,
    ) -> io::Result<Result<Map<String, Value>, KeeperRefusal>> {
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or_else(|| invalid("this connection used every request id"))?;
        let mut body = Map::new();
        body.insert("type".to_owned(), Value::from(kind));
        body.insert("id".to_owned(), Value::from(id));
        for (name, value) in members {
            body.insert(name.to_owned(), value);
        }
        let payload = serde_json::to_vec(&body).map_err(|_| invalid("a request did not encode"))?;
        let length = u32::try_from(payload.len())
            .ok()
            .filter(|length| (1..=MAXIMUM_PAYLOAD as u32).contains(length))
            .ok_or_else(|| invalid("a request exceeds the frame bound"))?;
        let deadline = Instant::now() + KEEPER_REPLY_BOUND;
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&MAGIC);
        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(&payload);
        self.stream.set_write_timeout(Some(remaining(deadline)?))?;
        self.stream.write_all(&frame)?;
        loop {
            self.stream.set_read_timeout(Some(remaining(deadline)?))?;
            let reply = self.read_frame()?;
            let Some(reply_id) = reply.get("id") else {
                continue;
            };
            if reply_id.as_u64() != Some(u64::from(id)) {
                return Err(invalid("the Keeper answered another request"));
            }
            return match reply.get("type").and_then(Value::as_str) {
                Some("refused") => Ok(Err(KeeperRefusal::Refused(
                    reply
                        .get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_owned(),
                ))),
                Some("unsupported") => Ok(Err(KeeperRefusal::Unsupported)),
                Some(result) if result.strip_suffix("-result") == Some(kind) => Ok(Ok(reply)),
                _ => Err(invalid("the Keeper answered with another result")),
            };
        }
    }

    fn read_frame(&mut self) -> io::Result<Map<String, Value>> {
        let mut header = [0_u8; 8];
        self.stream.read_exact(&mut header)?;
        if header[..4] != MAGIC {
            return Err(invalid("the Keeper sent a frame of another protocol"));
        }
        let length = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        let length = usize::try_from(length)
            .ok()
            .filter(|length| (1..=MAXIMUM_PAYLOAD).contains(length))
            .ok_or_else(|| invalid("the Keeper sent a frame outside the bound"))?;
        let mut payload = vec![0_u8; length];
        self.stream.read_exact(&mut payload)?;
        match serde_json::from_slice(&payload) {
            Ok(Value::Object(members)) => Ok(members),
            _ => Err(invalid("the Keeper sent a frame that is not a JSON object")),
        }
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(remaining)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn refusal_error(refusal: KeeperRefusal) -> io::Error {
    match refusal {
        KeeperRefusal::Refused(reason) => io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("the Keeper refused the Worker: {reason}"),
        ),
        KeeperRefusal::Unsupported => invalid("the Keeper does not know a Worker request"),
    }
}

fn process_value(process: KeeperProcess) -> Value {
    let mut members = Map::new();
    members.insert("pid".to_owned(), Value::from(process.pid));
    members.insert(
        "created".to_owned(),
        Value::from(format!("{:016x}", process.created)),
    );
    Value::Object(members)
}

fn process(value: Option<&Value>) -> io::Result<KeeperProcess> {
    let value = value.ok_or_else(|| invalid("a process is missing"))?;
    let pid = value
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok())
        .filter(|pid| *pid != 0)
        .ok_or_else(|| invalid("a process has no valid ID"))?;
    let created = value
        .get("created")
        .and_then(Value::as_str)
        .filter(|text| text.len() == 16 && lower_hex(text))
        .and_then(|text| u64::from_str_radix(text, 16).ok())
        .filter(|created| *created != 0)
        .ok_or_else(|| invalid("a process has no valid creation time"))?;
    Ok(KeeperProcess { pid, created })
}

fn holding_state(entry: &Value) -> io::Result<HoldingState> {
    Ok(match entry.get("state").and_then(Value::as_str) {
        Some("held") => HoldingState::Held,
        Some("quiesced") => HoldingState::Quiesced,
        Some("orphaned") => HoldingState::Orphaned,
        Some(_) => HoldingState::Unknown,
        None => return Err(invalid("a holding has no state")),
    })
}

fn program_holding(entry: &Value) -> io::Result<ProgramHolding> {
    let facts = entry
        .get("facts")
        .ok_or_else(|| invalid("a holding has no facts"))?;
    Ok(ProgramHolding {
        holding: identity(entry.get("holding"))?,
        owner: process(entry.get("owner"))?,
        state: holding_state(entry)?,
        process: process(facts.get("process"))?,
        program_id: identity(facts.get("programId"))?,
    })
}

/// A terminal holding; a PTY holding's facts name its `leader` and no console.
fn holding(entry: &Value, pty: bool) -> io::Result<Holding> {
    let state = holding_state(entry)?;
    let facts = entry
        .get("facts")
        .ok_or_else(|| invalid("a holding has no facts"))?;
    let terminal = facts
        .get("terminal")
        .ok_or_else(|| invalid("a holding has no terminal"))?;
    Ok(Holding {
        holding: identity(entry.get("holding"))?,
        owner: process(entry.get("owner"))?,
        state,
        terminal: HeldTerminal {
            creation_id: identity(terminal.get("creationId"))?,
            host_epoch: identity(terminal.get("hostEpoch"))?,
            terminal_id: identity(terminal.get("terminalId"))?,
        },
        program: process(facts.get(if pty { "leader" } else { "program" }))?,
        console: if pty {
            None
        } else {
            Some(process(facts.get("console"))?)
        },
    })
}

/// Sixteen bytes as 32 lowercase hexadecimal digits.
fn identity(value: Option<&Value>) -> io::Result<[u8; 16]> {
    let text = value
        .and_then(Value::as_str)
        .filter(|text| text.len() == 32 && lower_hex(text))
        .ok_or_else(|| invalid("an identity is not 32 lowercase hexadecimal digits"))?;
    let mut bytes = [0_u8; 16];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| invalid("an identity is not text"))?;
        *byte =
            u8::from_str_radix(pair, 16).map_err(|_| invalid("an identity is not hexadecimal"))?;
    }
    Ok(bytes)
}

fn lower_hex(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Sixteen bytes as 32 lowercase hexadecimal digits.
fn hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
