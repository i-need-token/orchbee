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
    // Canonical Windows catalog paths use the verbatim prefix. cmd.exe needs
    // its ordinary spelling; reject components whose meaning would change.
    var script = script_path;
    var unc = false;
    if (std.mem.startsWith(u8, script, "\\\\?\\")) {
        script = script[4..];
        if (std.ascii.startsWithIgnoreCase(script, "UNC\\")) {
            script = script[4..];
            unc = true;
        } else if (script.len < 3 or !std.ascii.isAlphabetic(script[0]) or script[1] != ':' or script[2] != '\\') {
            return error.InvalidBatchPath;
        }
        var parts = std.mem.splitScalar(u8, script, '\\');
        while (parts.next()) |part| {
            if (part.len != 0 and (std.mem.endsWith(u8, part, ".") or std.mem.endsWith(u8, part, " ")))
                return error.InvalidBatchPath;
        }
    }
    if (std.mem.startsWith(u8, script, "\\\\.\\") or std.mem.endsWith(u8, script, "\\")) return error.InvalidBatchPath;
    const literal_script = if (unc) try std.fmt.allocPrint(allocator, "\\\\{s}", .{script}) else script;
    defer if (unc) allocator.free(literal_script);
    try appendBatchArgument(writer, literal_script);
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
