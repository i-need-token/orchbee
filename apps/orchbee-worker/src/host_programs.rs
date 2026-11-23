//! The Terminal Host connection as the program host of the Worker's plugin job owner, where the
//! Host negotiated `background-programs`: each program request as one correlated exchange, and the
//! `ProgramExited` events the connection received meanwhile. Each answer notes on the connection
//! which programs that Host runs, and a Worker holding several Hosts starts a program on its own
//! Host and sends every other request to the Host that runs the program.

use std::time::Duration;

use orchbee_local_ipc::LocalStream as UnixStream;
use orchbee_terminal_protocol::{
    CorrelationId, PayloadEncoding, ProgramExited, ProgramRequest, decode_handover_refusal,
    decode_program_answer, decode_transport_envelope, encode_program_request,
};

use crate::host_connection::{HostBrokerConnection, exchange, exchange_between_requests};
use crate::plugin_jobs::{ProgramHost, ProgramReply};

/// How long one program request between Server requests may take, as a Keeper request may.
const BETWEEN_REQUESTS_BUDGET: Duration = Duration::from_secs(3);

pub(crate) struct HostPrograms<'a> {
    host: &'a HostBrokerConnection,
    /// The Server stream of the request being served; `None` between requests.
    server: Option<&'a UnixStream>,
}

impl<'a> HostPrograms<'a> {
    /// The programs of `host` while the Worker serves a request read from `server`.
    pub(crate) fn serving(host: &'a HostBrokerConnection, server: &'a UnixStream) -> Option<Self> {
        host.background_programs().then_some(Self {
            host,
            server: Some(server),
        })
    }

    /// The programs of `host` between Server requests.
    pub(crate) fn between_requests(host: &'a HostBrokerConnection) -> Option<Self> {
        host.background_programs()
            .then_some(Self { host, server: None })
    }
}

impl ProgramHost for HostPrograms<'_> {
    fn request(&self, request: &ProgramRequest) -> Result<ProgramReply, ()> {
        let encoded = encode_program_request(request).map_err(drop)?;
        let identity = crate::worker_identity::fresh_identity().map_err(drop)?;
        let correlation = CorrelationId::from_bytes(identity).map_err(drop)?;
        let record = match self.server {
            Some(server) => exchange(
                self.host,
                server,
                correlation,
                encoded.kind,
                &encoded.payload,
                PayloadEncoding::ControlJson,
            ),
            None => exchange_between_requests(
                self.host,
                correlation,
                encoded.kind,
                &encoded.payload,
                PayloadEncoding::ControlJson,
                BETWEEN_REQUESTS_BUDGET,
            ),
        }
        .map_err(drop)?;
        let envelope = decode_transport_envelope(&record).map_err(drop)?;
        if envelope.kind == encoded.kind {
            return decode_program_answer(envelope.kind, envelope.role, envelope.payload)
                .map(|answer| {
                    self.host.note_program_answer(&answer);
                    ProgramReply::Answer(answer)
                })
                .map_err(drop);
        }
        // The Host's refusal names a program reason, which the handover decoder reads.
        let refusal = decode_handover_refusal(envelope.kind, envelope.role, envelope.payload)
            .map_err(drop)?;
        Ok(ProgramReply::Refused {
            code: refusal.code,
            reason: refusal.reason,
        })
    }

    fn take_exits(&self) -> Vec<ProgramExited> {
        self.host.take_program_exits()
    }
}

/// The programs of every Host a Worker holds: a start goes to its own Host, and a request that names
/// a program to the Host that runs it. Without an own Host no program starts in a Host.
pub(crate) struct HeldPrograms<'a> {
    /// Each Host that runs programs, and whether it is the Worker's own.
    hosts: Vec<(HostPrograms<'a>, bool)>,
}

impl<'a> HeldPrograms<'a> {
    /// The programs of `hosts`, each with whether it is the Worker's own, while the Worker serves a
    /// request read from `server`.
    pub(crate) fn serving(
        hosts: impl IntoIterator<Item = (&'a HostBrokerConnection, bool)>,
        server: &'a UnixStream,
    ) -> Option<Self> {
        let hosts: Vec<_> = hosts
            .into_iter()
            .filter_map(|(host, own)| Some((HostPrograms::serving(host, server)?, own)))
            .collect();
        (!hosts.is_empty()).then_some(Self { hosts })
    }

    /// The programs of `hosts`, each with whether it is the Worker's own, between Server requests.
    #[cfg(any(windows, target_os = "macos"))]
    pub(crate) fn between_requests(
        hosts: impl IntoIterator<Item = (&'a HostBrokerConnection, bool)>,
    ) -> Option<Self> {
        let hosts: Vec<_> = hosts
            .into_iter()
            .filter_map(|(host, own)| Some((HostPrograms::between_requests(host)?, own)))
            .collect();
        (!hosts.is_empty()).then_some(Self { hosts })
    }

    fn own(&self) -> Option<&HostPrograms<'a>> {
        self.hosts
            .iter()
            .find_map(|(programs, own)| own.then_some(programs))
    }
}

impl ProgramHost for HeldPrograms<'_> {
    fn request(&self, request: &ProgramRequest) -> Result<ProgramReply, ()> {
        let program = match request {
            ProgramRequest::Start(_) | ProgramRequest::List { .. } => None,
            ProgramRequest::Cancel(program) | ProgramRequest::Dispose(program) => Some(*program),
            ProgramRequest::ReadOutput { program_id, .. }
            | ProgramRequest::Recover { program_id, .. } => Some(*program_id),
        };
        let host = program
            .and_then(|program| {
                self.hosts
                    .iter()
                    .find(|(programs, _)| programs.host.runs_program(program))
                    .map(|(programs, _)| programs)
            })
            .or_else(|| self.own())
            .ok_or(())?;
        host.request(request)
    }

    fn take_exits(&self) -> Vec<ProgramExited> {
        self.hosts
            .iter()
            .flat_map(|(programs, _)| programs.take_exits())
            .collect()
    }

    fn starts(&self) -> bool {
        self.own().is_some()
    }
}
