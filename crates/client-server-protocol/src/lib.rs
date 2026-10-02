mod codec;
mod control;
mod error;
mod geometry;
mod identity;
mod launch_read;
mod launch_selection;
mod model;
mod number;
mod open;
mod paste_record;
mod record_identities;
mod send;
mod send_record;
mod send_stroke;
mod viewer;
mod viewer_record;
mod wire;
mod working_directory;

pub const PROTOCOL_VERSION: u8 = 2;

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
    ControllerId, HostEpoch, HostTerminalId, IdentityError, PanelId, PresentationId, RequestId,
    SiteId, TabId, TerminalContextId, TerminalId, ViewerId, WorkerId,
};
pub use launch_read::{
    LaunchProfileProvenance, TerminalLaunchBoundTo, TerminalLaunchFact, TerminalLaunchReadOutcome,
    TerminalLaunchReadRequest, TerminalLaunchReadResult, TerminalLaunchReadUnavailableReason,
    TerminalLaunchRoute, TerminalLaunchState,
};
pub use launch_selection::{
    LaunchSelection, LaunchSelectionError, MAXIMUM_LAUNCH_SELECTION_ARGUMENTS,
    MAXIMUM_LAUNCH_SELECTION_BYTES,
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
    MAXIMUM_SEND_ACCEPTED_BYTES, MAXIMUM_SEND_ACCEPTED_ENCODED_BYTES, MAXIMUM_SEND_AGGREGATE_BYTES,
    MAXIMUM_SEND_BYTES_BYTES, MAXIMUM_SEND_PASTE_BYTES, MAXIMUM_SEND_RECORDS,
    MAXIMUM_SEND_TEXT_BYTES, SendAcceptedBytes, SendBytes, SendCleanupStatus, SendComboBase,
    SendKey, SendModifiers, SendPasteContent, SendProgress, SendRecord, SendRecords,
    SendRejectedReason, SendTarget, SendText, SendTextInput, SendUnavailableReason,
    TerminalSendOutcome, TerminalSendRequest, TerminalSendResult,
};
pub use send_record::SEND_RECORD_MAGIC;
pub use send_stroke::{SendKeyStroke, SendStrokeKey};
pub use viewer::{
    HistoryDemand, MAXIMUM_READ_ROWS, MAXIMUM_READ_ROWS_BYTES, MAXIMUM_VIEWER_RECORD_BYTES,
    MINIMUM_READ_ROWS_BYTES, ProjectionDemand, TerminalViewerAction, TerminalViewerOutcome,
    TerminalViewerRequest, TerminalViewerResult, ViewerRejectedReason, ViewerRowDetail,
    ViewerRowsRejectedReason, ViewerSelectionExportCancelReason, ViewerSelectionExportFormat,
    ViewerSelectionGestureAction, ViewerSelectionGestureGeometry, ViewerSelectionGesturePosition,
    ViewerSelectionViewportPoint, ViewerSnapshotReason, ViewerUnavailableReason, ViewportRowOrigin,
};
pub use working_directory::{
    MAXIMUM_WORKING_DIRECTORY_BYTES, WorkingDirectory, WorkingDirectoryError,
};

pub use open::{OpenOutcome, TerminalOpenRequest, TerminalOpenResult};

mod launch_environment;
pub use launch_environment::{
    LaunchEnvironment, MAXIMUM_LAUNCH_ENVIRONMENT_BYTES, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
};

mod plugin_terminal_environment;
pub use plugin_terminal_environment::{
    MAXIMUM_PLUGIN_TERMINAL_ENVIRONMENT_BYTES, PLUGIN_TERMINAL_ENVIRONMENT_NAMES,
    PluginTerminalEnvironment,
};
