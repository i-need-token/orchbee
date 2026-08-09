const std = @import("std");

/// A borrowed, allocation-free view of `--name value` command-line pairs.
/// Parsing owns only generic syntax and duplicate detection; callers retain
/// command selection, supported-option declarations, and semantic validation.
pub const Options = struct {
    arguments: []const []const u8,

    pub fn parse(arguments: []const []const u8) !Options {
        if (arguments.len % 2 != 0) return error.InvalidArguments;

        var index: usize = 0;
        while (index < arguments.len) : (index += 2) {
            const name = arguments[index];
            if (name.len <= 2 or !std.mem.startsWith(u8, name, "--")) {
                return error.InvalidArguments;
            }

            var previous_index: usize = 0;
            while (previous_index < index) : (previous_index += 2) {
                if (std.mem.eql(u8, name, arguments[previous_index])) {
                    return error.InvalidArguments;
                }
            }
        }
        return .{ .arguments = arguments };
    }

    pub fn get(self: Options, name: []const u8) ?[]const u8 {
        var index: usize = 0;
        while (index < self.arguments.len) : (index += 2) {
            if (std.mem.eql(u8, name, self.arguments[index])) {
                return self.arguments[index + 1];
            }
        }
        return null;
    }

    pub fn require(self: Options, name: []const u8) ![]const u8 {
        return self.get(name) orelse error.InvalidArguments;
    }

    pub fn ensureOnly(self: Options, supported_names: []const []const u8) !void {
        var index: usize = 0;
        while (index < self.arguments.len) : (index += 2) {
            const name = self.arguments[index];
            var supported = false;
            for (supported_names) |supported_name| {
                if (!std.mem.eql(u8, name, supported_name)) continue;
                supported = true;
                break;
            }
            if (!supported) return error.InvalidArguments;
        }
    }
};

pub fn parseInteger(comptime Integer: type, value: []const u8) !Integer {
    return std.fmt.parseInt(Integer, value, 10) catch return error.InvalidArguments;
}
