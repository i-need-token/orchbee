const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const render_encoding = @import("../render_encoding.zig");

const max_frame_bytes = protocol.Limits.frame_bytes;
const max_frame_strings = protocol.Limits.frame_strings;
const max_link_cache_bytes = max_frame_bytes;
const max_string_bytes = render_encoding.max_string_bytes;
const no_string_index = render_encoding.no_string_index;

pub const LinkRow = struct {
    indices: std.ArrayList(u32) = .empty,

    pub fn deinit(self: *LinkRow, allocator: std.mem.Allocator) void {
        self.indices.deinit(allocator);
        self.* = .{};
    }

    pub fn clear(self: *LinkRow) void {
        self.indices.clearRetainingCapacity();
    }

    pub fn rebuild(
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

    pub fn uri(self: *const LinkRow, column: usize, link_table: *const LinkTable) ?[]const u8 {
        if (column >= self.indices.items.len) return null;
        return link_table.uri(self.indices.items[column]);
    }
};

pub const LinkTable = struct {
    allocator: std.mem.Allocator,
    indices: std.StringHashMapUnmanaged(u32) = .empty,
    values: std.ArrayList([]u8) = .empty,
    ref_counts: std.ArrayList(u32) = .empty,
    byte_count: usize = 0,

    pub fn init(allocator: std.mem.Allocator) LinkTable {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *LinkTable) void {
        self.indices.deinit(self.allocator);
        for (self.values.items) |value| self.allocator.free(value);
        self.values.deinit(self.allocator);
        self.ref_counts.deinit(self.allocator);
        self.* = undefined;
    }

    pub fn clear(self: *LinkTable) void {
        self.indices.clearRetainingCapacity();
        for (self.values.items) |value| self.allocator.free(value);
        self.values.clearRetainingCapacity();
        self.ref_counts.clearRetainingCapacity();
        self.byte_count = 0;
    }

    pub fn intern(self: *LinkTable, allocator: std.mem.Allocator, value: []const u8) !u32 {
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

    pub fn retain(self: *LinkTable, index: u32) void {
        if (index == no_string_index) return;
        self.ref_counts.items[index] += 1;
    }

    pub fn release(self: *LinkTable, index: u32) void {
        if (index == no_string_index) return;
        std.debug.assert(self.ref_counts.items[index] > 0);
        self.ref_counts.items[index] -= 1;
    }

    pub fn releaseRow(self: *LinkTable, row: *const LinkRow) void {
        for (row.indices.items) |index| self.release(index);
    }

    pub fn hasReclaimable(self: *const LinkTable) bool {
        for (self.ref_counts.items) |count| if (count == 0) return true;
        return false;
    }

    pub fn uri(self: *const LinkTable, index: u32) ?[]const u8 {
        if (index == no_string_index or index >= self.values.items.len) return null;
        return self.values.items[index];
    }
};
