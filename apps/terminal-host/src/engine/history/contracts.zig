const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const terminal_interaction = model.interaction;
const protocol = @import("terminal-protocol");
const terminal_selection = @import("../selection.zig");
const selection_export = @import("../selection_export.zig");

pub const max_row_window_rows = protocol.Limits.row_window_rows;
pub const max_pending_export_cancellations = selection_export.max_pending_cancellations;

pub const AttachmentId = identifiers.AttachmentId;
pub const HostEpoch = identifiers.HostEpoch;

pub const Detail = protocol.RowWindowDetail;

pub const Purpose = terminal_interaction.Purpose;
pub const Origin = terminal_interaction.Origin;

pub const ReadRequest = struct {
    attachment_id: AttachmentId,
    detail: Detail,
    generation: u64,
    host_epoch: HostEpoch,
    maximum_bytes: usize,
    origin: Origin,
    purpose: Purpose,
    request_sequence: u64,
    revision: u64,
    theme_revision: u64 = 0,
    row_count: u16,
};

pub const EncodedRows = struct {
    bytes: []u8,
    generation: u64,
    revision: u64,
    row_count: u16,
    sequence: u64,
    start_row: u64,
    total_rows: u64,
};

pub const HistoryDemand = terminal_interaction.HistoryDemand;

pub const AnchorStatus = enum {
    evicted,
    none,
    valid,
};

pub const HistoryEvent = struct {
    anchor_status: AnchorStatus,
    attachment_id: AttachmentId,
    generation: u64,
    revision: u64,
    sequence: u64,
    total_rows: u64,
};

pub const SelectionPoint = terminal_selection.Point;
pub const SelectionGestureGeometry = terminal_selection.GestureGeometry;
pub const SelectionGesturePosition = terminal_selection.GesturePosition;
pub const SelectionGestureEvent = terminal_selection.GestureEvent;
pub const SelectionGesture = terminal_selection.Gesture(AttachmentId);
pub const SelectionCommand = terminal_selection.Command;
pub const SelectionCommandRequest = terminal_selection.CommandRequest(AttachmentId);
pub const SelectionSnapshot = terminal_selection.Snapshot;

pub const ExportFormat = selection_export.Format;
pub const ExportStarted = selection_export.Started;
pub const ExportStartTransaction = selection_export.StartTransaction;
pub const ExportFinished = selection_export.Finished;
pub const ExportCancellation = selection_export.Cancellation;
pub const PreparedExportCancellation = selection_export.PreparedCancellation;
pub const ExportChunkToken = selection_export.ChunkToken;
pub const EncodedExportChunk = selection_export.EncodedChunk;
pub const ExportInvalidationSnapshot = selection_export.InvalidationSnapshot;

pub const HistoryError = terminal_selection.Error || error{
    AttachmentConflict,
    ExportConflict,
    ExportTooLarge,
    InvalidAcknowledgement,
    MissingAttachment,
    ProjectionTooLarge,
};
