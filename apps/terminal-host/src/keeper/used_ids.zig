//! The request ids one Keeper connection has used, which it never accepts again.

const std = @import("std");

/// The ids a connection has used, as disjoint ascending ranges, so a requester that counts up
/// costs one range however long it lives.
pub const UsedIds = struct {
    ranges: std.ArrayList([2]u32) = .empty,

    pub fn deinit(self: *UsedIds, allocator: std.mem.Allocator) void {
        self.ranges.deinit(allocator);
    }

    /// A set holding a copy of `ranges`, ascending and disjoint as `ranges` reads them.
    pub fn fromRanges(allocator: std.mem.Allocator, ranges: []const [2]u32) error{OutOfMemory}!UsedIds {
        var used: UsedIds = .{};
        try used.ranges.appendSlice(allocator, ranges);
        return used;
    }

    /// Records `id`; false when it was already used.
    pub fn insert(self: *UsedIds, allocator: std.mem.Allocator, id: u32) error{OutOfMemory}!bool {
        const ranges = self.ranges.items;
        var low: usize = 0;
        var high: usize = ranges.len;
        while (low < high) {
            const middle = low + (high - low) / 2;
            if (ranges[middle][1] < id) low = middle + 1 else high = middle;
        }
        const index = low;
        if (index < ranges.len and ranges[index][0] <= id) return false;
        const joins_left = index > 0 and ranges[index - 1][1] + 1 == id;
        const joins_right = index < ranges.len and ranges[index][0] - 1 == id;
        if (joins_left and joins_right) {
            ranges[index - 1][1] = ranges[index][1];
            _ = self.ranges.orderedRemove(index);
        } else if (joins_left) {
            ranges[index - 1][1] = id;
        } else if (joins_right) {
            ranges[index][0] = id;
        } else {
            try self.ranges.insert(allocator, index, .{ id, id });
        }
        return true;
    }
};
