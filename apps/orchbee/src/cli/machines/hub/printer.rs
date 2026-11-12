//! `orchbee --machine-hub-state-v1`: attaches with `watch` alone, so it holds nothing, starting the
//! hub when none runs, and prints the hub's state, its process id, the catalog revision, and each
//! Machine's state as localized text.

use super::super::shared_carrier::frames;
use super::attach::{self, AttachFailure};
use super::lifetime::Moment;
use crate::{cli::Output, cli_text::CliText, localization::Locale};
use orchbee_local_ipc::LocalStream;
use orchbee_machine_protocol::{
    ByeReason, Connection, Delivery, FrameDecoder, MachineId, Message,
    hub::{self, Attach, AttachOutcome, Catalog, Event, HubState, MachineState, Reply, Request},
};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    time::{Duration, Instant},
};

/// The bound on reaching the hub, a start included.
const ATTACH: Duration = Duration::from_secs(10);
/// The bound on the attachment's reply and events, and on the `query`.
const READ: Duration = Duration::from_secs(5);

/// Why the printer printed no state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StateFailure {
    DataRoot,
    Unavailable,
    Denied,
    Closing,
    Lost,
    Incompatible,
    Timeout,
}

impl StateFailure {
    const fn status(self) -> u8 {
        match self {
            Self::DataRoot => 1,
            Self::Denied => 3,
            Self::Unavailable | Self::Closing => 4,
            Self::Lost => 5,
            Self::Incompatible => 6,
            Self::Timeout => 7,
        }
    }
}

impl From<AttachFailure> for StateFailure {
    fn from(failure: AttachFailure) -> Self {
        match failure {
            AttachFailure::DataRoot => Self::DataRoot,
            AttachFailure::Unavailable => Self::Unavailable,
            AttachFailure::Denied => Self::Denied,
            AttachFailure::Incompatible => Self::Incompatible,
            AttachFailure::Timeout => Self::Timeout,
        }
    }
}

/// What the attachment reported.
struct Report {
    hub: HubState,
    catalog: Catalog,
    machines: BTreeMap<String, MachineState>,
}

pub(super) fn run(locale: Locale) -> Output {
    let text = CliText::new(locale);
    match collect() {
        Ok((report, pid)) => Output {
            status: 0,
            stdout: render(text, &report, pid).into_bytes(),
            stderr: String::new(),
            finalization: None,
        },
        Err(failure) => Output {
            status: failure.status(),
            stdout: Vec::new(),
            stderr: format!("{}\n", text.cli_machine_hub_failure(failure)),
            finalization: None,
        },
    }
}

fn collect() -> Result<(Report, u32), StateFailure> {
    let stream = attach::attach_or_start(Instant::now() + ATTACH)?;
    let report = watch(&stream, Instant::now() + READ)?;
    let pid = attach::query(Instant::now() + READ)?
        .processes()
        .first()
        .map(|process| process.pid.get())
        .ok_or(StateFailure::Incompatible)?;
    Ok((report, pid))
}

/// Attaches with `watch` and reads the reply, `catalog`, and one `machine` per id.
fn watch(stream: &LocalStream, deadline: Instant) -> Result<Report, StateFailure> {
    let mut connection = Connection::<hub::Hub>::initiator();
    let attach = connection
        .request(Request::Attach(Attach {
            presence: false,
            asks: false,
            watch: true,
        }))
        .map_err(|_| StateFailure::Lost)?;
    send(&mut connection, stream)?;
    let mut decoder = FrameDecoder::new();
    let mut buffer = vec![0; 64 * 1024];
    let mut hub: Option<HubState> = None;
    let mut catalog: Option<Catalog> = None;
    let mut machines: BTreeMap<String, MachineState> = BTreeMap::new();
    let mut failure: Option<StateFailure> = None;
    loop {
        if let (Some(hub), Some(catalog)) = (hub, catalog.as_ref())
            && catalog
                .machines()
                .iter()
                .all(|id: &MachineId| machines.contains_key(id.as_str()))
        {
            let _farewell = connection.bye(ByeReason::Done);
            let _sent = send(&mut connection, stream);
            return Ok(Report {
                hub,
                catalog: catalog.clone(),
                machines,
            });
        }
        if let Some(failure) = failure {
            return Err(failure);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(StateFailure::Timeout);
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| StateFailure::Lost)?;
        let mut input = stream;
        let read = match input.read(&mut buffer) {
            Ok(0) => return Err(StateFailure::Lost),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                return Err(StateFailure::Timeout);
            }
            Err(_) => return Err(StateFailure::Lost),
        };
        let bytes = buffer.get(..read).unwrap_or_default();
        frames(&mut decoder, bytes, |frame| {
            let Ok(receipt) = connection.receive(frame) else {
                failure = Some(StateFailure::Lost);
                return false;
            };
            match receipt.delivery {
                Some(Delivery::Message(Message::Reply {
                    id,
                    reply: Reply::Attach(outcome),
                    ..
                })) if id == attach => match outcome {
                    AttachOutcome::Attached { hub: state } => hub = Some(state),
                    AttachOutcome::Stopping | AttachOutcome::Unknown => {
                        failure = Some(StateFailure::Closing);
                    }
                },
                Some(Delivery::Message(Message::Event { event, .. })) => match event {
                    Event::Hub(state) => hub = Some(state),
                    Event::Catalog(next) => catalog = Some(next),
                    Event::Machine(machine) => {
                        machines.insert(machine.machine.as_str().to_owned(), machine.state);
                    }
                    _ => {}
                },
                Some(Delivery::Message(Message::Bye { .. })) => {
                    failure = Some(StateFailure::Closing)
                }
                _ => {}
            }
            failure.is_none()
        })
        .map_err(|_| StateFailure::Lost)?;
        send(&mut connection, stream)?;
    }
}

/// Writes every frame the connection has queued.
fn send(connection: &mut Connection<hub::Hub>, stream: &LocalStream) -> Result<(), StateFailure> {
    let mut output = stream;
    while let Some(frame) = connection.poll_transmit() {
        output
            .write_all(&frame.encode())
            .map_err(|_| StateFailure::Lost)?;
    }
    Ok(())
}

fn render(text: CliText, report: &Report, pid: u32) -> String {
    let state = match report.hub {
        HubState::Held => text.cli_machine_hub_held().to_owned(),
        HubState::Expiring { expires_at } => {
            let seconds = expires_at
                .get()
                .saturating_sub(Moment::now().wall)
                .div_ceil(1_000);
            text.cli_machine_hub_expiring(seconds)
        }
        HubState::Closing => text.cli_machine_hub_closing().to_owned(),
    };
    let mut out = text.cli_machine_hub_summary(&state, pid, report.catalog.revision().get());
    if report.catalog.machines().is_empty() {
        out.push_str(text.cli_machine_message("empty"));
        out.push('\n');
    }
    for id in report.catalog.machines() {
        let state = report.machines.get(id.as_str()).map_or_else(
            || text.cli_machine_hub_machine_state(None),
            |state| text.cli_machine_hub_machine_state(Some(state)),
        );
        out.push_str(&format!("{id}  {state}\n"));
    }
    out
}
