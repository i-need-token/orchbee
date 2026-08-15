const ghostty_vt = @import("ghostty-vt");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");

const max_row_window_rows = contracts.max_row_window_rows;
const max_pending_export_cancellations = contracts.max_pending_export_cancellations;
const AttachmentId = contracts.AttachmentId;
const HostEpoch = contracts.HostEpoch;
const Detail = contracts.Detail;
const Purpose = contracts.Purpose;
const Origin = contracts.Origin;
const ReadRequest = contracts.ReadRequest;
const EncodedRows = contracts.EncodedRows;
const HistoryDemand = contracts.HistoryDemand;
const AnchorStatus = contracts.AnchorStatus;
const HistoryEvent = contracts.HistoryEvent;
const SelectionPoint = contracts.SelectionPoint;
const SelectionGestureGeometry = contracts.SelectionGestureGeometry;
const SelectionGesturePosition = contracts.SelectionGesturePosition;
const SelectionGestureEvent = contracts.SelectionGestureEvent;
const SelectionGesture = contracts.SelectionGesture;
const SelectionCommand = contracts.SelectionCommand;
const SelectionCommandRequest = contracts.SelectionCommandRequest;
const SelectionSnapshot = contracts.SelectionSnapshot;
const ExportFormat = contracts.ExportFormat;
const ExportStarted = contracts.ExportStarted;
const ExportStartTransaction = contracts.ExportStartTransaction;
const ExportFinished = contracts.ExportFinished;
const ExportCancellation = contracts.ExportCancellation;
const PreparedExportCancellation = contracts.PreparedExportCancellation;
const ExportChunkToken = contracts.ExportChunkToken;
const EncodedExportChunk = contracts.EncodedExportChunk;
const ExportInvalidationSnapshot = contracts.ExportInvalidationSnapshot;
const HistoryError = contracts.HistoryError;

pub fn takeHistoryEvent(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    attachment_id: AttachmentId,
    revision: u64,
    generation: u64,
) HistoryError!?HistoryEvent {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    if (state.demand != .viewing_history or !state.history_dirty or
        state.history_in_flight != null or state.pending != null)
    {
        return null;
    }
    const anchor_status: AnchorStatus = if (state.committed) |anchor|
        if (storage.anchorValid(terminal, anchor)) .valid else .evicted
    else
        .none;
    const event: HistoryEvent = .{
        .anchor_status = anchor_status,
        .attachment_id = attachment_id,
        .generation = generation,
        .revision = revision,
        .sequence = state.history_next_sequence,
        .total_rows = @intCast(terminal.screens.active.pages.total_rows),
    };
    state.history_dirty = false;
    state.history_in_flight = event;
    // The sequence is consumed on installation and never restored by a
    // rollback, so it is monotonic and never reused once handed off.
    state.history_next_sequence += 1;
    return event;
}

pub fn rollbackHistoryEvent(
    self: anytype,
    attachment_id: AttachmentId,
    expected: HistoryEvent,
) HistoryError!void {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    const event = state.history_in_flight orelse {
        // A concurrent screen change invalidates the in-flight event and
        // marks history dirty again, which already has rollback semantics.
        if (state.history_dirty) return;
        return error.InvalidAcknowledgement;
    };
    if (!storage.sameHistoryEvent(event, expected)) return error.InvalidAcknowledgement;
    state.history_in_flight = null;
    state.history_dirty = true;
}

pub fn historyObserved(
    self: anytype,
    attachment_id: AttachmentId,
    sequence: u64,
    revision: u64,
    generation: u64,
) HistoryError!void {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    const event = state.history_in_flight orelse {
        // No event is in flight. A sequence below the next unissued identity
        // refers to an already observed or abandoned event and is an
        // idempotent no-op; a higher one was never issued.
        if (sequence < state.history_next_sequence) return;
        return error.InvalidAcknowledgement;
    };
    // A stale or duplicate acknowledgement older than the current flight is
    // an idempotent no-op and never clears a newer flight.
    if (sequence < event.sequence) return;
    if (sequence > event.sequence) return error.InvalidAcknowledgement;
    // Revision and generation remain validated for the exactly matching
    // flight, so a wrong identity never clears it.
    if (revision != event.revision or generation != event.generation) {
        return error.InvalidAcknowledgement;
    }
    state.history_in_flight = null;
}

pub fn takeSelectionEvent(
    self: anytype,
    attachment_id: AttachmentId,
) HistoryError!?SelectionSnapshot {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.selection_state.takeEvent();
}

pub fn rollbackSelectionEvent(
    self: anytype,
    attachment_id: AttachmentId,
    event: SelectionSnapshot,
) HistoryError!void {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    // A newer runtime event supersedes the unsent snapshot. Otherwise put
    // the exact event back so the next scheduler pass can retry it.
    state.selection_state.rollbackEvent(event);
}

pub fn takeSelectionExportCancellation(
    self: anytype,
    attachment_id: AttachmentId,
) HistoryError!?ExportCancellation {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.export_ledger.takeCancellation();
}

pub fn prepareSelectionExportCancellation(
    self: anytype,
    attachment_id: AttachmentId,
) HistoryError!?PreparedExportCancellation {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.export_ledger.prepareCancellation();
}

pub fn commitPreparedSelectionExportCancellation(
    self: anytype,
    prepared: PreparedExportCancellation,
) void {
    const expected = prepared.cancellation;
    const state = storage.find(self, expected.attachment_id) orelse unreachable;
    state.export_ledger.commitPreparedCancellation(prepared);
}

pub fn snapshotInvalidatedExportHistory(
    self: anytype,
    attachment_id: AttachmentId,
) HistoryError!ExportInvalidationSnapshot {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.export_ledger.snapshotInvalidations();
}

pub fn restoreInvalidatedExportHistory(
    self: anytype,
    attachment_id: AttachmentId,
    snapshot: ExportInvalidationSnapshot,
) void {
    const state = storage.find(self, attachment_id) orelse return;
    state.export_ledger.restoreInvalidations(snapshot);
}

pub fn restoreSelectionExportCancellations(
    self: anytype,
    attachment_id: AttachmentId,
    cancellations: []const ExportCancellation,
) HistoryError!void {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    try state.export_ledger.restoreCancellations(cancellations);
}
