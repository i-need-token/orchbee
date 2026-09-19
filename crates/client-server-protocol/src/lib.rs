mod codec;
mod control;
mod error;
mod geometry;
mod identity;
mod model;
mod number;
mod open;
mod paste_record;
mod send;
mod viewer;
mod viewer_record;
mod wire;
mod working_directory;

pub const PROTOCOL_VERSION: u8 = 1;

pub use codec::{
    FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION, MAXIMUM_FRAME_PAYLOAD_BYTES,
    decode_client_frame, decode_server_frame, encode_client_frame, encode_server_frame,
    read_client_message, read_server_message, write_client_message, write_server_message,
};
pub use control::{
    ControlInput, ControlRejectedReason, ControlUnavailableReason, MAXIMUM_CONTROL_INPUT_BYTES,
    MAXIMUM_PASTE_CONTENT_BYTES, PasteAcceptedBytes, PasteContent, TerminalControlAction,
    TerminalControlOutcome, TerminalControlRequest, TerminalControlResult,
};
pub use error::{ProtocolError, ReadMessageError, WriteMessageError};
pub use geometry::{
    Geometry, GeometryError, GeometryField, MAXIMUM_GEOMETRY_VALUE, MAXIMUM_PIXEL_EXTENT,
    MAXIMUM_TERMINAL_CELLS,
};
pub use identity::{
    ControllerId, HostEpoch, HostTerminalId, IdentityError, RequestId, SiteId, TerminalId,
    ViewerId, WorkerId,
};
pub use model::{
    ClientMessage, CreateAcceptedState, CreateFailureReason, CreateOutcome, CreateRejectedReason,
    CreateUnavailableReason, ServerMessage, StartFailureReason, StartMode, StartOutcome,
    StartRejectedReason, StartRequestError, StartTransition, StartUnavailableReason,
    TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest, TerminalStartResult,
    WorkerConstraint,
};
pub use paste_record::PASTE_RECORD_MAGIC;
pub use send::{
    MAXIMUM_SEND_TEXT_BYTES, SendAcceptedBytes, SendCleanupStatus, SendKey, SendRejectedReason,
    SendTarget, SendText, SendTextInput, SendUnavailableReason, TerminalSendOutcome,
    TerminalSendPayload, TerminalSendRequest, TerminalSendResult,
};
pub use viewer::{
    HistoryDemand, MAXIMUM_READ_ROWS, MAXIMUM_READ_ROWS_BYTES, MAXIMUM_VIEWER_RECORD_BYTES,
    MINIMUM_READ_ROWS_BYTES, ProjectionDemand, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, TerminalViewerResult, ViewerRejectedReason, ViewerRowsRejectedReason,
    ViewerSnapshotReason, ViewerUnavailableReason, ViewportRowOrigin,
};
pub use working_directory::{
    MAXIMUM_WORKING_DIRECTORY_BYTES, WorkingDirectory, WorkingDirectoryError,
};

pub use open::{OpenOutcome, TerminalOpenRequest, TerminalOpenResult};
