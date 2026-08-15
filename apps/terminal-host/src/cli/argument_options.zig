const std = @import("std");

/// An allocator-owned table of borrowed command-line options. Repeated named
/// values become `.many`; repeated flags remain `.flag`; mixing both forms for
/// one name is rejected because it has no unambiguous value shape.
pub const Options = struct {
    allocator: std.mem.Allocator,
    entries: []Entry,
    value_storage: [][]const u8,

    pub const Entry = struct {
        name: []const u8,
        value: Value,
    };

    pub const Value = union(enum) {
        flag,
        one: []const u8,
        many: []const []const u8,
    };

    pub fn parse(allocator: std.mem.Allocator, arguments: []const []const u8) !Options {
        var occurrences = try allocator.alloc(Occurrence, arguments.len);
        defer allocator.free(occurrences);
        var occurrence_count: usize = 0;
        var argument_index: usize = 0;
        while (argument_index < arguments.len) {
            const name = arguments[argument_index];
            if (!isOptionName(name)) return error.InvalidArguments;
            argument_index += 1;

            var value: ?[]const u8 = null;
            if (argument_index < arguments.len and !startsOption(arguments[argument_index])) {
                value = arguments[argument_index];
                argument_index += 1;
            }
            occurrences[occurrence_count] = .{ .name = name, .value = value };
            occurrence_count += 1;
        }

        var groups = try allocator.alloc(Group, occurrence_count);
        defer allocator.free(groups);
        var group_count: usize = 0;
        for (occurrences[0..occurrence_count]) |occurrence| {
            if (findGroup(groups[0..group_count], occurrence.name)) |group| {
                if (occurrence.value == null) {
                    group.has_flag = true;
                } else {
                    group.value_count += 1;
                }
                continue;
            }
            groups[group_count] = .{
                .name = occurrence.name,
                .has_flag = occurrence.value == null,
                .value_count = if (occurrence.value == null) 0 else 1,
            };
            group_count += 1;
        }

        var value_count: usize = 0;
        for (groups[0..group_count]) |group| {
            if (group.has_flag and group.value_count > 0) return error.InvalidArguments;
            value_count += group.value_count;
        }

        var entries = try allocator.alloc(Entry, group_count);
        errdefer allocator.free(entries);
        var value_storage = try allocator.alloc([]const u8, value_count);
        errdefer allocator.free(value_storage);
        var value_index: usize = 0;
        for (groups[0..group_count], 0..) |group, entry_index| {
            if (group.has_flag) {
                entries[entry_index] = .{ .name = group.name, .value = .flag };
                continue;
            }

            const start = value_index;
            for (occurrences[0..occurrence_count]) |occurrence| {
                if (!std.mem.eql(u8, group.name, occurrence.name)) continue;
                if (occurrence.value) |value| {
                    value_storage[value_index] = value;
                    value_index += 1;
                }
            }
            const values = value_storage[start..value_index];
            entries[entry_index] = .{
                .name = group.name,
                .value = if (values.len == 1)
                    .{ .one = values[0] }
                else
                    .{ .many = values },
            };
        }
        return .{
            .allocator = allocator,
            .entries = entries,
            .value_storage = value_storage,
        };
    }

    pub fn deinit(self: *Options) void {
        self.allocator.free(self.entries);
        self.allocator.free(self.value_storage);
        self.* = undefined;
    }

    pub fn get(self: Options, name: []const u8) ?Value {
        for (self.entries) |entry| {
            if (std.mem.eql(u8, name, entry.name)) return entry.value;
        }
        return null;
    }

    pub fn require(self: Options, name: []const u8) !Value {
        return self.get(name) orelse error.InvalidArguments;
    }

    pub fn optionalOne(self: Options, name: []const u8) !?[]const u8 {
        const value = self.get(name) orelse return null;
        return switch (value) {
            .one => |string| string,
            .flag, .many => error.InvalidArguments,
        };
    }

    pub fn requireOne(self: Options, name: []const u8) ![]const u8 {
        return (try self.optionalOne(name)) orelse error.InvalidArguments;
    }

    pub fn requireMany(self: Options, name: []const u8) ![]const []const u8 {
        const value = try self.require(name);
        return switch (value) {
            .many => |strings| strings,
            .flag, .one => error.InvalidArguments,
        };
    }

    pub fn requireFlag(self: Options, name: []const u8) !void {
        const value = try self.require(name);
        return switch (value) {
            .flag => {},
            .one, .many => error.InvalidArguments,
        };
    }

    pub fn ensureOnly(self: Options, supported_names: []const []const u8) !void {
        for (self.entries) |entry| {
            var supported = false;
            for (supported_names) |supported_name| {
                if (!std.mem.eql(u8, entry.name, supported_name)) continue;
                supported = true;
                break;
            }
            if (!supported) return error.InvalidArguments;
        }
    }
};

const Occurrence = struct {
    name: []const u8,
    value: ?[]const u8,
};

const Group = struct {
    name: []const u8,
    has_flag: bool,
    value_count: usize,
};

fn startsOption(argument: []const u8) bool {
    return std.mem.startsWith(u8, argument, "--");
}

fn isOptionName(argument: []const u8) bool {
    return argument.len > 2 and startsOption(argument);
}

fn findGroup(groups: []Group, name: []const u8) ?*Group {
    for (groups) |*group| {
        if (std.mem.eql(u8, name, group.name)) return group;
    }
    return null;
}

pub fn parseInteger(comptime Integer: type, value: []const u8) !Integer {
    return std.fmt.parseInt(Integer, value, 10) catch return error.InvalidArguments;
}
