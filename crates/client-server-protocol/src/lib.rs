mod codec;
mod error;
mod geometry;
mod identity;
mod model;
mod wire;
mod working_directory;

pub const PROTOCOL_VERSION: u8 = 1;

pub use codec::{
    FRAME_HEADER_BYTES, FRAME_MAGIC, FRAME_VERSION, MAXIMUM_FRAME_PAYLOAD_BYTES,
    decode_client_frame, decode_server_frame, encode_client_frame, encode_server_frame,
    read_client_message, read_server_message, write_client_message, write_server_message,
};
pub use error::{ProtocolError, ReadMessageError, WriteMessageError};
pub use geometry::{
    Geometry, GeometryError, GeometryField, MAXIMUM_GEOMETRY_VALUE, MAXIMUM_PIXEL_EXTENT,
    MAXIMUM_TERMINAL_CELLS,
};
pub use identity::{IdentityError, RequestId, WorkerId};
pub use model::{
    ClientMessage, CreateAcceptedState, CreateFailureReason, CreateOutcome, CreateRejectedReason,
    CreateUnavailableReason, ServerMessage, StartFailureReason, StartMode, StartOutcome,
    StartRejectedReason, StartRequestError, StartTransition, StartUnavailableReason,
    TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest, TerminalStartResult,
    WorkerConstraint,
};
pub use working_directory::{
    MAXIMUM_WORKING_DIRECTORY_BYTES, WorkingDirectory, WorkingDirectoryError,
};
