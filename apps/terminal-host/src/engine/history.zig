const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const row_encoding = @import("row_encoding.zig");
const terminal_selection = @import("selection.zig");
const selection_export = @import("selection_export.zig");

const max_row_window_bytes = row_encoding.max_bytes;
pub const max_row_window_rows = protocol.Limits.row_window_rows;
const row_window_header_bytes = row_encoding.header_bytes;
const max_row_window_entries = row_encoding.max_entries;
pub const max_pending_export_cancellations = selection_export.max_pending_cancellations;

pub const AttachmentId = identifiers.AttachmentId;
pub const HostEpoch = identifiers.HostEpoch;

pub const Detail = protocol.RowWindowDetail;

pub const Purpose = enum {
    buffer,
    search,
    viewport,
};

pub const Origin = union(enum) {
    absolute: u64,
    committed: i32,
    oldest: u32,
    tail: u32,
};

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

const RowEncodingTarget = union(enum) {
    allocated: std.mem.Allocator,
    fixed: struct {
        destination: []u8,
        scratch_allocator: std.mem.Allocator,
    },
};

pub const HistoryDemand = enum {
    following_bottom,
    viewing_history,
};

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

const TrackedAnchor = struct {
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
const AppliedRowsIdentity = struct {
    generation: u64,
    revision: u64,
    sequence: u64,
};

const AttachmentState = struct {
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

pub const Manager = struct {
    allocator: std.mem.Allocator,
    selection_count: usize = 0,
    states: std.ArrayList(AttachmentState) = .empty,
    viewing_count: usize = 0,

    pub fn init(allocator: std.mem.Allocator) Manager {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Manager, terminal: *ghostty_vt.Terminal) void {
        for (self.states.items) |*state| self.clearState(terminal, state);
        self.states.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn attach(self: *Manager, attachment_id: AttachmentId) !void {
        if (self.find(attachment_id) != null) return;
        try self.states.append(self.allocator, .{ .attachment_id = attachment_id });
    }

    pub fn reserveAttachment(self: *Manager, attachment_id: AttachmentId) !void {
        std.debug.assert(self.find(attachment_id) == null);
        try self.states.ensureUnusedCapacity(self.allocator, 1);
    }

    pub fn commitReservedAttachment(self: *Manager, attachment_id: AttachmentId) void {
        std.debug.assert(self.find(attachment_id) == null);
        self.states.appendAssumeCapacity(.{ .attachment_id = attachment_id });
    }

    pub fn detach(self: *Manager, terminal: *ghostty_vt.Terminal, attachment_id: AttachmentId) void {
        const index = self.findIndex(attachment_id) orelse return;
        var state = self.states.orderedRemove(index);
        if (state.demand == .viewing_history) self.viewing_count -= 1;
        self.clearState(terminal, &state);
    }

    pub fn setDemand(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        demand: HistoryDemand,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.demand == demand) return;
        if (state.demand == .viewing_history) self.viewing_count -= 1;
        state.demand = demand;
        if (demand == .viewing_history) {
            self.viewing_count += 1;
            state.history_dirty = true;
        } else {
            self.releaseAnchor(terminal, &state.committed);
            self.releaseAnchor(terminal, &state.pending);
            state.history_dirty = false;
            state.history_in_flight = null;
        }
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
        return self.readRowsTo(
            .{ .allocated = output_allocator },
            terminal,
            current_revision,
            current_generation,
            cell_pixel_width,
            cell_pixel_height,
            request,
        );
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
        return self.readRowsTo(
            .{ .fixed = .{
                .destination = destination,
                .scratch_allocator = scratch_allocator,
            } },
            terminal,
            current_revision,
            current_generation,
            cell_pixel_width,
            cell_pixel_height,
            request,
        );
    }

    fn readRowsTo(
        self: *Manager,
        target: RowEncodingTarget,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
        request: ReadRequest,
    ) (HistoryError || std.mem.Allocator.Error)!EncodedRows {
        if (request.revision != current_revision) return error.StaleRevision;
        if (request.generation != current_generation) return error.StaleGeneration;
        if (request.request_sequence == 0 or request.row_count == 0 or
            request.row_count > max_row_window_rows or request.maximum_bytes < row_window_header_bytes or
            request.maximum_bytes > max_row_window_bytes)
        {
            return error.InvalidRequest;
        }
        const state = self.find(request.attachment_id) orelse return error.MissingAttachment;
        if (request.request_sequence <= state.last_row_request_sequence) {
            return error.InvalidRequest;
        }
        if (request.purpose == .viewport and state.pending != null) return error.InvalidRequest;

        const screen = terminal.screens.active;
        const total_rows: u64 = @intCast(screen.pages.total_rows);
        if (total_rows == 0 or total_rows > std.math.maxInt(u32)) return error.InvalidRequest;
        const maximum_rows_by_entries =
            @as(usize, max_row_window_entries) / @as(usize, terminal.cols);
        const bounded_request_rows: u16 = @intCast(@min(
            @as(usize, request.row_count),
            maximum_rows_by_entries,
        ));
        const start_row = try self.resolveStart(
            screen,
            state,
            request.origin,
            bounded_request_rows,
        );
        const available = total_rows - start_row;
        const row_count: u16 = @intCast(@min(@as(u64, bounded_request_rows), available));
        if (row_count == 0) return error.InvalidRequest;
        const start_pin = screen.pages.pin(.{ .screen = .{
            .x = 0,
            .y = @intCast(start_row),
        } }) orelse return error.InvalidPoint;

        const hard_limit = @min(request.maximum_bytes, max_row_window_bytes);
        const parameters: row_encoding.Parameters = .{
            .cell_pixel_height = cell_pixel_height,
            .cell_pixel_width = cell_pixel_width,
            .hard_limit = hard_limit,
            .request = .{
                .attachment_id = request.attachment_id,
                .detail = request.detail,
                .generation = current_generation,
                .host_epoch = request.host_epoch,
                .request_sequence = request.request_sequence,
                .revision = current_revision,
            },
            .row_count = row_count,
            .start_pin = start_pin,
            .start_row = start_row,
            .total_rows = total_rows,
        };
        const encoded = switch (target) {
            .allocated => |allocator| try row_encoding.encode(allocator, terminal, parameters),
            .fixed => |fixed| try row_encoding.encodeInto(
                fixed.scratch_allocator,
                terminal,
                parameters,
                fixed.destination,
            ),
        };
        errdefer switch (target) {
            .allocated => |allocator| allocator.free(encoded),
            .fixed => {},
        };

        if (request.purpose == .viewport) {
            const tracked = try screen.pages.trackPin(start_pin);
            state.pending = .{
                .generation = current_generation,
                .pin = tracked,
                .request_sequence = request.request_sequence,
                .revision = current_revision,
                .screen_generation = terminal.screens.generation(terminal.screens.active_key),
                .screen_key = terminal.screens.active_key,
            };
        }
        state.last_row_request_sequence = request.request_sequence;
        return .{
            .bytes = encoded,
            .generation = current_generation,
            .revision = current_revision,
            .row_count = row_count,
            .sequence = request.request_sequence,
            .start_row = start_row,
            .total_rows = total_rows,
        };
    }

    pub fn rowsApplied(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        request_sequence: u64,
        revision: u64,
        generation: u64,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        // A replay of the already-applied rows is an idempotent no-op even
        // while a newer row window is pending; it never clears or advances the
        // newer flight. The immutable acknowledgement tuple is preserved
        // separately from the mutable committed anchor, whose revision and
        // generation may be refreshed by later mutations.
        if (state.committed_ack) |ack| {
            if (ack.sequence == request_sequence and ack.revision == revision and
                ack.generation == generation)
            {
                return;
            }
        }
        const pending = state.pending orelse return error.InvalidAcknowledgement;
        if (pending.request_sequence != request_sequence or pending.revision != revision or
            pending.generation != generation)
        {
            return error.InvalidAcknowledgement;
        }
        if (!anchorValid(terminal, pending)) {
            self.releaseAnchor(terminal, &state.pending);
            return error.AnchorEvicted;
        }
        self.releaseAnchor(terminal, &state.committed);
        state.committed = pending;
        state.committed_ack = .{
            .generation = generation,
            .revision = revision,
            .sequence = request_sequence,
        };
        state.pending = null;
    }

    pub fn noteMutation(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        generation: u64,
    ) void {
        if (self.viewing_count == 0 and self.selection_count == 0) return;
        for (self.states.items) |*state| {
            if (state.demand == .viewing_history) state.history_dirty = true;
            if (state.selection_state.noteMutation(terminal)) {
                self.selection_count -= 1;
                self.invalidateExport(state);
            }
            if (state.committed) |*anchor| {
                anchor.revision = revision;
                anchor.generation = generation;
            }
        }
    }

    pub fn generationChanged(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        generation: u64,
    ) void {
        for (self.states.items) |*state| {
            self.releaseAnchor(terminal, &state.pending);
            state.history_in_flight = null;

            if (state.committed) |*anchor| {
                if (anchorValid(terminal, anchor.*)) {
                    anchor.revision = revision;
                    anchor.generation = generation;
                } else {
                    self.releaseAnchor(terminal, &state.committed);
                }
            }

            const selection_was_active = state.selection_state.active();
            state.selection_state.generationChanged(terminal);
            self.updateSelectionCount(selection_was_active, state.selection_state.active());
            self.invalidateExport(state);

            if (state.demand == .viewing_history) state.history_dirty = true;
        }
    }

    pub fn screenChanged(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
    ) void {
        for (self.states.items) |*state| {
            self.releaseAnchor(terminal, &state.committed);
            self.releaseAnchor(terminal, &state.pending);
            const selection_was_active = state.selection_state.active();
            state.selection_state.screenChanged(terminal);
            self.updateSelectionCount(selection_was_active, state.selection_state.active());
            self.invalidateExport(state);
            if (state.demand == .viewing_history) {
                state.history_in_flight = null;
                state.history_dirty = true;
            }
        }
    }

    pub fn takeHistoryEvent(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        revision: u64,
        generation: u64,
    ) HistoryError!?HistoryEvent {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.demand != .viewing_history or !state.history_dirty or
            state.history_in_flight != null or state.pending != null)
        {
            return null;
        }
        const anchor_status: AnchorStatus = if (state.committed) |anchor|
            if (anchorValid(terminal, anchor)) .valid else .evicted
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
        self: *Manager,
        attachment_id: AttachmentId,
        expected: HistoryEvent,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const event = state.history_in_flight orelse {
            // A concurrent screen change invalidates the in-flight event and
            // marks history dirty again, which already has rollback semantics.
            if (state.history_dirty) return;
            return error.InvalidAcknowledgement;
        };
        if (!sameHistoryEvent(event, expected)) return error.InvalidAcknowledgement;
        state.history_in_flight = null;
        state.history_dirty = true;
    }

    pub fn historyObserved(
        self: *Manager,
        attachment_id: AttachmentId,
        sequence: u64,
        revision: u64,
        generation: u64,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
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
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?SelectionSnapshot {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.selection_state.takeEvent();
    }

    pub fn rollbackSelectionEvent(
        self: *Manager,
        attachment_id: AttachmentId,
        event: SelectionSnapshot,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        // A newer runtime event supersedes the unsent snapshot. Otherwise put
        // the exact event back so the next scheduler pass can retry it.
        state.selection_state.rollbackEvent(event);
    }

    pub fn takeSelectionExportCancellation(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?ExportCancellation {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.export_ledger.takeCancellation();
    }

    /// Prepare the exact FIFO head without consuming it. Callers commit this
    /// token only after its corresponding event is part of a complete outbound
    /// batch, so every earlier fallible step leaves the cancellation retryable.
    pub fn prepareSelectionExportCancellation(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?PreparedExportCancellation {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.export_ledger.prepareCancellation();
    }

    pub fn commitPreparedSelectionExportCancellation(
        self: *Manager,
        prepared: PreparedExportCancellation,
    ) void {
        const expected = prepared.cancellation;
        const state = self.find(expected.attachment_id) orelse unreachable;
        state.export_ledger.commitPreparedCancellation(prepared);
    }

    pub fn snapshotInvalidatedExportHistory(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!ExportInvalidationSnapshot {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.export_ledger.snapshotInvalidations();
    }

    pub fn restoreInvalidatedExportHistory(
        self: *Manager,
        attachment_id: AttachmentId,
        snapshot: ExportInvalidationSnapshot,
    ) void {
        const state = self.find(attachment_id) orelse return;
        state.export_ledger.restoreInvalidations(snapshot);
    }

    pub fn restoreSelectionExportCancellations(
        self: *Manager,
        attachment_id: AttachmentId,
        cancellations: []const ExportCancellation,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        try state.export_ledger.restoreCancellations(cancellations);
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
        const state = self.find(attachment_id) orelse return;
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
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: SelectionGesture,
    ) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
        try terminal_selection.validateCurrent(
            request.revision,
            request.generation,
            current_revision,
            current_generation,
        );
        const state = self.find(request.attachment_id) orelse return error.MissingAttachment;
        const selection_was_active = state.selection_state.active();
        const snapshot = try state.selection_state.applyGesture(
            terminal,
            current_revision,
            current_generation,
            request,
        );
        self.updateSelectionCount(selection_was_active, state.selection_state.active());
        return snapshot;
    }

    pub fn applySelectionCommand(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: SelectionCommandRequest,
    ) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
        try terminal_selection.validateCurrent(
            request.revision,
            request.generation,
            current_revision,
            current_generation,
        );
        const state = self.find(request.attachment_id) orelse return error.MissingAttachment;
        const selection_was_active = state.selection_state.active();
        const snapshot = try state.selection_state.applyCommand(
            terminal,
            current_revision,
            current_generation,
            request,
        );
        self.updateSelectionCount(selection_was_active, state.selection_state.active());
        return snapshot;
    }

    pub fn startExport(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: ExportFormat,
    ) (HistoryError || std.mem.Allocator.Error)!ExportStarted {
        if (export_sequence == 0) return error.InvalidRequest;
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        try state.export_ledger.validateStart(export_sequence);
        const selection = try self.exportSelection(terminal, state);
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
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: ExportFormat,
    ) (HistoryError || std.mem.Allocator.Error)!ExportStartTransaction {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        try state.export_ledger.validateStart(export_sequence);
        const selection = try self.exportSelection(terminal, state);
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
        self: *Manager,
        output_allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
    ) (HistoryError || std.mem.Allocator.Error)!?EncodedExportChunk {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.export_ledger.encodeNextChunk(output_allocator, attachment_id);
    }

    pub fn rollbackExportChunk(
        self: *Manager,
        expected: ExportChunkToken,
    ) HistoryError!void {
        const state = self.find(expected.attachment_id) orelse return error.MissingAttachment;
        try state.export_ledger.rollbackChunk(expected);
    }

    pub fn exportChunkApplied(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
        chunk_sequence: u32,
    ) HistoryError!?ExportFinished {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.export_ledger.chunkApplied(
            self.allocator,
            attachment_id,
            export_sequence,
            chunk_sequence,
        );
    }

    pub fn cancelExport(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
    ) HistoryError!ExportFinished {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        return state.export_ledger.cancel(self.allocator, attachment_id, export_sequence);
    }

    fn resolveStart(
        self: *Manager,
        screen: *ghostty_vt.Screen,
        state: *AttachmentState,
        origin: Origin,
        row_count: u16,
    ) HistoryError!u64 {
        _ = self;
        const total: u64 = @intCast(screen.pages.total_rows);
        return switch (origin) {
            .absolute => |row| if (row < total) row else error.InvalidPoint,
            .oldest => |offset| if (offset < total) offset else error.InvalidPoint,
            .tail => |rows_before| blk: {
                const end = total - @min(total, @as(u64, rows_before));
                break :blk end - @min(end, @as(u64, row_count));
            },
            .committed => |delta| blk: {
                const anchor = state.committed orelse return error.InvalidRequest;
                if (!anchorValidScreen(screen, anchor)) return error.AnchorEvicted;
                const point = screen.pages.pointFromPin(.screen, anchor.pin.*) orelse
                    return error.AnchorEvicted;
                const base: i64 = point.screen.y;
                const candidate = std.math.add(i64, base, delta) catch
                    if (delta < 0) @as(i64, 0) else std.math.maxInt(i64);
                break :blk @intCast(@min(@max(candidate, 0), @as(i64, @intCast(total - 1))));
            },
        };
    }

    fn exportSelection(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
    ) HistoryError!ghostty_vt.Selection {
        const selection_was_active = state.selection_state.active();
        defer self.updateSelectionCount(
            selection_was_active,
            state.selection_state.active(),
        );
        return state.selection_state.exportSelection(terminal);
    }

    fn clearState(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
    ) void {
        self.releaseAnchor(terminal, &state.committed);
        self.releaseAnchor(terminal, &state.pending);
        const selection_was_active = state.selection_state.active();
        state.selection_state.clear(terminal);
        self.updateSelectionCount(selection_was_active, false);
        state.export_ledger.deinit(self.allocator);
    }

    fn releaseAnchor(
        self: *Manager,
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

    fn updateSelectionCount(self: *Manager, was_active: bool, is_active: bool) void {
        if (was_active == is_active) return;
        if (is_active) {
            self.selection_count += 1;
        } else {
            self.selection_count -= 1;
        }
    }

    fn invalidateExport(self: *Manager, state: *AttachmentState) void {
        state.export_ledger.invalidate(self.allocator);
    }

    fn find(self: *Manager, attachment_id: AttachmentId) ?*AttachmentState {
        const index = self.findIndex(attachment_id) orelse return null;
        return &self.states.items[index];
    }

    fn findIndex(self: *const Manager, attachment_id: AttachmentId) ?usize {
        for (self.states.items, 0..) |state, index| {
            if (state.attachment_id.eql(attachment_id)) return index;
        }
        return null;
    }
};

fn anchorValid(terminal: *ghostty_vt.Terminal, anchor: TrackedAnchor) bool {
    if (terminal.screens.generation(anchor.screen_key) != anchor.screen_generation) return false;
    const screen = terminal.screens.get(anchor.screen_key) orelse return false;
    return anchorValidScreen(screen, anchor);
}

fn anchorValidScreen(screen: *ghostty_vt.Screen, anchor: TrackedAnchor) bool {
    return !anchor.pin.garbage and screen.pages.pointFromPin(.screen, anchor.pin.*) != null;
}

fn sameHistoryEvent(left: HistoryEvent, right: HistoryEvent) bool {
    return left.anchor_status == right.anchor_status and
        left.attachment_id.eql(right.attachment_id) and
        left.generation == right.generation and
        left.revision == right.revision and
        left.sequence == right.sequence and
        left.total_rows == right.total_rows;
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const findStateFn = Manager.find;
    pub const row_window_byte_limit = max_row_window_bytes;
} else struct {};
