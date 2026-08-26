const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const contracts = @import("contracts.zig");
const storage = @import("storage.zig");
const attachments_ops = @import("attachments.zig");
const rows_ops = @import("rows.zig");
const events_ops = @import("events.zig");
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

pub const Manager = struct {
    allocator: std.mem.Allocator,
    animation_tick: u64 = 0,
    selection_count: usize = 0,
    states: std.ArrayList(AttachmentState) = .empty,
    viewing_count: usize = 0,

    pub fn init(allocator: std.mem.Allocator) Manager {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Manager, terminal: *ghostty_vt.Terminal) void {
        return attachments_ops.deinit(self, terminal);
    }

    pub fn attach(self: *Manager, attachment_id: AttachmentId) !void {
        return attachments_ops.attach(self, attachment_id);
    }

    pub fn reserveAttachment(self: *Manager, attachment_id: AttachmentId) !void {
        return attachments_ops.reserveAttachment(self, attachment_id);
    }

    pub fn commitReservedAttachment(self: *Manager, attachment_id: AttachmentId) void {
        return attachments_ops.commitReservedAttachment(self, attachment_id);
    }

    pub fn detach(self: *Manager, terminal: *ghostty_vt.Terminal, attachment_id: AttachmentId) void {
        return attachments_ops.detach(self, terminal, attachment_id);
    }

    pub fn setDemand(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        demand: HistoryDemand,
    ) HistoryError!void {
        return attachments_ops.setDemand(self, terminal, attachment_id, demand);
    }

    pub fn readRows(
        self: *Manager,
        output_allocator: std.mem.Allocator,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        request: ReadRequest,
    ) (HistoryError || std.mem.Allocator.Error)!EncodedRows {
        return rows_ops.readRows(self, output_allocator, terminal, current_revision, current_generation, cell_pixel_width, cell_pixel_height, request);
    }

    pub fn readRowsInto(
        self: *Manager,
        scratch_allocator: std.mem.Allocator,
        destination: []u8,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        request: ReadRequest,
    ) (HistoryError || std.mem.Allocator.Error)!EncodedRows {
        return rows_ops.readRowsInto(self, scratch_allocator, destination, terminal, current_revision, current_generation, cell_pixel_width, cell_pixel_height, request);
    }

    pub fn rowsApplied(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        request_sequence: u64,
        revision: u64,
        generation: u64,
    ) HistoryError!void {
        return rows_ops.rowsApplied(self, terminal, attachment_id, request_sequence, revision, generation);
    }

    pub fn noteMutation(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        generation: u64,
    ) void {
        return attachments_ops.noteMutation(self, terminal, revision, generation);
    }

    pub fn generationChanged(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        generation: u64,
    ) void {
        return attachments_ops.generationChanged(self, terminal, revision, generation);
    }

    pub fn screenChanged(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
    ) void {
        return attachments_ops.screenChanged(self, terminal);
    }

    pub fn takeHistoryEvent(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        revision: u64,
        generation: u64,
    ) HistoryError!?HistoryEvent {
        return events_ops.takeHistoryEvent(self, terminal, attachment_id, revision, generation);
    }

    pub fn rollbackHistoryEvent(
        self: *Manager,
        attachment_id: AttachmentId,
        expected: HistoryEvent,
    ) HistoryError!void {
        return events_ops.rollbackHistoryEvent(self, attachment_id, expected);
    }

    pub fn historyObserved(
        self: *Manager,
        attachment_id: AttachmentId,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) HistoryError!void {
        return events_ops.historyObserved(self, attachment_id, sequence, revision, generation);
    }

    pub fn takeSelectionEvent(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?SelectionSnapshot {
        return events_ops.takeSelectionEvent(self, attachment_id);
    }

    pub fn rollbackSelectionEvent(
        self: *Manager,
        attachment_id: AttachmentId,
        event: SelectionSnapshot,
    ) HistoryError!void {
        return events_ops.rollbackSelectionEvent(self, attachment_id, event);
    }

    pub fn takeSelectionExportCancellation(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?ExportCancellation {
        return events_ops.takeSelectionExportCancellation(self, attachment_id);
    }

    pub fn prepareSelectionExportCancellation(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?PreparedExportCancellation {
        return events_ops.prepareSelectionExportCancellation(self, attachment_id);
    }

    pub fn commitPreparedSelectionExportCancellation(
        self: *Manager,
        prepared: PreparedExportCancellation,
    ) void {
        return events_ops.commitPreparedSelectionExportCancellation(self, prepared);
    }

    pub fn snapshotInvalidatedExportHistory(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!ExportInvalidationSnapshot {
        return events_ops.snapshotInvalidatedExportHistory(self, attachment_id);
    }

    pub fn restoreInvalidatedExportHistory(
        self: *Manager,
        attachment_id: AttachmentId,
        snapshot: ExportInvalidationSnapshot,
    ) void {
        return events_ops.restoreInvalidatedExportHistory(self, attachment_id, snapshot);
    }

    pub fn restoreSelectionExportCancellations(
        self: *Manager,
        attachment_id: AttachmentId,
        cancellations: []const ExportCancellation,
    ) HistoryError!void {
        return events_ops.restoreSelectionExportCancellations(self, attachment_id, cancellations);
    }

    pub fn rollbackExportStart(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
        previous_completed_export_sequence: ?u64,
        previous_last_export_sequence: u64,
        cancellations: []const ExportCancellation,
        invalidation_snapshot: ExportInvalidationSnapshot,
    ) void {
        return selection_ops.rollbackExportStart(self, attachment_id, export_sequence, previous_completed_export_sequence, previous_last_export_sequence, cancellations, invalidation_snapshot);
    }

    pub fn applyGesture(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: SelectionGesture,
    ) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
        return selection_ops.applyGesture(self, terminal, current_revision, current_generation, request);
    }

    pub fn applySelectionCommand(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: SelectionCommandRequest,
    ) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
        return selection_ops.applySelectionCommand(self, terminal, current_revision, current_generation, request);
    }

    pub fn startExport(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: ExportFormat,
    ) (HistoryError || std.mem.Allocator.Error)!ExportStarted {
        return selection_ops.startExport(self, terminal, attachment_id, export_sequence, format);
    }

    pub fn startExportTransaction(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: ExportFormat,
    ) (HistoryError || std.mem.Allocator.Error)!ExportStartTransaction {
        return selection_ops.startExportTransaction(self, terminal, attachment_id, export_sequence, format);
    }

    pub fn encodeNextExportChunk(
        self: *Manager,
        output_allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
    ) (HistoryError || std.mem.Allocator.Error)!?EncodedExportChunk {
        return selection_ops.encodeNextExportChunk(self, output_allocator, attachment_id);
    }

    pub fn rollbackExportChunk(
        self: *Manager,
        expected: ExportChunkToken,
    ) HistoryError!void {
        return selection_ops.rollbackExportChunk(self, expected);
    }

    pub fn exportChunkApplied(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
        chunk_sequence: u32,
    ) HistoryError!?ExportFinished {
        return selection_ops.exportChunkApplied(self, attachment_id, export_sequence, chunk_sequence);
    }

    pub fn cancelExport(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
    ) HistoryError!ExportFinished {
        return selection_ops.cancelExport(self, attachment_id, export_sequence);
    }
};
