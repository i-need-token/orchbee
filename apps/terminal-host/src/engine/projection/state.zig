const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const render_encoding = @import("../render_encoding.zig");
const contracts = @import("contracts.zig");
const encoding = @import("encoding.zig");
const graphics = @import("graphics.zig");
const links = @import("links.zig");

const max_string_bytes = render_encoding.max_string_bytes;
const no_string_index = render_encoding.no_string_index;
const terminalInteraction = render_encoding.terminalInteraction;
const Counters = contracts.Counters;
const EncodeRequest = contracts.EncodeRequest;
const EncodedFrame = contracts.EncodedFrame;
const ProjectionError = contracts.ProjectionError;
const LinkRow = links.LinkRow;
const SourceLinkCache = links.SourceLinkCache;
const LinkTable = links.LinkTable;

const ProjectionTestingState = if (target_builtin.is_test) struct {
    frame_link_resolutions: usize = 0,
    source_link_resolutions: usize = 0,
} else struct {};

pub const LinkWorkObservation = struct {
    frame_link_resolutions: usize,
    source_link_resolutions: usize,
};

pub const Projection = struct {
    allocator: std.mem.Allocator,
    capture_full: bool = false,
    captured_cell_pixel_height: u16 = 1,
    captured_cell_pixel_width: u16 = 1,
    captured_current_directory: ?[]u8 = null,
    captured_default_colors: render_encoding.DefaultColors = .{},
    captured_generation: u64 = 1,
    captured_interaction: u32 = 0,
    captured_revision: u64 = 0,
    captured_scrollback_rows: u64 = 0,
    captured_title: ?[]u8 = null,
    counters: Counters = .{},
    graphics: graphics.State,
    link_table: LinkTable,
    links: std.ArrayList(LinkRow) = .empty,
    render_state: ?ghostty_vt.RenderState = null,
    row_revisions: std.ArrayList(u64) = .empty,
    testing_state: ProjectionTestingState = .{},

    pub fn init(allocator: std.mem.Allocator) Projection {
        return .{
            .allocator = allocator,
            .graphics = .init(allocator),
            .link_table = .init(allocator),
        };
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
        self.graphics.reset();
        if (self.captured_title) |value| self.allocator.free(value);
        if (self.captured_current_directory) |value| self.allocator.free(value);
        self.captured_title = null;
        self.captured_current_directory = null;
        self.captured_default_colors = .{};
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
        try state.update(self.allocator, terminal);
        try self.graphics.capture(
            terminal,
            revision,
            cell_pixel_width,
            cell_pixel_height,
        );

        const default_colors = render_encoding.defaultColors(terminal, &state.colors);
        const defaults_changed = !std.meta.eql(self.captured_default_colors, default_colors);
        const dimensions_changed = self.row_revisions.items.len != state.rows;
        // OSC-only changes need not dirty Ghostty rows. Compare the encoded
        // marker/RGB pair so equal-RGB set/reset also replaces every old row.
        self.capture_full = dimensions_changed or generation != self.captured_generation or
            state.dirty == .full or defaults_changed;

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
        var source_links: SourceLinkCache = .{};
        defer source_links.deinit(self.allocator);
        for (0..state.rows) |row_index| {
            if (!self.capture_full and !row_dirties[row_index]) continue;
            try self.links.items[row_index].rebuild(
                self.allocator,
                row_pins[row_index],
                row_cells[row_index].items(.raw),
                &self.link_table,
                &source_links,
            );
        }
        if (comptime target_builtin.is_test) {
            self.testing_state.source_link_resolutions +=
                source_links.testingResolutionCount();
        }

        const title = try duplicateOptional(self.allocator, terminal.getTitle());
        errdefer if (title) |value| self.allocator.free(value);
        const current_directory = try duplicateOptional(self.allocator, terminal.getPwd());
        errdefer if (current_directory) |value| self.allocator.free(value);
        if (self.captured_title) |value| self.allocator.free(value);
        if (self.captured_current_directory) |value| self.allocator.free(value);
        self.captured_title = title;
        self.captured_current_directory = current_directory;
        self.captured_default_colors = default_colors;
        self.captured_revision = revision;
        self.captured_generation = generation;
        self.captured_cell_pixel_width = cell_pixel_width;
        self.captured_cell_pixel_height = cell_pixel_height;
        self.captured_scrollback_rows = terminal.screens.active.pages.total_rows - terminal.rows;
        self.captured_interaction = terminalInteraction(terminal);
    }

    pub fn endCapture(self: *Projection) void {
        const state = &self.render_state.?;
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

    pub fn linkWorkObservation(self: *const Projection) LinkWorkObservation {
        return .{
            .frame_link_resolutions = if (comptime target_builtin.is_test)
                self.testing_state.frame_link_resolutions
            else
                0,
            .source_link_resolutions = if (comptime target_builtin.is_test)
                self.testing_state.source_link_resolutions
            else
                0,
        };
    }

    pub fn encode(
        self: *Projection,
        request: EncodeRequest,
    ) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
        return encoding.encode(self, request);
    }

    pub fn encodeOwned(
        self: *Projection,
        output_allocator: std.mem.Allocator,
        output_prefix_bytes: usize,
        request: EncodeRequest,
    ) (std.mem.Allocator.Error || ProjectionError)!EncodedFrame {
        return encoding.encodeOwned(self, output_allocator, output_prefix_bytes, request);
    }
};

fn duplicateOptional(allocator: std.mem.Allocator, value: ?[:0]const u8) !?[]u8 {
    const bytes = value orelse return null;
    if (bytes.len > max_string_bytes) return error.ProjectionTooLarge;
    return try allocator.dupe(u8, bytes);
}
