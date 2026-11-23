//! The Terminal Host connection as the program host of the Worker's plugin job owner, where the
//! Host negotiated `background-programs`: each program request as one correlated exchange, and the
//! `ProgramExited` events the connection received meanwhile.

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
                .map(ProgramReply::Answer)
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
