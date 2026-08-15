const ghostty_vt = @import("ghostty-vt");
const terminal_selection = @import("../selection.zig");
const selection_export = @import("../selection_export.zig");
const contracts = @import("contracts.zig");

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

pub const TrackedAnchor = struct {
    generation: u64,
    pin: *ghostty_vt.Pin,
    request_sequence: u64,
    revision: u64,
    screen_generation: usize,
    screen_key: ghostty_vt.ScreenSet.Key,
};

// The immutable acknowledgement identity of the latest applied viewport row
// window. It is recorded when pending rows commit and is never rewritten by
// noteMutation or generationChanged, which only refresh the mutable committed
// anchor's revision and generation. An exact replay of this tuple is an
// idempotent no-op even while a newer row window is pending.
pub const AppliedRowsIdentity = struct {
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub const AttachmentState = struct {
    attachment_id: AttachmentId,
    committed: ?TrackedAnchor = null,
    committed_ack: ?AppliedRowsIdentity = null,
    demand: HistoryDemand = .following_bottom,
    export_ledger: selection_export.State = .{},
    history_dirty: bool = false,
    history_in_flight: ?HistoryEvent = null,
    // The next positive attachment-local history-event sequence. It advances
    // once per installed event and is never reused, restored, or reset by a
    // demand change, so any acknowledgement older than the current in-flight
    // event (or than this watermark when no event is in flight) refers to an
    // already observed or abandoned event and is an idempotent no-op.
    history_next_sequence: u64 = 1,
    last_row_request_sequence: u64 = 0,
    pending: ?TrackedAnchor = null,
    selection_state: terminal_selection.State = .{},
};

pub fn clearState(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    state: *AttachmentState,
) void {
    releaseAnchor(self, terminal, &state.committed);
    releaseAnchor(self, terminal, &state.pending);
    const selection_was_active = state.selection_state.active();
    state.selection_state.clear(terminal);
    updateSelectionCount(self, selection_was_active, false);
    state.export_ledger.deinit(self.allocator);
}

pub fn releaseAnchor(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    anchor: *?TrackedAnchor,
) void {
    _ = self;
    const value = anchor.* orelse return;
    anchor.* = null;
    if (terminal.screens.generation(value.screen_key) != value.screen_generation) return;
    const screen = terminal.screens.get(value.screen_key) orelse return;
    screen.pages.untrackPin(value.pin);
}

pub fn updateSelectionCount(self: anytype, was_active: bool, is_active: bool) void {
    if (was_active == is_active) return;
    if (is_active) {
        self.selection_count += 1;
    } else {
        self.selection_count -= 1;
    }
}

pub fn invalidateExport(self: anytype, state: *AttachmentState) void {
    state.export_ledger.invalidate(self.allocator);
}

pub fn find(self: anytype, attachment_id: AttachmentId) ?*AttachmentState {
    const index = findIndex(self, attachment_id) orelse return null;
    return &self.states.items[index];
}

pub fn findIndex(self: anytype, attachment_id: AttachmentId) ?usize {
    for (self.states.items, 0..) |state, index| {
        if (state.attachment_id.eql(attachment_id)) return index;
    }
    return null;
}

pub fn anchorValid(terminal: *ghostty_vt.Terminal, anchor: TrackedAnchor) bool {
    if (terminal.screens.generation(anchor.screen_key) != anchor.screen_generation) return false;
    const screen = terminal.screens.get(anchor.screen_key) orelse return false;
    return anchorValidScreen(screen, anchor);
}

pub fn anchorValidScreen(screen: *ghostty_vt.Screen, anchor: TrackedAnchor) bool {
    return !anchor.pin.garbage and screen.pages.pointFromPin(.screen, anchor.pin.*) != null;
}

pub fn sameHistoryEvent(left: HistoryEvent, right: HistoryEvent) bool {
    return left.anchor_status == right.anchor_status and
        left.attachment_id.eql(right.attachment_id) and
        left.generation == right.generation and
        left.revision == right.revision and
        left.sequence == right.sequence and
        left.total_rows == right.total_rows;
}
