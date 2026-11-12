//! The sequence fixtures: one state machine fed the peer's frames and driven through its API for
//! its own, with the verdict of every received frame and every frame it writes compared. The step
//! format and its rules are in the contract's "Fixtures and conformance" section.

use std::error::Error;

use orchbee_machine_protocol::{
    Connection, Delivery, Frame, FrameKind, Message, Open, Protocol, Record, Role,
    hub::Hub,
    link::{Event, Link},
};
use serde::Deserialize;

use crate::support::{self, TestResult};

const SEQUENCES: &str = include_str!("../../../schemas/machine/v1/fixtures/sequences.json");
const BYE_VIOLATION: &[u8] = br#"{"type":"bye","reason":"violation"}"#;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cases {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    connection: String,
    machine: String,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    sender: String,
    #[serde(flatten)]
    frame: FrameSpec,
    verdict: Option<String>,
    violation: Option<String>,
    replies: Option<Vec<FrameSpec>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrameSpec {
    kind: String,
    #[serde(default)]
    channel: u32,
    payload: Option<String>,
    payload_hex: Option<String>,
    fill: Option<Fill>,
    repeat: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    byte: u8,
    length: usize,
}

impl FrameSpec {
    fn frame(&self) -> Result<Frame, Box<dyn Error>> {
        let kind = FrameKind::from_name(&self.kind)
            .ok_or_else(|| format!("unknown frame kind {}", self.kind))?;
        let payload = match (&self.payload, &self.payload_hex, &self.fill) {
            (Some(text), None, None) => text.as_bytes().to_vec(),
            (None, Some(hex), None) => support::from_hex(hex)?,
            (None, None, Some(fill)) => vec![fill.byte; fill.length],
            (None, None, None) => Vec::new(),
            _ => return Err("a frame holds one of payload, payloadHex, and fill".into()),
        };
        Ok(Frame::new(kind, self.channel, payload)?)
    }

    fn times(&self) -> usize {
        self.repeat.unwrap_or(1)
    }
}

/// How the harness starts a machine of one connection.
trait Machine: Protocol + Sized {
    /// The machine for `role`, and whether its constructor already queued the first step's frame.
    fn start(role: Role, steps: &[Step]) -> Result<(Connection<Self>, bool), Box<dyn Error>>;
}

impl Machine for Hub {
    fn start(role: Role, _steps: &[Step]) -> Result<(Connection<Self>, bool), Box<dyn Error>> {
        Ok(match role {
            Role::Initiator => (Connection::initiator(), false),
            Role::Acceptor => (Connection::<Hub>::acceptor(), false),
        })
    }
}

impl Machine for Link {
    fn start(role: Role, steps: &[Step]) -> Result<(Connection<Self>, bool), Box<dyn Error>> {
        if role == Role::Initiator {
            return Ok((Connection::initiator(), false));
        }
        let first = steps
            .first()
            .filter(|step| step.sender == Role::Acceptor.name())
            .ok_or("a link acceptor's first step is its facts")?;
        match Record::<Self>::decode(Role::Acceptor, &first.frame.frame()?)? {
            Record::Message(Message::Event {
                event: Event::Facts(facts),
                ..
            }) => Ok((Connection::<Link>::acceptor(facts), true)),
            other => Err(format!("a link acceptor starts with facts, not {other:?}").into()),
        }
    }
}

fn drive<P: Protocol>(connection: &mut Connection<P>, frame: &Frame) -> TestResult {
    let channel = frame.channel();
    match Record::<P>::decode(connection.role(), frame)? {
        Record::Open(Open::Known { channel: open, .. }) => {
            assert_eq!(connection.open(open)?, channel, "the opened id");
        }
        Record::Accept { .. } => connection.accept(channel)?,
        Record::Data(bytes) => assert_eq!(connection.write(channel, &bytes)?, bytes.len()),
        Record::End => connection.end(channel)?,
        Record::Close(close) => connection.close(channel, close.outcome)?,
        Record::Reset { reason, .. } => connection.reset(channel, reason)?,
        Record::Ping(bytes) => connection.ping(bytes)?,
        Record::Message(Message::Request { id, request, .. }) => {
            assert_eq!(connection.request(request)?, id, "the request id");
        }
        Record::Message(Message::Reply { id, reply, .. }) => connection.reply(id, reply)?,
        Record::Message(Message::Event { event, .. }) => connection.event(event)?,
        Record::Message(Message::Bye { reason, .. }) => connection.bye(reason)?,
        other => return Err(format!("{other:?} is not sent through the API").into()),
    }
    Ok(())
}

fn written<P: Protocol>(connection: &mut Connection<P>) -> Vec<Frame> {
    std::iter::from_fn(|| connection.poll_transmit()).collect()
}

fn expect_written(name: &str, at: usize, actual: &[Frame], expected: &[Frame]) -> TestResult {
    let matches = actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| support::same_frame(actual, expected));
    if matches {
        Ok(())
    } else {
        Err(format!("{name} step {at}: wrote {actual:?}, expected {expected:?}").into())
    }
}

fn expand(replies: &[FrameSpec]) -> Result<Vec<Frame>, Box<dyn Error>> {
    let mut frames = Vec::new();
    for reply in replies {
        let frame = reply.frame()?;
        frames.extend(std::iter::repeat_n(frame, reply.times()));
    }
    Ok(frames)
}

fn run<P: Machine>(case: &Case) -> TestResult {
    let name = &case.name;
    let role = Role::from_name(&case.machine).ok_or("machine is initiator or acceptor")?;
    let (mut connection, mut queued) = P::start(role, &case.steps)?;
    for (at, step) in case.steps.iter().enumerate() {
        let frame = step
            .frame
            .frame()
            .map_err(|error| format!("{name} step {at}: {error}"))?;
        let replies = step
            .replies
            .as_deref()
            .map(expand)
            .transpose()
            .map_err(|error| format!("{name} step {at} replies: {error}"))?;
        if step.sender == role.name() {
            assert_eq!(
                step.verdict, None,
                "{name} step {at}: own steps carry no verdict"
            );
            let mut expected = Vec::new();
            for _ in 0..step.frame.times() {
                if !std::mem::take(&mut queued) {
                    drive(&mut connection, &frame)
                        .map_err(|error| format!("{name} step {at}: {error}"))?;
                }
                expected.push(frame.clone());
            }
            expected.extend(replies.unwrap_or_default());
            expect_written(name, at, &written(&mut connection), &expected)?;
            continue;
        }
        let verdict = step
            .verdict
            .as_deref()
            .ok_or_else(|| format!("{name} step {at}: a received step names its verdict"))?;
        for _ in 0..step.frame.times() {
            match connection.receive(&frame) {
                Err(violation) => {
                    assert_eq!(verdict, "violation", "{name} step {at}: {violation}");
                    assert_eq!(
                        Some(violation.as_str()),
                        step.violation.as_deref(),
                        "{name} step {at}"
                    );
                    assert_eq!(
                        at + 1,
                        case.steps.len(),
                        "{name}: a violation ends the case"
                    );
                    let bye = Frame::new(FrameKind::Message, 0, BYE_VIOLATION.to_vec())?;
                    return expect_written(name, at, &written(&mut connection), &[bye]);
                }
                Ok(receipt) => {
                    assert_eq!(receipt.verdict.as_str(), verdict, "{name} step {at}");
                    if let Some(Delivery::Data { channel, bytes }) = receipt.delivery {
                        connection.take(channel, bytes.len())?;
                    }
                }
            }
        }
        if let Some(replies) = replies {
            expect_written(name, at, &written(&mut connection), &replies)?;
        }
    }
    expect_written(name, case.steps.len(), &written(&mut connection), &[])
}

#[test]
fn every_sequence_gets_its_verdicts_and_writes_its_replies() -> TestResult {
    let fixtures: Cases = serde_json::from_str(SEQUENCES)?;
    assert!(!fixtures.cases.is_empty());
    for case in &fixtures.cases {
        match case.connection.as_str() {
            "machine-hub" => run::<Hub>(case)?,
            "machine-link" => run::<Link>(case)?,
            other => return Err(format!("{}: unknown connection {other}", case.name).into()),
        }
    }
    Ok(())
}
