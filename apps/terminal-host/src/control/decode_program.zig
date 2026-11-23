//! The `background-programs` capability's requests on `worker-host`. A program id, a holding, and a
//! mark are 32 lowercase hexadecimal digits, not all zero. A label is at most 2,048 characters of
//! the base64 alphabet, which the Host returns but never reads.

const std = @import("std");
const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");

pub const max_label_bytes = 2048;
pub const max_environment_entries = 1024;
pub const max_environment_text_bytes = 32 * 1024;
pub const max_arguments = 4096;
pub const max_text_bytes = 32 * 1024;

pub const Variable = struct {
    name: []const u8,
    value: ?[]const u8,
};

/// `StartProgram`; every slice borrows the parse until `deinit`.
pub const Start = struct {
    program_id: [16]u8,
    executable: []const u8,
    arguments: []const []const u8,
    raw_argument: ?[]const u8,
    directory: []const u8,
    environment: []const Variable,
    /// Whether stdout and stderr reach the Host instead of the null device.
    capture: [2]bool,
    label: []const u8,
};

pub const Stream = enum(u1) { stdout, stderr };

pub const ReadOutput = struct {
    program_id: [16]u8,
    stream: Stream,
    offset: u32,
};

pub const Recover = struct {
    program_id: [16]u8,
    holding: [16]u8,
    mark: [16]u8,
    deadline_seconds: u8,
};

const WireCapture = struct {
    stdout: bool,
    stderr: bool,
};

const WireStart = struct {
    arguments: []const []const u8,
    capture: WireCapture,
    directory: []const u8,
    environment: []const Variable,
    executable: []const u8,
    label: []const u8,
    programId: []const u8,
    rawArgument: ?[]const u8 = null,
};

const WireProgram = struct {
    programId: []const u8,
};

const WireReadOutput = struct {
    offset: u32,
    programId: []const u8,
    stream: []const u8,
};

const WireList = struct {
    after: ?[]const u8 = null,
};

const WireRecover = struct {
    deadlineSeconds: u8,
    holding: []const u8,
    mark: []const u8,
    programId: []const u8,
};

pub fn decodeStart(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireStart, Start) {
    const parsed = try control_wire.decode(WireStart, allocator, payload);
    errdefer parsed.deinit();
    const wire = parsed.value;
    if (wire.arguments.len > max_arguments or wire.environment.len > max_environment_entries)
        return error.InvalidField;
    try text(wire.executable);
    try text(wire.directory);
    for (wire.arguments) |argument| try argumentText(argument);
    if (wire.rawArgument) |raw| try argumentText(raw);
    try environment(wire.environment);
    try label(wire.label);
    return .{ .parsed = parsed, .value = .{
        .program_id = try id(wire.programId),
        .executable = wire.executable,
        .arguments = wire.arguments,
        .raw_argument = wire.rawArgument,
        .directory = wire.directory,
        .environment = wire.environment,
        .capture = .{ wire.capture.stdout, wire.capture.stderr },
        .label = wire.label,
    } };
}

/// `CancelProgram` and `DisposeProgram`.
pub fn decodeProgram(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireProgram, [16]u8) {
    const parsed = try control_wire.decode(WireProgram, allocator, payload);
    errdefer parsed.deinit();
    return .{ .parsed = parsed, .value = try id(parsed.value.programId) };
}

pub fn decodeReadOutput(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireReadOutput, ReadOutput) {
    const parsed = try control_wire.decode(WireReadOutput, allocator, payload);
    errdefer parsed.deinit();
    const stream: Stream = if (std.mem.eql(u8, parsed.value.stream, "stdout"))
        .stdout
    else if (std.mem.eql(u8, parsed.value.stream, "stderr"))
        .stderr
    else
        return error.InvalidField;
    return .{ .parsed = parsed, .value = .{
        .program_id = try id(parsed.value.programId),
        .stream = stream,
        .offset = parsed.value.offset,
    } };
}

/// `ListPrograms`: the program id after which the page starts, or null for the first page.
pub fn decodeList(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireList, ?[16]u8) {
    const parsed = try control_wire.decode(WireList, allocator, payload);
    errdefer parsed.deinit();
    return .{ .parsed = parsed, .value = if (parsed.value.after) |after| try id(after) else null };
}

pub fn decodeRecover(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(WireRecover, Recover) {
    const parsed = try control_wire.decode(WireRecover, allocator, payload);
    errdefer parsed.deinit();
    const deadline = parsed.value.deadlineSeconds;
    if (deadline < 1 or deadline > 30) return error.InvalidField;
    return .{ .parsed = parsed, .value = .{
        .program_id = try id(parsed.value.programId),
        .holding = try id(parsed.value.holding),
        .mark = try id(parsed.value.mark),
        .deadline_seconds = deadline,
    } };
}

/// 32 lowercase hexadecimal digits, not all zero.
pub fn id(value: []const u8) error{InvalidIdentifier}![16]u8 {
    if (value.len != 32) return error.InvalidIdentifier;
    var bytes: [16]u8 = undefined;
    for (&bytes, 0..) |*byte, index| {
        const high = nibble(value[index * 2]) orelse return error.InvalidIdentifier;
        const low = nibble(value[index * 2 + 1]) orelse return error.InvalidIdentifier;
        byte.* = (high << 4) | low;
    }
    if (std.mem.allEqual(u8, &bytes, 0)) return error.InvalidIdentifier;
    return bytes;
}

fn nibble(digit: u8) ?u8 {
    return switch (digit) {
        '0'...'9' => digit - '0',
        'a'...'f' => digit - 'a' + 10,
        else => null,
    };
}

fn text(value: []const u8) error{InvalidField}!void {
    if (value.len == 0 or value.len > max_text_bytes or std.mem.indexOfScalar(u8, value, 0) != null)
        return error.InvalidField;
}

/// An argument may be empty, as an empty argument is.
fn argumentText(value: []const u8) error{InvalidField}!void {
    if (value.len > max_text_bytes or std.mem.indexOfScalar(u8, value, 0) != null) return error.InvalidField;
}

fn label(value: []const u8) error{InvalidField}!void {
    if (value.len > max_label_bytes) return error.InvalidField;
    for (value) |byte| switch (byte) {
        'A'...'Z', 'a'...'z', '0'...'9', '+', '/', '=' => {},
        else => return error.InvalidField,
    };
}

/// Names are unique ignoring ASCII case, as Windows compares them, and hold no `=` or NUL.
fn environment(variables: []const Variable) error{InvalidField}!void {
    for (variables, 0..) |variable, index| {
        const name = variable.name;
        if (name.len == 0 or name.len > max_environment_text_bytes or
            std.mem.indexOfAny(u8, name, "=\x00") != null) return error.InvalidField;
        if (variable.value) |value| if (value.len > max_environment_text_bytes or
            std.mem.indexOfScalar(u8, value, 0) != null) return error.InvalidField;
        for (variables[0..index]) |earlier| {
            if (std.ascii.eqlIgnoreCase(earlier.name, name)) return error.InvalidField;
        }
    }
}
