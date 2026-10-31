const std = @import("std");

pub fn executable(allocator: std.mem.Allocator, arguments: []const []const u8) ![:0]u8 {
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    const writer = &output.writer;
    for (arguments, 0..) |argument, argument_index| {
        if (argument_index != 0) try writer.writeByte(' ');
        const requires_quotes = argument.len == 0 or std.mem.indexOfAny(u8, argument, " \t\n\"") != null;
        if (!requires_quotes) {
            try writer.writeAll(argument);
            continue;
        }
        try writer.writeByte('"');
        var backslashes: usize = 0;
        for (argument) |byte| switch (byte) {
            '\\' => backslashes += 1,
            '"' => {
                try writer.splatByteAll('\\', backslashes * 2 + 1);
                try writer.writeByte('"');
                backslashes = 0;
            },
            else => {
                try writer.splatByteAll('\\', backslashes);
                try writer.writeByte(byte);
                backslashes = 0;
            },
        };
        try writer.splatByteAll('\\', backslashes * 2);
        try writer.writeByte('"');
    }
    return output.toOwnedSliceSentinel(0);
}

/// Extension selection is explicit; executable launches never enter a shell.
pub fn isBatch(path: []const u8) bool {
    const extension = std.fs.path.extension(path);
    return std.ascii.eqlIgnoreCase(extension, ".cmd") or std.ascii.eqlIgnoreCase(extension, ".bat");
}

/// cmd.exe consumes a different quoting language than a normal Win32 argv.
/// Always quote each argument, double embedded quotes/backslashes, suppress
/// percent expansion and disable AutoRun/delayed expansion. The script still
/// owns any subsequent expansion it performs on its parameters.
pub fn batch(allocator: std.mem.Allocator, script_path: []const u8, arguments: []const []const u8) ![:0]u8 {
    if (!std.fs.path.isAbsoluteWindows(script_path) or std.mem.indexOfAny(u8, script_path, "\x00\r\n\"") != null)
        return error.InvalidBatchPath;
    var output: std.Io.Writer.Allocating = .init(allocator);
    defer output.deinit();
    const writer = &output.writer;
    try writer.writeAll("cmd.exe /e:ON /v:OFF /d /c \"");
    // cmd.exe needs the ordinary spelling of a canonical catalog path.
    const script = (try ordinaryPath(allocator, script_path)) orelse return error.InvalidBatchPath;
    defer allocator.free(script);
    if (std.mem.startsWith(u8, script, "\\\\.\\") or std.mem.endsWith(u8, script, "\\")) return error.InvalidBatchPath;
    try appendBatchArgument(writer, script);
    for (arguments) |argument| {
        try writer.writeByte(' ');
        try appendBatchArgument(writer, argument);
        if (output.written().len > 8191) return error.BatchCommandTooLong;
    }
    try writer.writeByte('"');
    // cmd.exe's smaller limit must be enforced before any process exists.
    if (output.written().len > 8191) return error.BatchCommandTooLong;
    return output.toOwnedSliceSentinel(0);
}

/// Returns an owned ordinary Win32 spelling of `path`. Canonical Windows catalog paths use the
/// verbatim drive or UNC prefix, which programs such as cmd.exe refuse; other paths are copied
/// unchanged. Null means dropping the prefix could change what the path names: a device path,
/// an empty, `.` or `..` component, a forward slash, a component ending in a dot or space, or a
/// DOS device name, all of which the Win32 path parser rewrites.
pub fn ordinaryPath(allocator: std.mem.Allocator, path: []const u8) !?[]u8 {
    if (!std.mem.startsWith(u8, path, "\\\\?\\")) return try allocator.dupe(u8, path);
    var rest = path[4..];
    const unc = std.ascii.startsWithIgnoreCase(rest, "UNC\\");
    if (unc) {
        rest = rest[4..];
        if (rest.len == 0) return null;
    } else if (rest.len < 3 or !std.ascii.isAlphabetic(rest[0]) or rest[1] != ':' or rest[2] != '\\') {
        return null;
    }
    if (std.mem.indexOfScalar(u8, rest, '/') != null) return null;
    var parts = std.mem.splitScalar(u8, if (unc) rest else rest[3..], '\\');
    while (parts.next()) |part| {
        // Only a trailing separator may leave the final component empty.
        if (part.len == 0) {
            if (parts.peek() == null) break;
            return null;
        }
        if (std.mem.endsWith(u8, part, ".") or std.mem.endsWith(u8, part, " ") or isDosDeviceName(part))
            return null;
    }
    return if (unc) try std.fmt.allocPrint(allocator, "\\\\{s}", .{rest}) else try allocator.dupe(u8, rest);
}

/// Win32 maps these names to devices in any directory, with or without an extension.
fn isDosDeviceName(component: []const u8) bool {
    const stem = std.mem.trimEnd(u8, component[0 .. std.mem.indexOfScalar(u8, component, '.') orelse component.len], " ");
    for ([_][]const u8{ "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$" }) |name| {
        if (std.ascii.eqlIgnoreCase(stem, name)) return true;
    }
    if (stem.len < 4 or !(std.ascii.startsWithIgnoreCase(stem, "COM") or std.ascii.startsWithIgnoreCase(stem, "LPT")))
        return false;
    const suffix = stem[3..];
    return (suffix.len == 1 and std.ascii.isDigit(suffix[0])) or
        std.mem.eql(u8, suffix, "\u{00B9}") or std.mem.eql(u8, suffix, "\u{00B2}") or std.mem.eql(u8, suffix, "\u{00B3}");
}

fn appendBatchArgument(writer: *std.Io.Writer, argument: []const u8) !void {
    if (std.mem.indexOfAny(u8, argument, "\x00\r\n") != null) return error.InvalidBatchArgument;
    try writer.writeByte('"');
    var slashes: usize = 0;
    for (argument) |byte| {
        if (byte == '\\') {
            slashes += 1;
        } else {
            if (byte == '"') {
                try writer.splatByteAll('\\', slashes);
                try writer.writeByte('"');
            } else if (byte == '%') {
                // Empty CD substring prevents pairing this percent with the
                // next percent as an environment-variable expansion.
                try writer.writeAll("%%cd:~,");
            }
            slashes = 0;
        }
        try writer.writeByte(byte);
    }
    try writer.splatByteAll('\\', slashes);
    try writer.writeByte('"');
}
