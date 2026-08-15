const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");
const selection_ops = @import("selection_ops.zig");

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
const AttachmentState = storage.AttachmentState;

pub fn deinit(self: anytype, terminal: *ghostty_vt.Terminal) void {
    for (self.states.items) |*state| storage.clearState(self, terminal, state);
    self.states.deinit(self.allocator);
    self.* = undefined;
}

pub fn attach(self: anytype, attachment_id: AttachmentId) !void {
    if (storage.find(self, attachment_id) != null) return;
    try self.states.append(self.allocator, .{ .attachment_id = attachment_id });
}

pub fn reserveAttachment(self: anytype, attachment_id: AttachmentId) !void {
    std.debug.assert(storage.find(self, attachment_id) == null);
    try self.states.ensureUnusedCapacity(self.allocator, 1);
}

pub fn commitReservedAttachment(self: anytype, attachment_id: AttachmentId) void {
    std.debug.assert(storage.find(self, attachment_id) == null);
    self.states.appendAssumeCapacity(.{ .attachment_id = attachment_id });
}

pub fn detach(self: anytype, terminal: *ghostty_vt.Terminal, attachment_id: AttachmentId) void {
    const index = storage.findIndex(self, attachment_id) orelse return;
    var state = self.states.orderedRemove(index);
    if (state.demand == .viewing_history) self.viewing_count -= 1;
    storage.clearState(self, terminal, &state);
}

pub fn setDemand(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    attachment_id: AttachmentId,
    demand: HistoryDemand,
) HistoryError!void {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    if (state.demand == demand) return;
    if (state.demand == .viewing_history) self.viewing_count -= 1;
    state.demand = demand;
    if (demand == .viewing_history) {
        self.viewing_count += 1;
        state.history_dirty = true;
    } else {
        storage.releaseAnchor(self, terminal, &state.committed);
        storage.releaseAnchor(self, terminal, &state.pending);
        state.history_dirty = false;
        state.history_in_flight = null;
    }
}

pub fn noteMutation(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    revision: u64,
    generation: u64,
) void {
    if (self.viewing_count == 0 and self.selection_count == 0) return;
    for (self.states.items) |*state| {
        if (state.demand == .viewing_history) state.history_dirty = true;
        if (state.selection_state.noteMutation(terminal)) {
            self.selection_count -= 1;
            storage.invalidateExport(self, state);
        }
        if (state.committed) |*anchor| {
            anchor.revision = revision;
            anchor.generation = generation;
        }
    }
}

pub fn generationChanged(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    revision: u64,
    generation: u64,
) void {
    for (self.states.items) |*state| {
        storage.releaseAnchor(self, terminal, &state.pending);
        state.history_in_flight = null;

        if (state.committed) |*anchor| {
            if (storage.anchorValid(terminal, anchor.*)) {
                anchor.revision = revision;
                anchor.generation = generation;
            } else {
                storage.releaseAnchor(self, terminal, &state.committed);
            }
        }

        const selection_was_active = state.selection_state.active();
        state.selection_state.generationChanged(terminal);
        storage.updateSelectionCount(self, selection_was_active, state.selection_state.active());
        storage.invalidateExport(self, state);

        if (state.demand == .viewing_history) state.history_dirty = true;
    }
}

pub fn screenChanged(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
) void {
    for (self.states.items) |*state| {
        storage.releaseAnchor(self, terminal, &state.committed);
        storage.releaseAnchor(self, terminal, &state.pending);
        const selection_was_active = state.selection_state.active();
        state.selection_state.screenChanged(terminal);
        storage.updateSelectionCount(self, selection_was_active, state.selection_state.active());
        storage.invalidateExport(self, state);
        if (state.demand == .viewing_history) {
            state.history_in_flight = null;
            state.history_dirty = true;
        }
    }
}
