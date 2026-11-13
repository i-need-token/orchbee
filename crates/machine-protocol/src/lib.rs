//! The codec and channel state machine of the two machine connections, `machine-link` from the
//! remote-machine hub to the remote helper and `machine-hub` from a client to the hub, after their
//! `OBHS` answer selects generation 1. The contract in `schemas/machine/README.md` is the
//! authority; this crate binds it: `OBMX` frames, the JSON payload rules, each connection's
//! records, the answer delivery record `OBMA`, and one end's channels, credit, request ledger, and
//! fair sender. It performs no I/O, starts no thread, and keeps no clock, so carriers, timers,
//! liveness, policy, and relaying stay with the endpoints that own them.

mod answer;
mod channel;
mod frame;
mod payload;
mod protocol;
mod record;
mod value;

pub use answer::{
    ANSWER_GENERATION, ANSWER_HEADER_BYTES, ANSWER_MAGIC, ANSWER_TAKEN, AnswerError,
    MAXIMUM_ANSWER_BYTES, admissible_answer, decode_answer_header, encode_answer, wipe,
};
pub use channel::{
    CREDIT_REGRANT_THRESHOLD, Connection, Delivery, INITIAL_CREDIT, MAXIMUM_OPEN_CHANNELS,
    MAXIMUM_UNANSWERED_REQUESTS, Receipt, SendError, Verdict, Violation,
};
pub use frame::{
    FRAME_HEADER_BYTES, FRAME_MAGIC, Frame, FrameDecoder, FrameError, FrameKind,
    MAXIMUM_DATA_PAYLOAD_BYTES, MAXIMUM_JSON_PAYLOAD_BYTES, decode_frame,
};
pub use orchbee_connection_handshake::Extensions;
pub use protocol::{MAXIMUM_ROUTE_REPLY_BYTES, MAXIMUM_ROUTE_REQUEST_BYTES, Protocol};
pub use record::{
    ByeReason, Close, EncodeError, MAXIMUM_CREDIT, Message, Open, Record, RecordError, Role,
};
pub use value::{
    AnswerToken, Count, HostKey, Id, InvalidValue, KeyFingerprint, MAXIMUM_COUNT, MachineId, Name,
    PathText, Prompt, ResetReason, SessionName, Target, Time, Token,
};

/// The records of `machine-link` generation 1.
pub mod link {
    pub use crate::record::link::{Channel, ChannelKind, Event, Facts, Link, Outcome};
}

/// The records of `machine-hub` generation 1.
pub mod hub {
    pub use crate::record::hub::{
        AnswerEndpoint, Attach, AttachOutcome, Catalog, Channel, ChannelKind, ClaimOutcome,
        Claimant, Connect, ConnectFailure, ConnectOutcome, ConnectResult, ConnectWaiting,
        DisconnectOutcome, DismissOutcome, Draft, Event, Hub, HubState, LoginEnd, LoginEvent,
        LoginFailedCause, LoginPhase, MAXIMUM_CATALOG_MACHINES, MachineEvent, MachineState,
        MachineStateName, NeedsLoginCause, NeedsUpdateCause, OfflineCause, Origin, Outcome,
        Question, QuestionKind, ReconnectCause, ReloadOutcome, Remote, Reply, Request, Stage,
        StopSessionOutcome, Trigger, UnsupportedCause, WaitReason,
    };
}
