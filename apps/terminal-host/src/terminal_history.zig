const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const projection = @import("terminal_projection.zig");

pub const max_row_window_bytes = 2 * 1024 * 1024;
pub const max_row_window_rows = 4096;
pub const max_selection_export_bytes = 16 * 1024 * 1024;
pub const max_selection_export_chunk_bytes = 1024 * 1024;
const row_window_header_bytes = 112;
const row_capture_bytes = max_row_window_bytes;
const selection_chunk_header_bytes = 32;
const max_row_window_entries = 256 * 1024;

pub const Identifier = [16]u8;

pub const Detail = enum {
    render,
    text,
};

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
    attachment_id: Identifier,
    detail: Detail,
    generation: u64,
    host_epoch: Identifier,
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
    attachment_id: Identifier,
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
    attachment_id: Identifier,
    click_count: u2,
    generation: u64,
    point: SelectionPoint,
    rectangle: bool,
    revision: u64,
};

pub const SelectionCommand = union(enum) {
    clear,
    select_all,
    select_output: SelectionPoint,
};

pub const SelectionCommandRequest = struct {
    attachment_id: Identifier,
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
    attachment_id: Identifier,
    byte_length: usize,
    export_sequence: u64,
    format: ExportFormat,
};

pub const ExportFinished = struct {
    attachment_id: Identifier,
    byte_length: usize,
    export_sequence: u64,
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
    attachment_id: Identifier,
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
    attachment_id: Identifier,
    committed: ?TrackedAnchor = null,
    demand: HistoryDemand = .following_bottom,
    export_state: ?ExportState = null,
    gesture_anchor: ?TrackedAnchor = null,
    gesture_click_count: u2 = 1,
    history_dirty: bool = false,
    history_in_flight: ?HistoryEvent = null,
    pending: ?TrackedAnchor = null,
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

    pub fn attach(self: *Manager, attachment_id: Identifier) !void {
        if (self.find(attachment_id) != null) return;
        try self.states.append(self.allocator, .{ .attachment_id = attachment_id });
    }

    pub fn detach(self: *Manager, terminal: *ghostty_vt.Terminal, attachment_id: Identifier) void {
        const index = self.findIndex(attachment_id) orelse return;
        var state = self.states.orderedRemove(index);
        if (state.demand == .viewing_history) self.viewing_count -= 1;
        self.clearState(terminal, &state);
    }

    pub fn setDemand(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: Identifier,
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
        const encoded = try encodeRows(
            self.allocator,
            terminal,
            start_pin,
            row_count,
            start_row,
            total_rows,
            current_revision,
            current_generation,
            cell_pixel_width,
            cell_pixel_height,
            request,
            hard_limit,
        );
        errdefer self.allocator.free(encoded);

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
        attachment_id: Identifier,
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
            if (state.committed) |*anchor| {
                if (anchorValid(terminal, anchor.*)) {
                    anchor.revision = revision;
                    anchor.generation = generation;
                } else {
                    self.releaseAnchor(terminal, &state.committed);
                }
            }
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
            self.releaseExport(state);
            if (state.demand == .viewing_history) state.history_dirty = true;
        }
    }

    pub fn takeHistoryEvent(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: Identifier,
        revision: u64,
        generation: u64,
    ) HistoryError!?HistoryEvent {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        if (state.demand != .viewing_history or !state.history_dirty or
            state.history_in_flight != null)
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

    pub fn historyObserved(
        self: *Manager,
        attachment_id: Identifier,
        revision: u64,
        generation: u64,
    ) HistoryError!void {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const event = state.history_in_flight orelse return error.InvalidAcknowledgement;
        if (event.revision != revision or event.generation != generation) {
            return error.InvalidAcknowledgement;
        }
        state.history_in_flight = null;
    }

    pub fn takeSelectionEvent(
        self: *Manager,
        attachment_id: Identifier,
    ) HistoryError!?SelectionSnapshot {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const event = state.selection_event;
        state.selection_event = null;
        return event;
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
        const point_pin = try pointPin(terminal.screens.active, request.point);
        switch (request.action) {
            .start => {
                self.releaseGesture(terminal, state);
                const tracked = try terminal.screens.active.pages.trackPin(point_pin);
                state.gesture_anchor = .{
                    .generation = current_generation,
                    .pin = tracked,
                    .request_sequence = 0,
                    .revision = current_revision,
                    .screen_generation = terminal.screens.generation(terminal.screens.active_key),
                    .screen_key = terminal.screens.active_key,
                };
                state.gesture_click_count = request.click_count;
                const initial = switch (request.click_count) {
                    1 => null,
                    2 => terminal.screens.active.selectWord(point_pin, &default_word_boundaries),
                    3 => terminal.screens.active.selectLine(.{ .pin = point_pin }),
                    else => return error.InvalidRequest,
                };
                try self.replaceSelection(terminal, state, initial);
            },
            .update, .end => {
                const anchor = state.gesture_anchor orelse return error.InvalidRequest;
                if (!anchorValid(terminal, anchor)) {
                    self.releaseGesture(terminal, state);
                    return error.AnchorEvicted;
                }
                const selection = selectionForGesture(
                    terminal.screens.active,
                    anchor.pin.*,
                    point_pin,
                    state.gesture_click_count,
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
        self.releaseGesture(terminal, state);
        const screen = terminal.screens.active;
        const selection: ?ghostty_vt.Selection = switch (request.command) {
            .clear => null,
            .select_all => if (screen.pages.getBottomRight(.screen)) |end|
                .init(screen.pages.getTopLeft(.screen), end, false)
            else
                null,
            .select_output => |point| screen.selectOutput(try pointPin(screen, point)),
        };
        try self.replaceSelection(terminal, state, selection);
        return self.selectionSnapshot(terminal, state);
    }

    pub fn startExport(
        self: *Manager,
        terminal: *ghostty_vt.Terminal,
        attachment_id: Identifier,
        export_sequence: u64,
        format: ExportFormat,
    ) (HistoryError || std.mem.Allocator.Error)!ExportStarted {
        if (export_sequence == 0) return error.InvalidRequest;
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
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
        return .{
            .attachment_id = attachment_id,
            .byte_length = formatted.len,
            .export_sequence = export_sequence,
            .format = format,
        };
    }

    pub fn encodeNextExportChunk(
        self: *Manager,
        attachment_id: Identifier,
    ) (HistoryError || std.mem.Allocator.Error)!?[]u8 {
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
        const bytes = try self.allocator.alloc(u8, selection_chunk_header_bytes + chunk_length);
        errdefer self.allocator.free(bytes);
        @memcpy(bytes[0..16], &attachment_id);
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
        return bytes;
    }

    pub fn exportChunkApplied(
        self: *Manager,
        attachment_id: Identifier,
        export_sequence: u64,
        chunk_sequence: u32,
    ) HistoryError!?ExportFinished {
        const state = self.find(attachment_id) orelse return error.MissingAttachment;
        const export_state = if (state.export_state) |*value| value else return error.InvalidAcknowledgement;
        if (!export_state.chunk_in_flight or export_state.export_sequence != export_sequence or
            export_state.chunk_sequence != chunk_sequence)
        {
            return error.InvalidAcknowledgement;
        }
        export_state.next_offset += export_state.chunk_length;
        export_state.chunk_length = 0;
        export_state.chunk_in_flight = false;
        export_state.chunk_sequence = std.math.add(u32, export_state.chunk_sequence, 1) catch
            return error.InvalidAcknowledgement;
        if (export_state.next_offset < export_state.bytes_length) return null;
        const finished: ExportFinished = .{
            .attachment_id = attachment_id,
            .byte_length = export_state.bytes_length,
            .export_sequence = export_sequence,
        };
        self.releaseExport(state);
        return finished;
    }

    pub fn cancelExport(
        self: *Manager,
        attachment_id: Identifier,
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
        state.selection_event = null;
        if (selection) |untracked| {
            const tracked = try untracked.track(terminal.screens.active);
            self.releaseSelection(terminal, state);
            state.selection = tracked;
            self.selection_count += 1;
            state.selection_screen_key = terminal.screens.active_key;
            state.selection_screen_generation = terminal.screens.generation(terminal.screens.active_key);
        } else {
            self.releaseSelection(terminal, state);
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

    fn releaseExport(self: *Manager, state: *AttachmentState) void {
        if (state.export_state) |export_state| self.allocator.free(export_state.storage);
        state.export_state = null;
    }

    fn find(self: *Manager, attachment_id: Identifier) ?*AttachmentState {
        const index = self.findIndex(attachment_id) orelse return null;
        return &self.states.items[index];
    }

    fn findIndex(self: *const Manager, attachment_id: Identifier) ?usize {
        for (self.states.items, 0..) |state, index| {
            if (std.mem.eql(u8, &state.attachment_id, &attachment_id)) return index;
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
    rectangle: bool,
) ?ghostty_vt.Selection {
    if (rectangle or click_count == 1) return .init(anchor, current, rectangle);
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

fn encodeRows(
    allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    start_pin: ghostty_vt.Pin,
    row_count: u16,
    start_row: u64,
    total_rows: u64,
    revision: u64,
    generation: u64,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    request: ReadRequest,
    hard_limit: usize,
) (HistoryError || std.mem.Allocator.Error)![]u8 {
    const capture = try allocator.alloc(u8, row_capture_bytes);
    defer allocator.free(capture);
    var fixed: std.heap.FixedBufferAllocator = .init(capture);
    const capture_allocator = fixed.allocator();
    var pins: std.ArrayList(ghostty_vt.Pin) = .empty;
    defer pins.deinit(capture_allocator);
    try pins.ensureTotalCapacity(capture_allocator, row_count);
    var iterator = start_pin.rowIterator(.right_down, null);
    while (pins.items.len < row_count) {
        const pin = iterator.next() orelse break;
        if (pin.cells(.all).len != terminal.cols) return error.InvalidRequest;
        pins.appendAssumeCapacity(pin);
    }
    if (pins.items.len != row_count) return error.InvalidRequest;

    const body = switch (request.detail) {
        .render => try encodeRenderBody(
            allocator,
            capture_allocator,
            terminal,
            pins.items,
            start_row,
            revision,
            generation,
            cell_pixel_width,
            cell_pixel_height,
            request,
            hard_limit - row_window_header_bytes,
        ),
        .text => try encodeTextBody(allocator, pins.items, hard_limit - row_window_header_bytes),
    };
    defer allocator.free(body.bytes);

    var writer = projection.LimitedWriter.init(allocator, hard_limit);
    errdefer writer.deinit();
    try writer.appendSlice("OBRW");
    try writer.appendInt(u8, 1);
    try writer.appendInt(u8, 1);
    try writer.appendInt(u8, switch (request.detail) {
        .render => 1,
        .text => 2,
    });
    try writer.appendInt(u8, 0);
    try writer.appendInt(u32, @intCast(row_window_header_bytes + body.bytes.len));
    try writer.appendInt(u16, row_window_header_bytes);
    try writer.appendInt(u16, terminal.cols);
    try writer.appendSlice(&request.host_epoch);
    try writer.appendSlice(&request.attachment_id);
    try writer.appendInt(u64, request.request_sequence);
    try writer.appendInt(u64, revision);
    try writer.appendInt(u64, generation);
    try writer.appendInt(u64, total_rows);
    try writer.appendInt(u64, start_row);
    try writer.appendInt(u16, row_count);
    try writer.appendInt(u16, 0);
    try writer.appendInt(u32, body.entry_count);
    try writer.appendInt(u32, @intCast(body.bytes.len));
    try writer.appendZeros(12);
    std.debug.assert(writer.bytes.items.len == row_window_header_bytes);
    try writer.appendSlice(body.bytes);
    return writer.toOwnedSlice();
}

const EncodedBody = struct {
    bytes: []u8,
    entry_count: u32,
};

fn encodeRenderBody(
    allocator: std.mem.Allocator,
    capture_allocator: std.mem.Allocator,
    terminal: *ghostty_vt.Terminal,
    pins: []const ghostty_vt.Pin,
    start_row: u64,
    revision: u64,
    generation: u64,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
    request: ReadRequest,
    hard_limit: usize,
) (HistoryError || std.mem.Allocator.Error)!EncodedBody {
    var strings = projection.StringTable.init(capture_allocator);
    defer strings.deinit();
    var styles = projection.StyleTable.init(capture_allocator);
    defer styles.deinit();
    const title_index = try strings.addOptional(terminal.getTitle());
    const current_directory_index = try strings.addOptional(terminal.getPwd());
    const colors = renderColors(terminal);
    for (pins) |pin| {
        const cells = pin.cells(.all);
        for (cells, 0..) |cell, column| {
            var grapheme_buffer: [projection.max_string_bytes]u8 = undefined;
            const grapheme = projection.cellGrapheme(
                cell,
                pin.grapheme(&cell) orelse &.{},
                &grapheme_buffer,
            ) catch return error.ProjectionTooLarge;
            _ = strings.add(grapheme) catch return error.ProjectionTooLarge;
            const hyperlink_index = strings.addOptional(hyperlinkTarget(pin, cell, column)) catch
                return error.ProjectionTooLarge;
            _ = styles.add(projection.styleRecord(
                cell,
                pin.style(&cell),
                &colors,
                hyperlink_index,
            )) catch return error.ProjectionTooLarge;
        }
    }

    var writer = projection.LimitedWriter.init(allocator, hard_limit);
    errdefer writer.deinit();
    try writer.appendSlice("OBTF");
    try writer.appendInt(u8, 2);
    try writer.appendInt(u8, 0);
    try writer.appendInt(u8, 1);
    try writer.appendInt(u8, 0);
    const total_length_offset = writer.bytes.items.len;
    try writer.appendInt(u32, 0);
    try writer.appendInt(u16, 160);
    try writer.appendInt(u16, 0);
    try writer.appendSlice(&request.host_epoch);
    try writer.appendSlice(&request.attachment_id);
    try writer.appendInt(u64, request.request_sequence);
    try writer.appendInt(u64, projection.no_base_revision);
    try writer.appendInt(u64, revision);
    try writer.appendInt(u64, generation);
    try writer.appendInt(u64, start_row);
    try writer.appendInt(u16, terminal.cols);
    try writer.appendInt(u16, @intCast(pins.len));
    try writer.appendInt(u16, 0);
    try writer.appendInt(u16, 0);
    try writer.appendInt(u8, 1);
    try writer.appendInt(u8, 0);
    try writer.appendInt(u8, if (terminal.screens.active_key == .primary) 1 else 2);
    try writer.appendInt(u8, 0);
    try writer.appendInt(u32, projection.terminalInteraction(terminal));
    try writer.appendInt(u32, projection.rgbCode(colors.foreground));
    try writer.appendInt(u32, projection.rgbCode(colors.background));
    try writer.appendInt(u32, if (colors.cursor) |value|
        projection.rgbCode(value)
    else
        projection.color_default);
    try writer.appendInt(u32, title_index);
    try writer.appendInt(u32, current_directory_index);
    try writer.appendInt(u32, @intCast(strings.values.items.len));
    try writer.appendInt(u16, @intCast(styles.values.items.len));
    try writer.appendInt(u16, @intCast(pins.len));
    try writer.appendInt(u16, 0);
    try writer.appendInt(u16, 0);
    try writer.appendInt(u16, cell_pixel_width);
    try writer.appendInt(u16, cell_pixel_height);
    const entry_count: u32 = @intCast(pins.len * terminal.cols);
    try writer.appendInt(u32, entry_count);
    try writer.appendZeros(16);
    std.debug.assert(writer.bytes.items.len == 160);

    for (strings.values.items) |value| {
        try writer.appendInt(u32, @intCast(value.len));
        try writer.appendSlice(value);
    }
    for (styles.values.items) |style| {
        try writer.appendInt(u32, style.foreground);
        try writer.appendInt(u32, style.background);
        try writer.appendInt(u32, style.underline);
        try writer.appendInt(u32, style.hyperlink_index);
        try writer.appendInt(u16, style.flags);
        try writer.appendInt(u8, style.underline_kind);
        try writer.appendInt(u8, 0);
    }
    for (pins, 0..) |pin, row_index| {
        const row = pin.rowAndCell().row;
        try writer.appendInt(u16, @intCast(row_index));
        try writer.appendInt(u8, (if (row.wrap) @as(u8, 1) else 0) |
            (if (row.wrap_continuation) @as(u8, 2) else 0));
        try writer.appendInt(u8, projection.rowPromptClass(row.semantic_prompt));
        try writer.appendInt(u16, terminal.cols);
        try writer.appendInt(u16, 0);
        for (pin.cells(.all), 0..) |cell, column| {
            var grapheme_buffer: [projection.max_string_bytes]u8 = undefined;
            const grapheme = projection.cellGrapheme(
                cell,
                pin.grapheme(&cell) orelse &.{},
                &grapheme_buffer,
            ) catch return error.ProjectionTooLarge;
            const hyperlink_index = strings.indexOfOptional(hyperlinkTarget(pin, cell, column));
            const style = projection.styleRecord(
                cell,
                pin.style(&cell),
                &colors,
                hyperlink_index,
            );
            try writer.appendInt(u32, strings.indexOf(grapheme).?);
            try writer.appendInt(u16, styles.indexOf(style).?);
            try writer.appendInt(u8, projection.cellWidth(cell.wide) |
                (projection.semanticContent(cell.semantic_content) << 2) |
                (if (cell.protected) @as(u8, 0x10) else 0));
            try writer.appendInt(u8, 0);
        }
    }
    if (writer.bytes.items.len > std.math.maxInt(u32)) return error.ProjectionTooLarge;
    std.mem.writeInt(
        u32,
        writer.bytes.items[total_length_offset..][0..4],
        @intCast(writer.bytes.items.len),
        .little,
    );
    return .{ .bytes = try writer.toOwnedSlice(), .entry_count = entry_count };
}

fn encodeTextBody(
    allocator: std.mem.Allocator,
    pins: []const ghostty_vt.Pin,
    hard_limit: usize,
) (HistoryError || std.mem.Allocator.Error)!EncodedBody {
    var writer = projection.LimitedWriter.init(allocator, hard_limit);
    errdefer writer.deinit();
    var entry_count: u32 = 0;
    for (pins) |pin| {
        const cells = pin.cells(.all);
        const logical_columns = textColumns(cells);
        const identity_ascii = isIdentityAsciiRow(pin, cells[0..logical_columns]);
        var text_bytes: u32 = 0;
        var mapping_count: u32 = 0;
        for (cells[0..logical_columns]) |cell| {
            if (isSpacer(cell)) continue;
            text_bytes = std.math.add(u32, text_bytes, cellTextByteLength(pin, cell)) catch
                return error.ProjectionTooLarge;
            mapping_count += 1;
        }
        const row = pin.rowAndCell().row;
        try writer.appendInt(u8, (if (row.wrap) @as(u8, 1) else 0) |
            (if (row.wrap_continuation) @as(u8, 2) else 0) |
            (if (identity_ascii) @as(u8, 4) else 0));
        try writer.appendInt(u8, projection.rowPromptClass(row.semantic_prompt));
        try writer.appendInt(u16, 0);
        try writer.appendInt(u32, text_bytes);
        try writer.appendInt(u32, if (identity_ascii) 0 else mapping_count);
        for (cells[0..logical_columns]) |cell| {
            if (isSpacer(cell)) continue;
            try appendCellText(&writer, pin, cell);
        }
        if (!identity_ascii) {
            var byte_offset: u32 = 0;
            var utf16_offset: u32 = 0;
            for (cells[0..logical_columns], 0..) |cell, column| {
                if (isSpacer(cell)) continue;
                try writer.appendInt(u32, byte_offset);
                try writer.appendInt(u32, utf16_offset);
                try writer.appendInt(u16, @intCast(column));
                try writer.appendInt(u8, if (cell.wide == .wide) 2 else 1);
                try writer.appendInt(u8, 0);
                byte_offset += cellTextByteLength(pin, cell);
                utf16_offset += cellTextUtf16Length(pin, cell);
            }
        }
        if (mapping_count > max_row_window_entries - entry_count) {
            return error.ProjectionTooLarge;
        }
        entry_count = std.math.add(u32, entry_count, mapping_count) catch
            return error.ProjectionTooLarge;
    }
    return .{ .bytes = try writer.toOwnedSlice(), .entry_count = entry_count };
}

fn renderColors(terminal: *const ghostty_vt.Terminal) ghostty_vt.RenderState.Colors {
    const background = terminal.colors.background.get() orelse ghostty_vt.color.RGB{
        .r = 0,
        .g = 0,
        .b = 0,
    };
    const foreground = terminal.colors.foreground.get() orelse ghostty_vt.color.RGB{
        .r = 0xff,
        .g = 0xff,
        .b = 0xff,
    };
    return .{
        .background = if (terminal.modes.get(.reverse_colors)) foreground else background,
        .foreground = if (terminal.modes.get(.reverse_colors)) background else foreground,
        .cursor = terminal.colors.cursor.get(),
        .palette = terminal.colors.palette.current,
    };
}

fn hyperlinkTarget(pin: ghostty_vt.Pin, cell: ghostty_vt.page.Cell, column: usize) ?[]const u8 {
    if (!cell.hyperlink) return null;
    const page = &pin.node.data;
    const page_cell = page.getRowAndCell(column, pin.y).cell;
    const identifier = page.lookupHyperlink(page_cell) orelse return null;
    const entry = page.hyperlink_set.get(page.memory, identifier);
    return entry.uri.slice(page.memory);
}

fn textColumns(cells: []const ghostty_vt.page.Cell) usize {
    var index = cells.len;
    while (index > 0) {
        index -= 1;
        if (cells[index].hasText()) return index + 1;
    }
    return 0;
}

fn isSpacer(cell: ghostty_vt.page.Cell) bool {
    return cell.wide == .spacer_tail or cell.wide == .spacer_head;
}

fn isIdentityAsciiRow(pin: ghostty_vt.Pin, cells: []const ghostty_vt.page.Cell) bool {
    for (cells) |cell| {
        if (cell.wide != .narrow) return false;
        if (!cell.hasText()) continue;
        if (cell.codepoint() > 0x7f) return false;
        if (pin.grapheme(&cell)) |trailing| {
            if (trailing.len != 0) return false;
        }
    }
    return true;
}

fn cellTextByteLength(pin: ghostty_vt.Pin, cell: ghostty_vt.page.Cell) u32 {
    if (!cell.hasText()) return 1;
    var length: u32 = std.unicode.utf8CodepointSequenceLength(cell.codepoint()) catch 0;
    if (pin.grapheme(&cell)) |trailing| {
        for (trailing) |codepoint| {
            length += std.unicode.utf8CodepointSequenceLength(codepoint) catch 0;
        }
    }
    return length;
}

fn cellTextUtf16Length(pin: ghostty_vt.Pin, cell: ghostty_vt.page.Cell) u32 {
    if (!cell.hasText()) return 1;
    var length: u32 = if (cell.codepoint() > 0xffff) 2 else 1;
    if (pin.grapheme(&cell)) |trailing| {
        for (trailing) |codepoint| length += if (codepoint > 0xffff) 2 else 1;
    }
    return length;
}

fn appendCellText(
    writer: *projection.LimitedWriter,
    pin: ghostty_vt.Pin,
    cell: ghostty_vt.page.Cell,
) !void {
    if (!cell.hasText()) {
        try writer.appendSlice(" ");
        return;
    }
    var buffer: [projection.max_string_bytes]u8 = undefined;
    const grapheme = try projection.cellGrapheme(
        cell,
        pin.grapheme(&cell) orelse &.{},
        &buffer,
    );
    try writer.appendSlice(grapheme);
}

test "idle manager owns no tracked pins or row capture" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const initial_pins = terminal.screens.active.pages.countTrackedPins();
    try manager.attach([_]u8{2} ** 16);
    try std.testing.expectEqual(@as(usize, 1), manager.states.items.len);
    try std.testing.expectEqual(initial_pins, terminal.screens.active.pages.countTrackedPins());
}

test "text row window is bounded and cross-language valid" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("alpha\r\nbeta");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = [_]u8{2} ** 16;
    try manager.attach(attachment);
    const rows = try manager.readRows(&terminal, 1, 1, 8, 16, .{
        .attachment_id = attachment,
        .detail = .text,
        .generation = 1,
        .host_epoch = [_]u8{1} ** 16,
        .maximum_bytes = max_row_window_bytes,
        .origin = .{ .absolute = 0 },
        .purpose = .search,
        .request_sequence = 1,
        .revision = 1,
        .row_count = 2,
    });
    defer std.testing.allocator.free(rows.bytes);
    const summary = try protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes);
    try std.testing.expectEqual(protocol.RowWindowDetail.text, summary.detail);
    try std.testing.expectEqual(@as(u16, 2), summary.row_count);
    try std.testing.expectEqual(@as(u8, 1), rows.bytes[5]);
    try std.testing.expect(rows.bytes[112] & 0x04 != 0);
    try std.testing.expectEqual(
        @as(u32, 0),
        std.mem.readInt(u32, rows.bytes[120..124], .little),
    );

    rows.bytes[5] = 0;
    try std.testing.expectError(
        error.UnknownRequiredFlag,
        protocol.validateTerminalRowWindow(std.testing.allocator, rows.bytes),
    );
}

test "screen replacement publishes one selection eviction event" {
    var terminal: ghostty_vt.Terminal = try .init(std.testing.allocator, .{
        .cols = 8,
        .rows = 2,
    });
    defer terminal.deinit(std.testing.allocator);
    try terminal.printString("selected");
    var manager = Manager.init(std.testing.allocator);
    defer manager.deinit(&terminal);
    const attachment = [_]u8{2} ** 16;
    try manager.attach(attachment);

    const selected = try manager.applySelectionCommand(&terminal, 1, 1, .{
        .attachment_id = attachment,
        .command = .select_all,
        .generation = 1,
        .revision = 1,
    });
    try std.testing.expectEqual(SelectionSnapshot.active, std.meta.activeTag(selected));

    manager.screenChanged(&terminal);
    const event = (try manager.takeSelectionEvent(attachment)).?;
    try std.testing.expectEqual(SelectionSnapshot.evicted, std.meta.activeTag(event));
    try std.testing.expect((try manager.takeSelectionEvent(attachment)) == null);
}
