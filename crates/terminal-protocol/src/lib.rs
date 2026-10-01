mod attachment;
mod control;
mod creation;
mod envelope;
mod error;
mod frame_route;
mod geometry;
mod history;
mod identity;
mod input;
mod input_authority;
mod inventory;
mod lifecycle;
mod limits;
mod message;
mod process_info;
mod projection_buffer;
mod projection_demand;
mod projection_feedback;
mod resize;
mod row_access;
mod row_window;
mod screen_control;
mod selection;
mod selection_export;
mod selection_export_chunk;
mod semantic_input;
mod terminal_configuration;
mod terminal_effect;
mod terminal_fact;
mod version;

pub use attachment::{
    AttachTerminal, AttachmentCapabilities, AttachmentCapabilityState, AttachmentControl,
    AttachmentTarget, EncodedAttachmentControl, ResizeLeaseChanged,
    UpdateAttachmentInputCapability, decode_attachment_control, encode_attachment_control,
};
pub use control::{
    ConnectionControl, EncodedConnectionControl, Hello, HelloAccepted, ProtocolRejected,
    ProtocolRejectionReason, TypedFailure, decode_connection_control, encode_connection_control,
};
pub use creation::{
    CreateScrollbackEditor, CreateTerminal, CursorShape, DeviceAttributes,
    EncodedTerminalCreationControl, EnvironmentOverlayEntry, LaunchEnvironmentEntry,
    LaunchStartMode, ScrollbackSource, TerminalCreationControl, TerminalLaunch, TerminalProfile,
    decode_terminal_creation_control, encode_terminal_creation_control,
};
pub use envelope::{
    DecodedEnvelope, Envelope, decode_transport_envelope, encode_transport_envelope,
};
pub use error::{ProtocolError, ProtocolErrorCode};
pub use frame_route::{
    TERMINAL_FRAME_HEADER_BYTES, TerminalFrameRoute, inspect_terminal_frame_route,
};
pub use geometry::TerminalPixelGeometry;
pub use history::{
    EncodedHistoryControl, HistoryAnchorStatus, HistoryChanged, HistoryControl, HistoryDemand,
    HistoryDemandAccepted, HistoryObserved, SetHistoryDemand, decode_history_control,
    encode_history_control,
};
pub use identity::{
    AttachmentId, ClientId, CorrelationId, CreationId, HostEpoch, IdentityError, TerminalId,
    TerminalThemeOperationId,
};
pub use input::{
    EncodedInputMessage, InputAccepted, InputMessage, MAXIMUM_WRITE_INPUT_PAYLOAD_BYTES,
    WRITE_INPUT_HEADER_BYTES, WriteInput, decode_input_message, encode_input_message,
};
pub use inventory::{
    ColorScheme, EncodedInventoryControl, FailureContainment, InventoryControl, ListTerminals,
    TerminalGeometry, TerminalInventoryCursor, TerminalList, TerminalSummary, TerminalSummaryCore,
    TerminalTheme, TerminalThemeState, decode_inventory_control, encode_inventory_control,
};
pub use lifecycle::{
    EncodedLifecycleControl, LifecycleControl, StartTerminal, StartTransition, TerminalExited,
    TerminalFailed, TerminalStartAccepted, TerminalTarget, decode_lifecycle_control,
    encode_lifecycle_control,
};
pub use limits::{
    MAXIMUM_CELL_PIXEL_DIMENSION, MAXIMUM_CLIPBOARD_TEXT_BYTES, MAXIMUM_CONTROL_JSON_BYTES,
    MAXIMUM_ENVIRONMENT_NAME_BYTES, MAXIMUM_ENVIRONMENT_OVERLAY_ENTRIES,
    MAXIMUM_ENVIRONMENT_OVERLAY_VALUE_BYTES, MAXIMUM_FRAME_CELLS, MAXIMUM_FRAME_COLUMNS,
    MAXIMUM_FRAME_ROWS, MAXIMUM_INPUT_BYTES, MAXIMUM_INVENTORY_PAGE_TERMINALS,
    MAXIMUM_KEY_TEXT_BYTES, MAXIMUM_LAUNCH_ARGUMENTS, MAXIMUM_LAUNCH_ENVIRONMENT_ENTRIES,
    MAXIMUM_OPTIONAL_FEATURE_NAME_BYTES, MAXIMUM_OPTIONAL_FEATURES, MAXIMUM_PASTE_AUTO_INPUT_BYTES,
    MAXIMUM_PASTE_AUTO_INPUT_PAYLOAD_BYTES, MAXIMUM_RAW_PAYLOAD_BYTES, MAXIMUM_RGB_VALUE,
    MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_ROW_WINDOW_ROWS, MAXIMUM_SCROLLBACK_BYTES,
    MAXIMUM_SELECTION_EXPORT_BYTES, MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES,
    MAXIMUM_SELECTION_PREVIEW_BYTES, MAXIMUM_SEMANTIC_INPUT_PAYLOAD_BYTES,
    MAXIMUM_TERMINAL_APPLICATION_SIGNAL_BYTES, MAXIMUM_TERMINAL_FRAME_BYTES,
    MAXIMUM_TERMINAL_METADATA_TEXT_BYTES, MAXIMUM_TERMINAL_PIXEL_EXTENT,
    MAXIMUM_TERMINAL_PROFILE_FEATURES, MAXIMUM_TERMINAL_RESPONSE_BYTES, MAXIMUM_TEXT_CELL_BYTES,
    MAXIMUM_TRANSPORT_RECORD_BYTES, MAXIMUM_TYPED_FAILURE_CODE_BYTES,
    MAXIMUM_TYPED_FAILURE_MESSAGE_BYTES, TERMINAL_THEME_PALETTE_ENTRIES,
    TRANSPORT_ENVELOPE_HEADER_BYTES,
};
pub use message::{
    MESSAGE_CONTRACTS, MessageContract, MessageRole, PayloadEncoding, RequestMessageContract,
    message_contract,
};
pub use process_info::{
    EncodedProcessInfoControl, MAXIMUM_PROCESS_SAMPLE_ROWS, MAXIMUM_PROCESS_TEXT_UTF16_UNITS,
    ProcessInfoControl, READ_PROCESS_INFO_KIND, TerminalProcess, TerminalProcessInfo,
    decode_process_info_control, encode_process_info_control,
};
pub use projection_buffer::{
    TerminalProjectionBuffer, TerminalProjectionCell, TerminalProjectionRow,
    decode_terminal_projection_buffer,
};
pub use projection_demand::{
    EncodedProjectionDemandControl, ProjectionBlockReason, ProjectionBlocked, ProjectionDemand,
    ProjectionDemandAccepted, ProjectionDemandControl, SetProjectionDemand,
    decode_projection_demand_control, encode_projection_demand_control,
};
pub use projection_feedback::{
    EncodedProjectionFeedbackControl, FrameApplied, NeedSnapshot, ProjectionFeedbackControl,
    SnapshotReason, decode_projection_feedback_control, encode_projection_feedback_control,
};
pub use resize::{
    EncodedResizeControl, ResizeAccepted, ResizeControl, ResizeTerminal, decode_resize_control,
    encode_resize_control,
};
pub use row_access::{
    EncodedRowAccessControl, ReadRows, RowAccessControl, RowRevisionPolicy, RowWindowDetail,
    RowWindowOrigin, RowWindowPurpose, RowsApplied, decode_row_access_control,
    encode_row_access_control,
};
pub use row_window::{
    TERMINAL_ROW_WINDOW_HEADER_BYTES, TerminalRenderRowWindow, TerminalRowPromptClass,
    TerminalRowWindowRoute, TerminalTextCellMapping, TerminalTextRowWindow, TerminalTextWindowRow,
    decode_terminal_render_row_window, decode_terminal_text_row_window,
    extract_terminal_text_selection, inspect_terminal_row_window_route,
};
pub use screen_control::{
    ClearTerminal, EncodedScreenControl, ScreenControl, ScreenControlOperation,
    TerminalControlAccepted, decode_screen_control, encode_screen_control,
};
pub use selection::{
    EncodedSelectionControl, SelectionChanged, SelectionChangedState, SelectionCommand,
    SelectionCommandKind, SelectionControl, SelectionGesture, SelectionGestureAction,
    SelectionGestureGeometry, SelectionGesturePosition, SelectionPoint, SelectionViewportPoint,
    decode_selection_control, encode_selection_control,
};
pub use selection_export::{
    EncodedSelectionExportControl, SelectionExport, SelectionExportCancel,
    SelectionExportCancelReason, SelectionExportChunkApplied, SelectionExportControl,
    SelectionExportFinished, SelectionExportFormat, SelectionExportStarted, SelectionExportStatus,
    decode_selection_export_control, encode_selection_export_control,
};
pub use selection_export_chunk::{
    EncodedSelectionExportChunk, MAXIMUM_SELECTION_EXPORT_CHUNK_PAYLOAD_BYTES,
    SELECTION_EXPORT_CHUNK_HEADER_BYTES, SelectionExportChunk, SelectionExportChunkRoute,
    decode_selection_export_chunk, decode_selection_export_chunk_route,
    encode_selection_export_chunk,
};
pub use semantic_input::{
    FOCUS_INPUT_BYTES, FocusInput, KEY_INPUT_HEADER_BYTES, KeyAction, KeyInput, MOUSE_INPUT_BYTES,
    MouseAction, MouseButton, MouseInput, PASTE_AUTO_INPUT_HEADER_BYTES, PASTE_INPUT_HEADER_BYTES,
    PasteAutoInput, PasteInput, TEXT_INPUT_HEADER_BYTES, TerminalKeyCode, TerminalModifiers,
    TextInput,
};
pub use terminal_configuration::{
    EncodedTerminalConfigurationControl, SetTerminalDefaults, TerminalConfiguration,
    TerminalConfigurationAccepted, TerminalConfigurationControl, TerminalDefaultsAccepted,
    UpdateTerminalConfiguration, decode_terminal_configuration_control,
    encode_terminal_configuration_control,
};
pub use terminal_effect::{
    EncodedTerminalEffectEvent, TerminalApplicationSignal, TerminalApplicationSignalChannel,
    TerminalBell, TerminalClipboardWriteRequest, TerminalEffectEvent, TerminalMetadataChanged,
    decode_terminal_effect_event, encode_terminal_effect_event,
};
pub use terminal_fact::{
    ExitResult, ProcessObservation, RuntimeCleanupFailure, RuntimeFailure, TerminalIdentity,
};
pub use version::{CURRENT_PROTOCOL_VERSION, ProtocolVersion};

mod text_frame;
pub use text_frame::{
    MouseFormat, MouseTracking, TerminalInteraction, TerminalProgress, TerminalProgressValue,
    TerminalTextCell, TerminalTextCursor, TerminalTextFrame, TerminalTextRow, TerminalTextStyle,
    decode_terminal_text_frame,
};
