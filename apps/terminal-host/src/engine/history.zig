const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const row_encoding = @import("row_encoding.zig");

const max_row_window_bytes = row_encoding.max_bytes;
pub const max_row_window_rows = protocol.Limits.row_window_rows;
const max_selection_export_bytes = protocol.Limits.selection_export_bytes;
const max_selection_export_chunk_bytes = protocol.Limits.selection_export_chunk_bytes;
const row_window_header_bytes = row_encoding.header_bytes;
const selection_chunk_header_bytes = 32;
const max_row_window_entries = row_encoding.max_entries;
// One queued cancellation may precede a newer export; invalidating that newer
// export produces the only second entry before the queue must be drained.
pub const max_pending_export_cancellations = 2;

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
    total_rows: u64,
};

pub const SelectionPoint = struct {
    column: u16,
    row: u64,
};

pub const SelectionGestureAction = enum {
    start,
    update,
    end,
};

pub const SelectionGesture = struct {
    action: SelectionGestureAction,
    attachment_id: AttachmentId,
    click_count: u2,
    generation: u64,
    point: SelectionPoint,
    rectangle: bool,
    revision: u64,
};

pub const SelectionCommand = union(enum) {
    clear,
    select_all,
    set_range: struct {
        end: SelectionPoint,
        rectangle: bool,
        start: SelectionPoint,
    },
    select_output: SelectionPoint,
};

pub const SelectionCommandRequest = struct {
    attachment_id: AttachmentId,
    command: SelectionCommand,
    generation: u64,
    revision: u64,
};

pub const SelectionSnapshot = union(enum) {
    active: struct {
        end: SelectionPoint,
        rectangle: bool,
        start: SelectionPoint,
    },
    evicted,
    none,
};

pub const ExportFormat = enum {
    html,
    plain,
    vt,
};

pub const ExportStarted = struct {
    attachment_id: AttachmentId,
    byte_length: usize,
    export_sequence: u64,
    format: ExportFormat,
};

pub const ExportStartTransaction = struct {
    previous_completed_export_sequence: ?u64,
    previous_last_export_sequence: u64,
    started: ExportStarted,
};

pub const ExportFinished = struct {
    attachment_id: AttachmentId,
    byte_length: usize,
    export_sequence: u64,
};

pub const ExportCancellation = struct {
    attachment_id: AttachmentId,
    byte_length: usize,
    export_sequence: u64,
};

pub const PreparedExportCancellation = struct {
    cancellation: ExportCancellation,
};

pub const ExportChunkToken = struct {
    attachment_id: AttachmentId,
    chunk_sequence: u32,
    export_sequence: u64,
};

pub const EncodedExportChunk = struct {
    bytes: []u8,
    token: ExportChunkToken,
};

pub const ExportInvalidationSnapshot = struct {
    count: usize,
    entries: [max_pending_export_cancellations]ExportCancellation,
};

pub const HistoryError = error{
    AnchorEvicted,
    AttachmentConflict,
    ExportConflict,
    ExportTooLarge,
    InvalidAcknowledgement,
    InvalidPoint,
    InvalidRequest,
    MissingAttachment,
    MissingSelection,
    ProjectionTooLarge,
    StaleGeneration,
    StaleRevision,
};

const TrackedAnchor = struct {
    generation: u64,
    pin: *ghostty_vt.Pin,
    request_sequence: u64,
    revision: u64,
    screen_generation: usize,
    screen_key: ghostty_vt.ScreenSet.Key,
};

const ExportState = struct {
    attachment_id: AttachmentId,
    bytes_length: usize,
    bytes_offset: usize,
    chunk_in_flight: bool = false,
    chunk_length: usize = 0,
    chunk_sequence: u32 = 0,
    export_sequence: u64,
    format: ExportFormat,
    next_offset: usize = 0,
    storage: []u8,
};

const AttachmentState = struct {
    attachment_id: AttachmentId,
    committed: ?TrackedAnchor = null,
    demand: HistoryDemand = .following_bottom,
    export_state: ?ExportState = null,
    gesture_anchor: ?TrackedAnchor = null,
    gesture_click_count: u2 = 1,
    gesture_dragged: bool = false,
    history_dirty: bool = false,
    history_in_flight: ?HistoryEvent = null,
    last_completed_export_sequence: ?u64 = null,
    last_export_sequence: u64 = 0,
    last_history_observation: ?HistoryEvent = null,
    invalidated_exports: [max_pending_export_cancellations]ExportCancellation = undefined,
    invalidated_export_count: usize = 0,
    last_row_request_sequence: u64 = 0,
    pending: ?TrackedAnchor = null,
    pending_export_cancellations: [max_pending_export_cancellations]ExportCancellation = undefined,
    pending_export_cancellation_count: usize = 0,
    selection: ?ghostty_vt.Selection = null,
    selection_event: ?SelectionSnapshot = null,
    selection_screen_generation: usize = 0,
    selection_screen_key: ghostty_vt.ScreenSet.Key = .primary,
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
        if (state.committed) |committed| {
            if (state.pending == null and committed.request_sequence == request_sequence and
                committed.revision == revision and committed.generation == generation)
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
            if (state.selection) |selection| {
                if (selection.start().garbage or selection.end().garbage) {
                    self.releaseSelection(terminal, state);
                    state.selection_event = .evicted;
                    self.invalidateExport(state);
                }
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
            self.releaseGesture(terminal, state);
            state.history_in_flight = null;

            if (state.committed) |*anchor| {
                if (anchorValid(terminal, anchor.*)) {
                    anchor.revision = revision;
                    anchor.generation = generation;
                } else {
                    self.releaseAnchor(terminal, &state.committed);
                }
            }

            if (state.selection != null) {
                const snapshot = self.selectionSnapshot(terminal, state) catch .evicted;
                switch (snapshot) {
                    .active => state.selection_event = snapshot,
                    .evicted, .none => {
                        self.releaseSelection(terminal, state);
                        state.selection_event = .evicted;
                        self.invalidateExport(state);
                    },
                }
            }
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
            self.releaseGesture(terminal, state);
            const selection_was_active = state.selection != null;
            self.releaseSelection(terminal, state);
            if (selection_was_active) state.selection_event = .evicted;
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
            .total_rows = @intCast(terminal.screens.active.pages.total_rows),
        };
        state.history_dirty = false;
        state.history_in_flight = event;
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
        revision: u64,
        generation: u64,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.last_history_observation) |observed| {
            if (observed.revision == revision and observed.generation == generation) return;
        }
        const event = state.history_in_flight orelse return error.InvalidAcknowledgement;
        if (event.revision != revision or event.generation != generation) {
            return error.InvalidAcknowledgement;
        }
        state.history_in_flight = null;
        state.last_history_observation = event;
    }

    pub fn takeSelectionEvent(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?SelectionSnapshot {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const event = state.selection_event;
        state.selection_event = null;
        return event;
    }

    pub fn rollbackSelectionEvent(
        self: *Manager,
        attachment_id: AttachmentId,
        event: SelectionSnapshot,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        // A newer runtime event supersedes the unsent snapshot. Otherwise put
        // the exact event back so the next scheduler pass can retry it.
        if (state.selection_event == null) state.selection_event = event;
    }

    pub fn takeSelectionExportCancellation(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?ExportCancellation {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.pending_export_cancellation_count == 0) return null;
        const cancellation = state.pending_export_cancellations[0];
        if (state.pending_export_cancellation_count > 1) {
            std.mem.copyForwards(
                ExportCancellation,
                state.pending_export_cancellations[0 .. state.pending_export_cancellation_count - 1],
                state.pending_export_cancellations[1..state.pending_export_cancellation_count],
            );
        }
        state.pending_export_cancellation_count -= 1;
        return cancellation;
    }

    /// Prepare the exact FIFO head without consuming it. Callers commit this
    /// token only after its corresponding event is part of a complete outbound
    /// batch, so every earlier fallible step leaves the cancellation retryable.
    pub fn prepareSelectionExportCancellation(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!?PreparedExportCancellation {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.pending_export_cancellation_count == 0) return null;
        return .{ .cancellation = state.pending_export_cancellations[0] };
    }

    pub fn commitPreparedSelectionExportCancellation(
        self: *Manager,
        prepared: PreparedExportCancellation,
    ) void {
        const expected = prepared.cancellation;
        const state = self.find(expected.attachment_id) orelse unreachable;
        std.debug.assert(state.pending_export_cancellation_count > 0);
        std.debug.assert(self.sameCancellation(
            state.pending_export_cancellations[0],
            expected,
        ));
        if (state.pending_export_cancellation_count > 1) {
            std.mem.copyForwards(
                ExportCancellation,
                state.pending_export_cancellations[0 .. state.pending_export_cancellation_count - 1],
                state.pending_export_cancellations[1..state.pending_export_cancellation_count],
            );
        }
        state.pending_export_cancellation_count -= 1;
    }

    pub fn snapshotInvalidatedExportHistory(
        self: *Manager,
        attachment_id: AttachmentId,
    ) HistoryError!ExportInvalidationSnapshot {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        var snapshot = std.mem.zeroes(ExportInvalidationSnapshot);
        snapshot.count = state.invalidated_export_count;
        @memcpy(
            snapshot.entries[0..snapshot.count],
            state.invalidated_exports[0..snapshot.count],
        );
        return snapshot;
    }

    pub fn restoreInvalidatedExportHistory(
        self: *Manager,
        attachment_id: AttachmentId,
        snapshot: ExportInvalidationSnapshot,
    ) void {
        const state = self.find(attachment_id) orelse return;
        std.debug.assert(snapshot.count <= max_pending_export_cancellations);
        @memcpy(
            state.invalidated_exports[0..snapshot.count],
            snapshot.entries[0..snapshot.count],
        );
        state.invalidated_export_count = snapshot.count;
    }

    pub fn restoreSelectionExportCancellations(
        self: *Manager,
        attachment_id: AttachmentId,
        cancellations: []const ExportCancellation,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.pending_export_cancellation_count != 0 or
            cancellations.len > max_pending_export_cancellations)
        {
            return error.InvalidRequest;
        }
        @memcpy(
            state.pending_export_cancellations[0..cancellations.len],
            cancellations,
        );
        state.pending_export_cancellation_count = cancellations.len;
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
        std.debug.assert(cancellations.len <= max_pending_export_cancellations);
        if (previous_last_export_sequence >= export_sequence or
            state.last_export_sequence != export_sequence)
        {
            std.debug.assert(false);
            return;
        }
        if (state.export_state) |export_state| {
            if (export_state.export_sequence != export_sequence or
                state.pending_export_cancellation_count != 0)
            {
                std.debug.assert(false);
                return;
            }
            self.releaseExport(state);
        } else if (state.pending_export_cancellation_count == 1 and
            self.sameExport(state.pending_export_cancellations[0], attachment_id, export_sequence))
        {
            // The reader may have invalidated the unannounced replacement while
            // HostService was building its response. That cancellation is not part
            // of the pre-start FIFO and must not escape the failed handoff.
            state.pending_export_cancellation_count = 0;
        } else {
            std.debug.assert(false);
            return;
        }
        std.debug.assert(state.pending_export_cancellation_count == 0);
        @memcpy(
            state.pending_export_cancellations[0..cancellations.len],
            cancellations,
        );
        state.pending_export_cancellation_count = cancellations.len;
        state.last_completed_export_sequence = previous_completed_export_sequence;
        state.last_export_sequence = previous_last_export_sequence;
        self.restoreInvalidatedExportHistory(attachment_id, invalidation_snapshot);
    }

    pub fn applyGesture(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: SelectionGesture,
    ) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
        try requireCurrent(request.revision, request.generation, current_revision, current_generation);
        const state = self.find(request.attachment_id) orelse return error.MissingAttachment;
        if (request.click_count == 0) return error.InvalidRequest;
        const point_pin = try pointPin(terminal.screens.active, request.point);
        switch (request.action) {
            .start => {
                const initial = switch (request.click_count) {
                    1 => null,
                    2 => terminal.screens.active.selectWord(point_pin, &default_word_boundaries),
                    3 => terminal.screens.active.selectLine(.{ .pin = point_pin }),
                    else => unreachable,
                };
                const tracked = try terminal.screens.active.pages.trackPin(point_pin);
                errdefer terminal.screens.active.pages.untrackPin(tracked);
                try self.replaceSelection(terminal, state, initial);
                self.releaseGesture(terminal, state);
                state.gesture_anchor = .{
                    .generation = current_generation,
                    .pin = tracked,
                    .request_sequence = 0,
                    .revision = current_revision,
                    .screen_generation = terminal.screens.generation(terminal.screens.active_key),
                    .screen_key = terminal.screens.active_key,
                };
                state.gesture_click_count = request.click_count;
                state.gesture_dragged = false;
            },
            .update, .end => {
                const anchor = state.gesture_anchor orelse return error.InvalidRequest;
                if (!anchorValid(terminal, anchor)) {
                    self.releaseGesture(terminal, state);
                    return error.AnchorEvicted;
                }
                if (!point_pin.eql(anchor.pin.*)) state.gesture_dragged = true;
                const selection = selectionForGesture(
                    terminal.screens.active,
                    anchor.pin.*,
                    point_pin,
                    state.gesture_click_count,
                    state.gesture_dragged,
                    request.rectangle,
                );
                try self.replaceSelection(terminal, state, selection);
                if (request.action == .end) self.releaseGesture(terminal, state);
            },
        }
        return self.selectionSnapshot(terminal, state);
    }

    pub fn applySelectionCommand(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        current_revision: u64,
        current_generation: u64,
        request: SelectionCommandRequest,
    ) (HistoryError || std.mem.Allocator.Error)!SelectionSnapshot {
        try requireCurrent(request.revision, request.generation, current_revision, current_generation);
        const state = self.find(request.attachment_id) orelse return error.MissingAttachment;
        const screen = terminal.screens.active;
        const selection: ?ghostty_vt.Selection = switch (request.command) {
            .clear => null,
            .select_all => if (screen.pages.getBottomRight(.screen)) |end|
                .init(screen.pages.getTopLeft(.screen), end, false)
            else
                null,
            .set_range => |range| .init(
                try pointPin(screen, range.start),
                try pointPin(screen, range.end),
                range.rectangle,
            ),
            .select_output => |point| screen.selectOutput(try pointPin(screen, point)),
        };
        try self.replaceSelection(terminal, state, selection);
        self.releaseGesture(terminal, state);
        return self.selectionSnapshot(terminal, state);
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
        if (export_sequence <= state.last_export_sequence) return error.InvalidRequest;
        // A full queue means two invalidated exports are still awaiting delivery;
        // do not admit another export that could create an unbounded backlog.
        if (state.pending_export_cancellation_count == max_pending_export_cancellations) {
            return error.ExportConflict;
        }
        if (state.export_state != null) return error.ExportConflict;
        const selection = state.selection orelse return error.MissingSelection;
        if (selection.start().garbage or selection.end().garbage or
            !selectionScreenValid(terminal, state))
        {
            self.releaseSelection(terminal, state);
            return error.AnchorEvicted;
        }

        const storage = try self.allocator.alloc(u8, max_selection_export_bytes + 1);
        errdefer self.allocator.free(storage);
        var fixed: std.heap.FixedBufferAllocator = .init(storage);
        var writer: std.Io.Writer.Allocating = .init(fixed.allocator());
        defer writer.deinit();
        var formatter: ghostty_vt.formatter.ScreenFormatter = .init(
            terminal.screens.active,
            .{
                .emit = switch (format) {
                    .html => .html,
                    .plain => .plain,
                    .vt => .vt,
                },
                .palette = &terminal.colors.palette.current,
                .trim = true,
                .unwrap = true,
            },
        );
        formatter.content = .{ .selection = selection };
        formatter.format(&writer.writer) catch return error.ExportTooLarge;
        const formatted = writer.toOwnedSliceSentinel(0) catch return error.ExportTooLarge;
        if (formatted.len > max_selection_export_bytes) return error.ExportTooLarge;
        const bytes_offset = @intFromPtr(formatted.ptr) - @intFromPtr(storage.ptr);
        state.export_state = .{
            .attachment_id = attachment_id,
            .bytes_length = formatted.len,
            .bytes_offset = bytes_offset,
            .export_sequence = export_sequence,
            .format = format,
            .storage = storage,
        };
        state.last_completed_export_sequence = null;
        state.last_export_sequence = export_sequence;
        return .{
            .attachment_id = attachment_id,
            .byte_length = formatted.len,
            .export_sequence = export_sequence,
            .format = format,
        };
    }

    pub fn startExportTransaction(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: AttachmentId,
        export_sequence: u64,
        format: ExportFormat,
    ) (HistoryError || std.mem.Allocator.Error)!ExportStartTransaction {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const previous_completed_export_sequence = state.last_completed_export_sequence;
        const previous_last_export_sequence = state.last_export_sequence;
        return .{
            .previous_completed_export_sequence = previous_completed_export_sequence,
            .previous_last_export_sequence = previous_last_export_sequence,
            .started = try self.startExport(
                terminal,
                attachment_id,
                export_sequence,
                format,
            ),
        };
    }

    pub fn encodeNextExportChunk(
        self: *Manager,
        output_allocator: std.mem.Allocator,
        attachment_id: AttachmentId,
    ) (HistoryError || std.mem.Allocator.Error)!?EncodedExportChunk {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const export_state = if (state.export_state) |*value| value else return null;
        const empty_first_chunk = export_state.bytes_length == 0 and export_state.chunk_sequence == 0;
        if (export_state.chunk_in_flight or
            (export_state.next_offset >= export_state.bytes_length and !empty_first_chunk))
        {
            return null;
        }
        const chunk_length = @min(
            max_selection_export_chunk_bytes,
            export_state.bytes_length - export_state.next_offset,
        );
        const final = export_state.next_offset + chunk_length == export_state.bytes_length;
        const bytes = try output_allocator.alloc(u8, selection_chunk_header_bytes + chunk_length);
        errdefer output_allocator.free(bytes);
        @memcpy(bytes[0..16], &attachment_id.bytes);
        std.mem.writeInt(u64, bytes[16..24], export_state.export_sequence, .little);
        std.mem.writeInt(u32, bytes[24..28], export_state.chunk_sequence, .little);
        bytes[28] = if (final) 1 else 0;
        @memset(bytes[29..32], 0);
        const source_start = export_state.bytes_offset + export_state.next_offset;
        @memcpy(
            bytes[selection_chunk_header_bytes..],
            export_state.storage[source_start..][0..chunk_length],
        );
        export_state.chunk_in_flight = true;
        export_state.chunk_length = chunk_length;
        return .{
            .bytes = bytes,
            .token = .{
                .attachment_id = attachment_id,
                .chunk_sequence = export_state.chunk_sequence,
                .export_sequence = export_state.export_sequence,
            },
        };
    }

    pub fn rollbackExportChunk(
        self: *Manager,
        expected: ExportChunkToken,
    ) HistoryError!void {
        const state = self.find(expected.attachment_id) orelse return error.MissingAttachment;
        // Runtime output may invalidate the export after the scheduler encoded
        // this chunk. Cancellation state then already supersedes the chunk.
        const export_state = if (state.export_state) |*value| value else return;
        if (!export_state.chunk_in_flight or
            export_state.export_sequence != expected.export_sequence or
            export_state.chunk_sequence != expected.chunk_sequence)
        {
            return error.InvalidAcknowledgement;
        }
        export_state.chunk_in_flight = false;
        export_state.chunk_length = 0;
    }

    pub fn exportChunkApplied(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
        chunk_sequence: u32,
    ) HistoryError!?ExportFinished {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (export_sequence == 0) return error.InvalidAcknowledgement;
        for (state.invalidated_exports[0..state.invalidated_export_count]) |cancellation| {
            if (self.sameExport(cancellation, attachment_id, export_sequence)) return null;
        }
        const export_state = if (state.export_state) |*value| value else {
            const completed = state.last_completed_export_sequence orelse
                return error.InvalidAcknowledgement;
            return if (completed == export_sequence)
                null
            else
                error.InvalidAcknowledgement;
        };
        if (export_sequence != export_state.export_sequence) return error.InvalidAcknowledgement;
        if (chunk_sequence < export_state.chunk_sequence) return null;
        if (!export_state.chunk_in_flight or chunk_sequence != export_state.chunk_sequence) {
            return error.InvalidAcknowledgement;
        }
        const next_offset = export_state.next_offset + export_state.chunk_length;
        const finished_export = next_offset >= export_state.bytes_length;
        const next_chunk_sequence = if (finished_export)
            export_state.chunk_sequence
        else
            std.math.add(u32, export_state.chunk_sequence, 1) catch
                return error.InvalidAcknowledgement;
        export_state.next_offset = next_offset;
        export_state.chunk_length = 0;
        export_state.chunk_in_flight = false;
        export_state.chunk_sequence = next_chunk_sequence;
        if (!finished_export) return null;
        const finished: ExportFinished = .{
            .attachment_id = attachment_id,
            .byte_length = export_state.bytes_length,
            .export_sequence = export_sequence,
        };
        state.last_completed_export_sequence = export_sequence;
        self.releaseExport(state);
        return finished;
    }

    pub fn cancelExport(
        self: *Manager,
        attachment_id: AttachmentId,
        export_sequence: u64,
    ) HistoryError!ExportFinished {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const export_state = state.export_state orelse return error.InvalidAcknowledgement;
        if (export_state.export_sequence != export_sequence) return error.InvalidAcknowledgement;
        const finished: ExportFinished = .{
            .attachment_id = attachment_id,
            .byte_length = export_state.bytes_length,
            .export_sequence = export_sequence,
        };
        state.last_completed_export_sequence = null;
        self.releaseExport(state);
        return finished;
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

    fn selectionSnapshot(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
    ) HistoryError!SelectionSnapshot {
        _ = self;
        const selection = state.selection orelse return .none;
        if (!selectionScreenValid(terminal, state) or selection.start().garbage or
            selection.end().garbage)
        {
            return .evicted;
        }
        const screen = terminal.screens.get(state.selection_screen_key) orelse return .evicted;
        const start = screen.pages.pointFromPin(.screen, selection.start()) orelse return .evicted;
        const end = screen.pages.pointFromPin(.screen, selection.end()) orelse return .evicted;
        return .{ .active = .{
            .end = .{ .column = end.screen.x, .row = end.screen.y },
            .rectangle = selection.rectangle,
            .start = .{ .column = start.screen.x, .row = start.screen.y },
        } };
    }

    fn replaceSelection(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
        selection: ?ghostty_vt.Selection,
    ) std.mem.Allocator.Error!void {
        if (selection) |untracked| {
            const tracked = try untracked.track(terminal.screens.active);
            self.releaseSelection(terminal, state);
            state.selection_event = null;
            state.selection = tracked;
            self.selection_count += 1;
            state.selection_screen_key = terminal.screens.active_key;
            state.selection_screen_generation = terminal.screens.generation(terminal.screens.active_key);
        } else {
            self.releaseSelection(terminal, state);
            state.selection_event = null;
        }
    }

    fn clearState(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
    ) void {
        self.releaseAnchor(terminal, &state.committed);
        self.releaseAnchor(terminal, &state.pending);
        self.releaseGesture(terminal, state);
        self.releaseSelection(terminal, state);
        self.releaseExport(state);
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

    fn releaseGesture(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
    ) void {
        self.releaseAnchor(terminal, &state.gesture_anchor);
        state.gesture_click_count = 1;
        state.gesture_dragged = false;
    }

    fn releaseSelection(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        state: *AttachmentState,
    ) void {
        const selection = state.selection orelse return;
        state.selection = null;
        self.selection_count -= 1;
        if (!selectionScreenValid(terminal, state)) return;
        const screen = terminal.screens.get(state.selection_screen_key) orelse return;
        selection.deinit(screen);
    }

    fn invalidateExport(self: *Manager, state: *AttachmentState) void {
        const export_state = state.export_state orelse return;
        const cancellation: ExportCancellation = .{
            .attachment_id = export_state.attachment_id,
            .byte_length = export_state.bytes_length,
            .export_sequence = export_state.export_sequence,
        };
        std.debug.assert(
            state.pending_export_cancellation_count < max_pending_export_cancellations,
        );
        self.rememberInvalidatedExport(state, cancellation);
        state.pending_export_cancellations[state.pending_export_cancellation_count] = cancellation;
        state.pending_export_cancellation_count += 1;
        self.releaseExport(state);
    }

    fn rememberInvalidatedExport(
        self: *Manager,
        state: *AttachmentState,
        cancellation: ExportCancellation,
    ) void {
        _ = self;
        if (state.invalidated_export_count < max_pending_export_cancellations) {
            state.invalidated_exports[state.invalidated_export_count] = cancellation;
            state.invalidated_export_count += 1;
            return;
        }
        for (1..max_pending_export_cancellations) |index| {
            state.invalidated_exports[index - 1] = state.invalidated_exports[index];
        }
        state.invalidated_exports[max_pending_export_cancellations - 1] = cancellation;
    }

    fn sameExport(
        self: *Manager,
        cancellation: ExportCancellation,
        attachment_id: AttachmentId,
        export_sequence: u64,
    ) bool {
        _ = self;
        return cancellation.export_sequence == export_sequence and
            cancellation.attachment_id.eql(attachment_id);
    }

    fn sameCancellation(
        self: *Manager,
        left: ExportCancellation,
        right: ExportCancellation,
    ) bool {
        _ = self;
        return left.byte_length == right.byte_length and
            left.export_sequence == right.export_sequence and
            left.attachment_id.eql(right.attachment_id);
    }

    fn releaseExport(self: *Manager, state: *AttachmentState) void {
        if (state.export_state) |export_state| self.allocator.free(export_state.storage);
        state.export_state = null;
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

const default_word_boundaries = [_]u21{
    0, ' ', '\t', '\'', '"', '│', '`', '|', ':', ';', ',', '(', ')', '[', ']', '{', '}', '<', '>', '$',
};

fn requireCurrent(
    revision: u64,
    generation: u64,
    current_revision: u64,
    current_generation: u64,
) HistoryError!void {
    if (revision != current_revision) return error.StaleRevision;
    if (generation != current_generation) return error.StaleGeneration;
}

fn pointPin(screen: *ghostty_vt.Screen, point: SelectionPoint) HistoryError!ghostty_vt.Pin {
    if (point.row > std.math.maxInt(u32)) return error.InvalidPoint;
    return screen.pages.pin(.{ .screen = .{
        .x = point.column,
        .y = @intCast(point.row),
    } }) orelse error.InvalidPoint;
}

fn anchorValid(terminal: *ghostty_vt.Terminal, anchor: TrackedAnchor) bool {
    if (terminal.screens.generation(anchor.screen_key) != anchor.screen_generation) return false;
    const screen = terminal.screens.get(anchor.screen_key) orelse return false;
    return anchorValidScreen(screen, anchor);
}

fn anchorValidScreen(screen: *ghostty_vt.Screen, anchor: TrackedAnchor) bool {
    return !anchor.pin.garbage and screen.pages.pointFromPin(.screen, anchor.pin.*) != null;
}

fn selectionScreenValid(terminal: *ghostty_vt.Terminal, state: *const AttachmentState) bool {
    return terminal.screens.generation(state.selection_screen_key) == state.selection_screen_generation and
        terminal.screens.get(state.selection_screen_key) != null;
}

fn selectionForGesture(
    screen: *ghostty_vt.Screen,
    anchor: ghostty_vt.Pin,
    current: ghostty_vt.Pin,
    click_count: u2,
    dragged: bool,
    rectangle: bool,
) ?ghostty_vt.Selection {
    if (rectangle or click_count == 1) {
        if (!dragged) return null;
        return .init(anchor, current, rectangle);
    }
    if (click_count == 2) {
        const first = screen.selectWord(anchor, &default_word_boundaries) orelse return null;
        const last = screen.selectWord(current, &default_word_boundaries) orelse return first;
        return if (current.before(anchor))
            .init(last.start(), first.end(), false)
        else
            .init(first.start(), last.end(), false);
    }
    if (click_count == 3) {
        const first = screen.selectLine(.{ .pin = anchor }) orelse return null;
        const last = screen.selectLine(.{ .pin = current }) orelse return first;
        return if (current.before(anchor))
            .init(last.start(), first.end(), false)
        else
            .init(first.start(), last.end(), false);
    }
    return null;
}

fn sameHistoryEvent(left: HistoryEvent, right: HistoryEvent) bool {
    return left.anchor_status == right.anchor_status and
        left.attachment_id.eql(right.attachment_id) and
        left.generation == right.generation and
        left.revision == right.revision and
        left.total_rows == right.total_rows;
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const findStateFn = Manager.find;
    pub const row_window_byte_limit = max_row_window_bytes;
} else struct {};
