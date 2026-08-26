const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const protocol = @import("terminal-protocol");
const render_encoding = @import("../render_encoding.zig");

const max_frame_bytes = protocol.Limits.frame_bytes;
const max_frame_strings = protocol.Limits.frame_strings;
const max_link_cache_bytes = max_frame_bytes;
const max_string_bytes = render_encoding.max_string_bytes;
const no_string_index = render_encoding.no_string_index;

pub const SourceLinkKey = struct {
    page: *const ghostty_vt.Page,
    identifier: ghostty_vt.size.HyperlinkCountInt,
};

const SourceLinkCacheTestingState = if (target_builtin.is_test) struct {
    resolutions: usize = 0,
} else struct {};

pub const SourceLinkCache = struct {
    indices: std.AutoHashMapUnmanaged(SourceLinkKey, u32) = .empty,
    testing_state: SourceLinkCacheTestingState = .{},

    pub fn deinit(self: *SourceLinkCache, allocator: std.mem.Allocator) void {
        self.indices.deinit(allocator);
        self.* = undefined;
    }

    pub fn resolve(
        self: *SourceLinkCache,
        allocator: std.mem.Allocator,
        key: SourceLinkKey,
        link_table: *LinkTable,
        resolver: anytype,
    ) !u32 {
        const result = try self.indices.getOrPut(allocator, key);
        if (result.found_existing) return result.value_ptr.*;
        errdefer _ = self.indices.remove(key);
        if (comptime target_builtin.is_test) self.testing_state.resolutions += 1;
        result.value_ptr.* = try link_table.intern(
            allocator,
            resolver.resolve(key),
        );
        return result.value_ptr.*;
    }

    pub fn testingResolutionCount(self: *const SourceLinkCache) usize {
        if (comptime target_builtin.is_test) return self.testing_state.resolutions;
        return 0;
    }
};

const LinkTableTestingState = if (target_builtin.is_test) struct {
    intern_calls: usize = 0,
} else struct {};

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
        source_links: *SourceLinkCache,
    ) (std.mem.Allocator.Error || error{ProjectionTooLarge})!void {
        self.clear();
        var any_link = false;
        for (cells) |cell| any_link = any_link or cell.hyperlink;
        if (!any_link) return;

        try self.indices.resize(allocator, cells.len);
        @memset(self.indices.items, no_string_index);

        const page: *ghostty_vt.Page = pin.node.page();
        for (cells, 0..) |cell, column| {
            if (!cell.hyperlink) continue;
            const page_cell = page.getRowAndCell(column, pin.y).cell;
            const identifier = page.lookupHyperlink(page_cell) orelse continue;
            const link_table_index = try source_links.resolve(
                allocator,
                .{ .page = page, .identifier = identifier },
                link_table,
                GhosttySourceResolver{},
            );
            if (link_table_index != no_string_index) link_table.retain(link_table_index);
            self.indices.items[column] = link_table_index;
        }
    }

    pub fn linkIndex(self: *const LinkRow, column: usize) u32 {
        if (column >= self.indices.items.len) return no_string_index;
        return self.indices.items[column];
    }
};

const GhosttySourceResolver = struct {
    fn resolve(_: GhosttySourceResolver, key: SourceLinkKey) []const u8 {
        const entry = key.page.hyperlink_set.get(key.page.memory, key.identifier);
        return entry.uri.slice(key.page.memory);
    }
};

pub const LinkTable = struct {
    allocator: std.mem.Allocator,
    indices: std.StringHashMapUnmanaged(u32) = .empty,
    values: std.ArrayList([]u8) = .empty,
    ref_counts: std.ArrayList(u32) = .empty,
    byte_count: usize = 0,
    testing_state: LinkTableTestingState = .{},

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
        if (comptime target_builtin.is_test) self.testing_state.intern_calls += 1;
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

    pub fn testingInternCallCount(self: *const LinkTable) usize {
        if (comptime target_builtin.is_test) return self.testing_state.intern_calls;
        return 0;
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
