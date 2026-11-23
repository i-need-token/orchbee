//! The `background-programs` capability's answers and events on `worker-host`. Members appear in
//! bytewise name order, as every control encoder writes them; a process is `{pid, creationTime}`.

const std = @import("std");

/// At most this many bytes of one stream travel in one `ReadProgramOutput` answer, which keeps the
/// base64 text far below the control payload bound.
pub const max_output_chunk_bytes = 24 * 1024;
/// At most this many programs travel in one `ListPrograms` page.
pub const max_list_page = 16;

pub const Exit = union(enum) {
    code: u32,
    cancelled,
};

/// What a captured stream kept: its byte count and whether it dropped more.
pub const StreamFacts = struct {
    bytes: u32,
    truncated: bool,
};

pub const Summary = struct {
    program_id: [16]u8,
    pid: u32,
    created: u64,
    /// Null while the program runs.
    exit: ?Exit,
    /// stdout and stderr, each present once the program exited and only when it was captured.
    streams: [2]?StreamFacts,
    label: []const u8,
};

/// `StartProgram`'s answer.
pub fn started(allocator: std.mem.Allocator, program_id: [16]u8, pid: u32, created: u64, existing: bool) ![]u8 {
    var id_text: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"existing\":{},\"process\":{{\"creationTime\":\"{d}\",\"pid\":{d}}},\"programId\":\"{s}\"}}",
        .{ existing, created, pid, hex(program_id, &id_text) },
    );
}

/// `RecoverProgram`'s answer.
pub fn recovered(allocator: std.mem.Allocator, program_id: [16]u8, pid: u32, created: u64) ![]u8 {
    var id_text: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"process\":{{\"creationTime\":\"{d}\",\"pid\":{d}}},\"programId\":\"{s}\"}}",
        .{ created, pid, hex(program_id, &id_text) },
    );
}

/// `CancelProgram`'s answer; `outcome` is `cancelling` or `exited`.
pub fn cancelled(allocator: std.mem.Allocator, program_id: [16]u8, outcome: []const u8) ![]u8 {
    var id_text: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"outcome\":\"{s}\",\"programId\":\"{s}\"}}", .{ outcome, hex(program_id, &id_text) });
}

/// `DisposeProgram`'s answer.
pub fn programNamed(allocator: std.mem.Allocator, program_id: [16]u8) ![]u8 {
    var id_text: [32]u8 = undefined;
    return std.fmt.allocPrint(allocator, "{{\"programId\":\"{s}\"}}", .{hex(program_id, &id_text)});
}

/// `ReadProgramOutput`'s answer: `data` from `offset`, base64, and whether the stream ends there.
pub fn output(
    allocator: std.mem.Allocator,
    program_id: [16]u8,
    stream: []const u8,
    offset: u32,
    data: []const u8,
    end: bool,
) ![]u8 {
    std.debug.assert(data.len <= max_output_chunk_bytes);
    var encoded: [std.base64.standard.Encoder.calcSize(max_output_chunk_bytes)]u8 = undefined;
    const text = std.base64.standard.Encoder.encode(&encoded, data);
    var id_text: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"data\":\"{s}\",\"end\":{},\"offset\":{d},\"programId\":\"{s}\",\"stream\":\"{s}\"}}",
        .{ text, end, offset, hex(program_id, &id_text), stream },
    );
}

/// `ProgramExited`: a summary whose `exit` is present.
pub fn exited(allocator: std.mem.Allocator, summary: Summary) ![]u8 {
    var buffer: std.Io.Writer.Allocating = .init(allocator);
    defer buffer.deinit();
    try writeSummary(&buffer.writer, summary, false);
    return buffer.toOwnedSlice();
}

/// `ListPrograms`' answer: up to 16 programs in ascending id order, and `next`, the last listed id,
/// exactly when more follow.
pub fn list(allocator: std.mem.Allocator, programs: []const Summary, more: bool) ![]u8 {
    var buffer: std.Io.Writer.Allocating = .init(allocator);
    defer buffer.deinit();
    const writer = &buffer.writer;
    try writer.writeAll("{");
    if (more and programs.len > 0) {
        var id_text: [32]u8 = undefined;
        try writer.print("\"next\":\"{s}\",", .{hex(programs[programs.len - 1].program_id, &id_text)});
    }
    try writer.writeAll("\"programs\":[");
    for (programs, 0..) |summary, index| {
        if (index != 0) try writer.writeByte(',');
        try writeSummary(writer, summary, true);
    }
    try writer.writeAll("]}");
    return buffer.toOwnedSlice();
}

fn writeSummary(writer: *std.Io.Writer, summary: Summary, listed: bool) !void {
    var id_text: [32]u8 = undefined;
    try writer.writeByte('{');
    if (summary.exit) |exit| switch (exit) {
        .code => |code| try writer.print("\"exit\":{{\"code\":{d}}},", .{code}),
        .cancelled => try writer.writeAll("\"exit\":{\"cancelled\":true},"),
    };
    if (listed) try writer.print("\"label\":\"{s}\",", .{summary.label});
    try writer.print(
        "\"process\":{{\"creationTime\":\"{d}\",\"pid\":{d}}},\"programId\":\"{s}\"",
        .{ summary.created, summary.pid, hex(summary.program_id, &id_text) },
    );
    if (listed) try writer.print(",\"state\":\"{s}\"", .{if (summary.exit == null) "running" else "exited"});
    if (summary.exit != null) {
        if (summary.streams[1]) |facts| try writeStream(writer, "stderr", facts);
        if (summary.streams[0]) |facts| try writeStream(writer, "stdout", facts);
    }
    try writer.writeByte('}');
}

fn writeStream(writer: *std.Io.Writer, name: []const u8, facts: StreamFacts) !void {
    try writer.print(",\"{s}\":{{\"bytes\":{d},\"truncated\":{}}}", .{ name, facts.bytes, facts.truncated });
}

fn hex(bytes: [16]u8, buffer: *[32]u8) []const u8 {
    const digits = "0123456789abcdef";
    for (bytes, 0..) |byte, index| {
        buffer[index * 2] = digits[byte >> 4];
        buffer[index * 2 + 1] = digits[byte & 0x0f];
    }
    return buffer;
}
