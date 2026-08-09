const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const render_encoding = @import("render_encoding.zig");

const max_frame_bytes = protocol.Limits.frame_bytes;
const max_frame_strings = protocol.Limits.frame_strings;
const max_link_cache_bytes = max_frame_bytes;
const max_string_bytes = render_encoding.max_string_bytes;
const no_string_index = render_encoding.no_string_index;
const no_base_revision = render_encoding.no_base_revision;
const color_default = render_encoding.color_default;
const LimitedWriter = render_encoding.LimitedWriter;
const StringTable = render_encoding.StringTable;
const StyleTable = render_encoding.StyleTable;
const cellGrapheme = render_encoding.cellGrapheme;
const cellWidth = render_encoding.cellWidth;
const cursorShape = render_encoding.cursorShape;
const rgbCode = render_encoding.rgbCode;
const rowPromptClass = render_encoding.rowPromptClass;
const semanticContent = render_encoding.semanticContent;
const styleRecord = render_encoding.styleRecord;
const capturedStyle = render_encoding.capturedStyle;
const terminalInteraction = render_encoding.terminalInteraction;

pub const ProjectionError = error{
    CaptureAllocationFailed,
    ProjectionTooLarge,
};

pub const AttachmentId = identifiers.AttachmentId;
pub const HostEpoch = identifiers.HostEpoch;

pub const Counters = struct {
    render_state_creations: u64 = 0,
    captures: u64 = 0,
    encodes: u64 = 0,
};

pub const FrameKind = protocol.FrameKind;

pub const EncodeRequest = struct {
    attachment_id: AttachmentId,
    base_generation: ?u64,
    base_revision: ?u64,
    force_snapshot: bool,
    host_epoch: HostEpoch,
    maximum_bytes: usize,
    sequence: u64,
};

pub const EncodedFrame = struct {
    allocation: []u8,
    base_revision: ?u64,
    bytes: []u8,
    generation: u64,
    kind: FrameKind,
    revision: u64,
    sequence: u64,
};

const LinkRow = struct {
    indices: std.ArrayList(u32) = .empty,

    fn deinit(self: *LinkRow, allocator: std.mem.Allocator) void {
        self.indices.deinit(allocator);
        self.* = .{};
    }

    fn clear(self: *LinkRow) void {
        self.indices.clearRetainingCapacity();
    }

    fn rebuild(
        self: *LinkRow,
        allocator: std.mem.Allocator,
        pin: ghostty_vt.PageList.Pin,
        cells: []const ghostty_vt.page.Cell,
        link_table: *LinkTable,
    ) (std.mem.Allocator.Error || error{ProjectionTooLarge})!void {
        self.clear();
        var any_link = false;
        for (cells) |cell| any_link = any_link or cell.hyperlink;
        if (!any_link) return;

        try self.indices.resize(allocator, cells.len);
        @memset(self.indices.items, no_string_index);

        const page: *ghostty_vt.Page = &pin.node.data;
        for (cells, 0..) |cell, column| {
            if (!cell.hyperlink) continue;
            const page_cell = page.getRowAndCell(column, pin.y).cell;
            const identifier = page.lookupHyperlink(page_cell) orelse continue;
            const entry = page.hyperlink_set.get(page.memory, identifier);
            const target = entry.uri.slice(page.memory);
            const index = try link_table.intern(allocator, target);
            if (index != no_string_index) link_table.retain(index);
            self.indices.items[column] = index;
        }
    }

    fn uri(self: *const LinkRow, column: usize, link_table: *const LinkTable) ?[]const u8 {
        if (column >= self.indices.items.len) return null;
        return link_table.uri(self.indices.items[column]);
    }
};

const LinkTable = struct {
    allocator: std.mem.Allocator,
    indices: std.StringHashMapUnmanaged(u32) = .empty,
    values: std.ArrayList([]u8) = .empty,
    ref_counts: std.ArrayList(u32) = .empty,
    byte_count: usize = 0,

    fn init(allocator: std.mem.Allocator) LinkTable {
        return .{ .allocator = allocator };
    }

    fn deinit(self: *LinkTable) void {
        self.indices.deinit(self.allocator);
        for (self.values.items) |value| self.allocator.free(value);
        self.values.deinit(self.allocator);
        self.ref_counts.deinit(self.allocator);
        self.* = undefined;
    }

    fn clear(self: *LinkTable) void {
        self.indices.clearRetainingCapacity();
        for (self.values.items) |value| self.allocator.free(value);
        self.values.clearRetainingCapacity();
        self.ref_counts.clearRetainingCapacity();
        self.byte_count = 0;
    }

    fn intern(self: *LinkTable, allocator: std.mem.Allocator, value: []const u8) !u32 {
        if (value.len > max_string_bytes) return error.ProjectionTooLarge;
        if (!std.unicode.utf8ValidateSlice(value)) return no_string_index;
        if (self.indices.get(value)) |index| return index;
        if (self.values.items.len >= max_frame_strings or
            value.len > max_link_cache_bytes -| self.byte_count)
        {
            return error.ProjectionTooLarge;
        }
        const copy = try allocator.dupe(u8, value);
        errdefer allocator.free(copy);
        try self.values.append(allocator, copy);
        errdefer _ = self.values.pop();
        try self.ref_counts.append(allocator, 0);
        errdefer _ = self.ref_counts.pop();
        const index: u32 = @intCast(self.values.items.len - 1);
        try self.indices.put(allocator, copy, index);
        self.byte_count += value.len;
        return index;
    }

    fn retain(self: *LinkTable, index: u32) void {
        if (index == no_string_index) return;
        self.ref_counts.items[index] += 1;
    }

    fn release(self: *LinkTable, index: u32) void {
        if (index == no_string_index) return;
        std.debug.assert(self.ref_counts.items[index] > 0);
        self.ref_counts.items[index] -= 1;
    }

    fn releaseRow(self: *LinkTable, row: *const LinkRow) void {
        for (row.indices.items) |index| self.release(index);
    }

    fn hasReclaimable(self: *const LinkTable) bool {
        for (self.ref_counts.items) |count| if (count == 0) return true;
        return false;
    }

    fn uri(self: *const LinkTable, index: u32) ?[]const u8 {
        if (index == no_string_index or index >= self.values.items.len) return null;
        return self.values.items[index];
    }
};

pub const Projection = struct {
    allocator: std.mem.Allocator,
    capture_full: bool = false,
    captured_cell_pixel_height: u16 = 1,
    captured_cell_pixel_width: u16 = 1,
    captured_current_directory: ?[]u8 = null,
    captured_generation: u64 = 1,
    captured_interaction: u32 = 0,
    captured_revision: u64 = 0,
    captured_scrollback_rows: u64 = 0,
    captured_title: ?[]u8 = null,
    counters: Counters = .{},
    link_table: LinkTable,
    links: std.ArrayList(LinkRow) = .empty,
    render_state: ?ghostty_vt.RenderState = null,
    row_revisions: std.ArrayList(u64) = .empty,

    pub fn init(allocator: std.mem.Allocator) Projection {
        return .{ .allocator = allocator, .link_table = .init(allocator) };
    }

    pub fn deinit(self: *Projection) void {
        self.releaseResources();
        self.* = undefined;
    }

    pub fn resetAfterCaptureFailure(self: *Projection) void {
        self.releaseResources();
    }

    pub fn releaseResources(self: *Projection) void {
        if (self.render_state) |*state| state.deinit(self.allocator);
        self.render_state = null;
        self.row_revisions.deinit(self.allocator);
        self.row_revisions = .empty;
        for (self.links.items) |*row| row.deinit(self.allocator);
        self.links.deinit(self.allocator);
        self.links = .empty;
        self.link_table.deinit();
        self.link_table = .init(self.allocator);
        if (self.captured_title) |value| self.allocator.free(value);
        if (self.captured_current_directory) |value| self.allocator.free(value);
        self.captured_title = null;
        self.captured_current_directory = null;
        self.capture_full = false;
        self.captured_interaction = 0;
        self.captured_revision = 0;
        self.captured_scrollback_rows = 0;
    }

    pub fn isResident(self: *const Projection) bool {
        return self.render_state != null;
    }

    pub fn beginCapture(
        self: *Projection,
        terminal: *ghostty_vt.Terminal,
        revision: u64,
        generation: u64,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) (std.mem.Allocator.Error || error{ProjectionTooLarge})!void {
        if (self.render_state == null) {
            self.render_state = .empty;
            self.counters.render_state_creations += 1;
        }
        const state = &self.render_state.?;
        try state.beginUpdate(self.allocator, terminal);

        const dimensions_changed = self.row_revisions.items.len != state.rows;
        self.capture_full = dimensions_changed or generation != self.captured_generation or
            state.dirty == .full;

        while (self.links.items.len > state.rows) {
            var row = self.links.pop().?;
            self.link_table.releaseRow(&row);
            row.deinit(self.allocator);
        }
        while (self.links.items.len < state.rows) try self.links.append(self.allocator, .{});
        if (self.capture_full) self.link_table.clear();
        const previous_row_count = self.row_revisions.items.len;
        try self.row_revisions.resize(self.allocator, state.rows);
        if (state.rows > previous_row_count) {
            @memset(self.row_revisions.items[previous_row_count..], revision);
        }

        const row_data = state.row_data.slice();
        const row_dirties = row_data.items(.dirty);
        const row_pins = row_data.items(.pin);
        const row_cells = row_data.items(.cells);
        if (!self.capture_full) {
            for (0..state.rows) |row_index| {
                if (!row_dirties[row_index]) continue;
                self.link_table.releaseRow(&self.links.items[row_index]);
                self.links.items[row_index].clear();
            }
            if (self.link_table.hasReclaimable()) {
                try self.compactLinkTable();
            }
        }
        for (0..state.rows) |row_index| {
            if (!self.capture_full and !row_dirties[row_index]) continue;
            try self.links.items[row_index].rebuild(
                self.allocator,
                row_pins[row_index],
                row_cells[row_index].items(.raw),
                &self.link_table,
            );
        }

        const title = try duplicateOptional(self.allocator, terminal.getTitle());
        errdefer if (title) |value| self.allocator.free(value);
        const current_directory = try duplicateOptional(self.allocator, terminal.getPwd());
        errdefer if (current_directory) |value| self.allocator.free(value);
        if (self.captured_title) |value| self.allocator.free(value);
        if (self.captured_current_directory) |value| self.allocator.free(value);
        self.captured_title = title;
        self.captured_current_directory = current_directory;
        self.captured_revision = revision;
        self.captured_generation = generation;
        self.captured_cell_pixel_width = cell_pixel_width;
        self.captured_cell_pixel_height = cell_pixel_height;
        self.captured_scrollback_rows = terminal.screens.active.pages.total_rows - terminal.rows;
        self.captured_interaction = terminalInteraction(terminal);
    }

    pub fn endCapture(self: *Projection) void {
        const state = &self.render_state.?;
        state.endUpdate();
        const dirties = state.row_data.items(.dirty);
        for (dirties, 0..) |*dirty, row_index| {
            if (self.capture_full or dirty.*) self.row_revisions.items[row_index] = self.captured_revision;
            dirty.* = false;
        }
        state.dirty = .false;
        self.counters.captures += 1;
    }

    fn compactLinkTable(self: *Projection) !void {
        const remap = try self.allocator.alloc(u32, self.link_table.values.items.len);
        defer self.allocator.free(remap);
        const live_values = try self.allocator.alloc([]u8, self.link_table.values.items.len);
        defer self.allocator.free(live_values);
        const live_ref_counts = try self.allocator.alloc(u32, self.link_table.values.items.len);
        defer self.allocator.free(live_ref_counts);
        @memset(remap, no_string_index);

        var live_count: usize = 0;
        var live_bytes: usize = 0;
        for (self.link_table.values.items, 0..) |value, old_index| {
            if (self.link_table.ref_counts.items[old_index] == 0) {
                self.allocator.free(value);
                continue;
            }
            remap[old_index] = @intCast(live_count);
            live_values[live_count] = value;
            live_ref_counts[live_count] = self.link_table.ref_counts.items[old_index];
            live_count += 1;
            live_bytes += value.len;
        }
        @memcpy(self.link_table.values.items[0..live_count], live_values[0..live_count]);
        @memcpy(self.link_table.ref_counts.items[0..live_count], live_ref_counts[0..live_count]);
        self.link_table.values.shrinkRetainingCapacity(live_count);
        self.link_table.ref_counts.shrinkRetainingCapacity(live_count);
        self.link_table.indices.clearRetainingCapacity();
        for (self.link_table.values.items, 0..) |value, index| {
            try self.link_table.indices.put(self.allocator, value, @intCast(index));
        }
        self.link_table.byte_count = live_bytes;

        for (self.links.items) |*row| {
            for (row.indices.items) |*index| {
                if (index.* == no_string_index) continue;
                const next = remap[index.*];
                std.debug.assert(next != no_string_index);
                index.* = next;
            }
        }
    }

    pub fn needsCapture(self: *const Projection, revision: u64, generation: u64) bool {
        return self.render_state == null or self.captured_revision < revision or
            self.captured_generation != generation;
    }

    pub fn encode(
        self: *Projection,
        request: EncodeRequest,
    ) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
        return self.encodeOwned(self.allocator, 0, request);
    }

    pub fn encodeOwned(
        self: *Projection,
        output_allocator: std.mem.Allocator,
        output_prefix_bytes: usize,
        request: EncodeRequest,
    ) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
        const state = if (self.render_state) |*value| value else return error.CaptureAllocationFailed;
        const snapshot = request.force_snapshot or request.base_revision == null or
            request.base_generation == null or
            request.base_generation.? != self.captured_generation;
        const kind: FrameKind = if (snapshot) .snapshot else .delta;
        const base_revision: ?u64 = if (snapshot) null else request.base_revision;
        if (!snapshot and base_revision.? >= self.captured_revision) return error.ProjectionTooLarge;

        var strings = StringTable.init(self.allocator);
        defer strings.deinit();
        var styles = StyleTable.init(self.allocator);
        defer styles.deinit();
        const title_index = try strings.addOptional(self.captured_title);
        const current_directory_index = try strings.addOptional(self.captured_current_directory);

        var included_rows: usize = 0;
        const row_data = state.row_data.slice();
        const row_cells = row_data.items(.cells);
        for (0..state.rows) |row_index| {
            if (!snapshot and self.row_revisions.items[row_index] <= base_revision.?) continue;
            included_rows += 1;
            const cells = row_cells[row_index].slice();
            for (0..state.cols) |column| {
                var grapheme_buffer: [max_string_bytes]u8 = undefined;
                const grapheme = try cellGrapheme(
                    cells.items(.raw)[column],
                    cells.items(.grapheme)[column],
                    &grapheme_buffer,
                );
                _ = try strings.add(grapheme);
                const hyperlink_index = try strings.addOptional(
                    self.links.items[row_index].uri(column, &self.link_table),
                );
                const style = styleRecord(
                    cells.items(.raw)[column],
                    capturedStyle(cells, column),
                    &state.colors,
                    hyperlink_index,
                );
                _ = try styles.add(style);
            }
        }

        const hard_limit = @min(request.maximum_bytes, max_frame_bytes);
        const allocation_limit = std.math.add(
            usize,
            output_prefix_bytes,
            hard_limit,
        ) catch return error.ProjectionTooLarge;
        var writer = LimitedWriter.init(output_allocator, allocation_limit);
        errdefer writer.deinit();
        try writer.appendZeros(output_prefix_bytes);
        const frame_offset = writer.bytes.items.len;
        try writer.appendSlice(protocol.terminal_frame_magic);
        try writer.appendInt(u8, protocol.terminal_frame_format_major);
        try writer.appendInt(u8, protocol.terminal_frame_format_minor);
        try writer.appendInt(u8, @intFromEnum(kind));
        try writer.appendInt(u8, 0);
        const total_length_offset = writer.bytes.items.len;
        try writer.appendInt(u32, 0);
        try writer.appendInt(u16, protocol.terminal_frame_header_bytes);
        try writer.appendInt(u16, 0);
        try writer.appendSlice(&request.host_epoch.bytes);
        try writer.appendSlice(&request.attachment_id.bytes);
        try writer.appendInt(u64, request.sequence);
        try writer.appendInt(u64, base_revision orelse no_base_revision);
        try writer.appendInt(u64, self.captured_revision);
        try writer.appendInt(u64, self.captured_generation);
        try writer.appendInt(u64, self.captured_scrollback_rows);
        try writer.appendInt(u16, state.cols);
        try writer.appendInt(u16, state.rows);
        const cursor = state.cursor.viewport;
        const cursor_column: u16 = if (cursor) |value| value.x else @min(state.cursor.active.x, state.cols - 1);
        const cursor_row: u16 = if (cursor) |value| value.y else @min(state.cursor.active.y, state.rows - 1);
        try writer.appendInt(u16, cursor_column);
        try writer.appendInt(u16, cursor_row);
        try writer.appendInt(u8, cursorShape(state.cursor.visual_style));
        try writer.appendInt(u8, (if (state.cursor.visible and cursor != null) @as(u8, 1) else 0) |
            (if (state.cursor.blinking) @as(u8, 2) else 0) |
            (if (cursor != null and cursor.?.wide_tail) @as(u8, 4) else 0));
        try writer.appendInt(u8, if (state.screen == .primary) 1 else 2);
        try writer.appendInt(u8, 0);
        try writer.appendInt(u32, self.captured_interaction);
        try writer.appendInt(u32, rgbCode(state.colors.foreground));
        try writer.appendInt(u32, rgbCode(state.colors.background));
        try writer.appendInt(u32, if (state.colors.cursor) |value| rgbCode(value) else color_default);
        try writer.appendInt(u32, title_index);
        try writer.appendInt(u32, current_directory_index);
        try writer.appendInt(u32, @intCast(strings.values.items.len));
        try writer.appendInt(u16, @intCast(styles.values.items.len));
        try writer.appendInt(u16, @intCast(included_rows));
        try writer.appendInt(u16, 0);
        try writer.appendInt(u16, 0);
        try writer.appendInt(u16, self.captured_cell_pixel_width);
        try writer.appendInt(u16, self.captured_cell_pixel_height);
        try writer.appendInt(u32, @intCast(included_rows * state.cols));
        try writer.appendInt(u8, protocol.terminal_frame_format_patch);
        try writer.appendZeros(15);
        std.debug.assert(
            writer.bytes.items.len - frame_offset == protocol.terminal_frame_header_bytes,
        );

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

        const row_raw = row_data.items(.raw);
        for (0..state.rows) |row_index| {
            if (!snapshot and self.row_revisions.items[row_index] <= base_revision.?) continue;
            const row = row_raw[row_index];
            try writer.appendInt(u16, @intCast(row_index));
            try writer.appendInt(u8, (if (row.wrap) @as(u8, 1) else 0) |
                (if (row.wrap_continuation) @as(u8, 2) else 0));
            try writer.appendInt(u8, rowPromptClass(row.semantic_prompt));
            try writer.appendInt(u16, state.cols);
            try writer.appendInt(u16, 0);
            const cells = row_cells[row_index].slice();
            for (0..state.cols) |column| {
                var grapheme_buffer: [max_string_bytes]u8 = undefined;
                const grapheme = try cellGrapheme(
                    cells.items(.raw)[column],
                    cells.items(.grapheme)[column],
                    &grapheme_buffer,
                );
                const grapheme_index = strings.indexOf(grapheme).?;
                const hyperlink_index = strings.indexOfOptional(
                    self.links.items[row_index].uri(column, &self.link_table),
                );
                const style = styleRecord(
                    cells.items(.raw)[column],
                    capturedStyle(cells, column),
                    &state.colors,
                    hyperlink_index,
                );
                try writer.appendInt(u32, grapheme_index);
                try writer.appendInt(u16, styles.indexOf(style).?);
                const raw = cells.items(.raw)[column];
                try writer.appendInt(u8, cellWidth(raw.wide) |
                    (semanticContent(raw.semantic_content) << 2) |
                    (if (raw.protected) @as(u8, 0x10) else 0));
                try writer.appendInt(u8, 0);
            }
        }

        const frame_length = writer.bytes.items.len - frame_offset;
        if (frame_length > std.math.maxInt(u32)) return error.ProjectionTooLarge;
        std.mem.writeInt(
            u32,
            writer.bytes.items[total_length_offset..][0..4],
            @intCast(frame_length),
            .little,
        );
        self.counters.encodes += 1;
        const allocation = try writer.toOwnedSlice();
        return .{
            .allocation = allocation,
            .base_revision = base_revision,
            .bytes = allocation[output_prefix_bytes..],
            .generation = self.captured_generation,
            .kind = kind,
            .revision = self.captured_revision,
            .sequence = request.sequence,
        };
    }
};

fn duplicateOptional(allocator: std.mem.Allocator, value: ?[:0]const u8) !?[]u8 {
    const bytes = value orelse return null;
    if (bytes.len > max_string_bytes) return error.ProjectionTooLarge;
    return try allocator.dupe(u8, bytes);
}

pub const testing = if (@import("builtin").is_test) struct {
    pub const frame_byte_limit = max_frame_bytes;
    pub const link_cache_byte_limit = max_link_cache_bytes;
    pub const link_entry_limit = max_frame_strings;
    pub const link_value_byte_limit = max_string_bytes;
    pub const LinkTableType = LinkTable;
    pub const linkTableDeinitFn = LinkTable.deinit;
    pub const linkTableInitFn = LinkTable.init;
    pub const linkTableInternFn = LinkTable.intern;
    pub const projection_source = @embedFile("projection.zig");
    pub const StringTableType = StringTable;
} else struct {};
