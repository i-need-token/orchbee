const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const terminal_selection = @import("../selection.zig");
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
const AttachmentState = storage.AttachmentState;

pub fn rollbackExportStart(
    self: anytype,
    attachment_id: AttachmentId,
    export_sequence: u64,
    previous_completed_export_sequence: ?u64,
    previous_last_export_sequence: u64,
    cancellations: []const ExportCancellation,
    invalidation_snapshot: ExportInvalidationSnapshot,
) void {
    const state = storage.find(self, attachment_id) orelse return;
    state.export_ledger.rollbackStart(
        self.allocator,
        attachment_id,
        export_sequence,
        previous_completed_export_sequence,
        previous_last_export_sequence,
        cancellations,
        invalidation_snapshot,
    );
}

pub fn applyGesture(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    current_generation: u64,
    request: SelectionGesture,
) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
    if (request.generation != current_generation) return error.StaleGeneration;
    const state = storage.find(self, request.attachment_id) orelse return error.MissingAttachment;
    const selection_was_active = state.selection_state.active();
    const snapshot = try state.selection_state.applyGesture(terminal, .{
        .event = try resolveGestureEvent(terminal, state, request.event),
    });
    storage.updateSelectionCount(self, selection_was_active, state.selection_state.active());
    return snapshot;
}

fn resolveGestureEvent(
    terminal: *ghostty_vt.Terminal,
    state: *AttachmentState,
    event: SelectionGestureEvent,
) HistoryError!terminal_selection.ResolvedGestureEvent {
    return switch (event) {
        .start => |start| .{ .start = .{
            .click_count = start.click_count,
            .point = try resolveViewportPoint(terminal, state, start.point),
            .position = start.position,
            .select_output = start.select_output,
        } },
        .update => |update| .{ .update = .{
            .geometry = update.geometry,
            .point = try resolveViewportPoint(terminal, state, update.point),
            .position = update.position,
            .rectangle = update.rectangle,
        } },
        .end => |point| .{ .end = try resolveViewportPoint(terminal, state, point) },
        .cancel => .cancel,
    };
}

fn resolveViewportPoint(
    terminal: *ghostty_vt.Terminal,
    state: *AttachmentState,
    point: terminal_selection.ViewportPoint,
) HistoryError!SelectionPoint {
    const screen = terminal.screens.active;
    if (point.column >= terminal.cols or point.row >= terminal.rows) return error.InvalidPoint;

    if (state.demand == .viewing_history) {
        if (state.committed) |anchor| {
            if (anchor.screen_key != terminal.screens.active_key or
                !storage.anchorValid(terminal, anchor))
            {
                return error.AnchorEvicted;
            }
            const top = screen.pages.pointFromPin(.screen, anchor.pin.*) orelse
                return error.AnchorEvicted;
            const row = std.math.add(u64, top.screen.y, point.row) catch
                return error.InvalidPoint;
            if (row >= screen.pages.total_rows) return error.InvalidPoint;
            return .{ .column = point.column, .row = row };
        }
    }

    const pin = screen.pages.pin(.{ .viewport = .{
        .x = point.column,
        .y = point.row,
    } }) orelse return error.InvalidPoint;
    const absolute = screen.pages.pointFromPin(.screen, pin) orelse return error.InvalidPoint;
    return .{ .column = absolute.screen.x, .row = absolute.screen.y };
}

pub fn applySelectionCommand(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    current_revision: u64,
    current_generation: u64,
    request: SelectionCommandRequest,
) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
    try terminal_selection.validateCommand(
        request.command,
        request.revision,
        request.generation,
        current_revision,
        current_generation,
    );
    const state = storage.find(self, request.attachment_id) orelse return error.MissingAttachment;
    const selection_was_active = state.selection_state.active();
    const snapshot = try state.selection_state.applyCommand(
        terminal,
        current_revision,
        current_generation,
        request,
    );
    storage.updateSelectionCount(self, selection_was_active, state.selection_state.active());
    return snapshot;
}

pub fn startExport(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    attachment_id: AttachmentId,
    export_sequence: u64,
    format: ExportFormat,
) (HistoryError || std.mem.Allocator.Error)!ExportStarted {
    if (export_sequence == 0) return error.InvalidRequest;
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    try state.export_ledger.validateStart(export_sequence);
    const selection = try exportSelection(self, terminal, state);
    return state.export_ledger.start(
        self.allocator,
        terminal,
        selection,
        attachment_id,
        export_sequence,
        format,
    );
}

pub fn startExportTransaction(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    attachment_id: AttachmentId,
    export_sequence: u64,
    format: ExportFormat,
) (HistoryError || std.mem.Allocator.Error)!ExportStartTransaction {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    try state.export_ledger.validateStart(export_sequence);
    const selection = try exportSelection(self, terminal, state);
    return state.export_ledger.startTransaction(
        self.allocator,
        terminal,
        selection,
        attachment_id,
        export_sequence,
        format,
    );
}

pub fn encodeNextExportChunk(
    self: anytype,
    output_allocator: std.mem.Allocator,
    attachment_id: AttachmentId,
) (HistoryError || std.mem.Allocator.Error)!?EncodedExportChunk {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.export_ledger.encodeNextChunk(output_allocator, attachment_id);
}

pub fn rollbackExportChunk(
    self: anytype,
    expected: ExportChunkToken,
) HistoryError!void {
    const state = storage.find(self, expected.attachment_id) orelse return error.MissingAttachment;
    try state.export_ledger.rollbackChunk(expected);
}

pub fn exportChunkApplied(
    self: anytype,
    attachment_id: AttachmentId,
    export_sequence: u64,
    chunk_sequence: u32,
) HistoryError!?ExportFinished {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.export_ledger.chunkApplied(
        self.allocator,
        attachment_id,
        export_sequence,
        chunk_sequence,
    );
}

pub fn cancelExport(
    self: anytype,
    attachment_id: AttachmentId,
    export_sequence: u64,
) HistoryError!ExportFinished {
    const state = storage.find(self, attachment_id) orelse return error.MissingAttachment;
    return state.export_ledger.cancel(self.allocator, attachment_id, export_sequence);
}

pub fn exportSelection(
    self: anytype,
    terminal: *ghostty_vt.Terminal,
    state: *AttachmentState,
) HistoryError!ghostty_vt.Selection {
    const selection_was_active = state.selection_state.active();
    defer storage.updateSelectionCount(
        self,
        selection_was_active,
        state.selection_state.active(),
    );
    return state.selection_state.exportSelection(terminal);
}
