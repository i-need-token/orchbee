//! The one record a fatal Host exit leaves, and the root panic and processor-exception handlers
//! that write it. Losing the Host ends every program it owns, so the Worker that loses one needs
//! the site and reason class of the fault; it never needs, and never gets, program output,
//! titles, paths or peer bytes. A panic message can be formatted from any of those, so only its
//! class is kept. The record is closed: the keys and value shapes below are the whole format.
//!
//! The executable's root installs `panic` as its panic namespace and `handleSegfault` as
//! `debug.handleSegfault`. Both write the record, when one is armed, and end the process with
//! `exit_code`, or with the code of a fatal exit already recorded. A fault before `arm` still exits
//! with that code but leaves no record, because no runtime directory is known to belong to this
//! Host yet. Only the faults these handlers receive are recorded: a fail-fast exit, an exception
//! the standard handler passes on, `abort`, or an external kill leaves no record.
const std = @import("std");
const record_file = @import("record_file.zig");

pub const file_name = "host-fatal.json";
/// The exit code of a Host that panicked or took a processor exception.
pub const exit_code: u8 = 16;
pub const format_version = 1;
pub const max_record_bytes = 512;
/// How long a second faulting thread waits for the first to finish the record before it ends
/// the process itself.
const concurrent_fault_grace_ms = 2000;
const max_error_name_bytes = 64;

/// The thread a fault happened on. A thread names its kind when it starts; any thread that does
/// not, including runtime-library threads, is `other`.
pub const ThreadKind = enum {
    dispatch,
    @"control-lane",
    @"terminal-output",
    @"terminal-input",
    @"terminal-wait",
    @"terminal-animation",
    other,
};

pub const Kind = enum {
    /// A safety check or an explicit panic.
    panic,
    /// A processor exception the standard handler routes: an access violation, misaligned access,
    /// illegal instruction or stack overflow on Windows; `SIGSEGV`, `SIGILL`, `SIGBUS` or `SIGFPE`
    /// elsewhere.
    exception,
    /// A serving or shutdown failure the Host returned from `main`.
    exit,
};

pub const Reason = enum {
    @"explicit-panic",
    @"sentinel-mismatch",
    @"unwrap-error",
    @"index-out-of-bounds",
    @"start-greater-than-end",
    @"inactive-union-field",
    @"slice-cast-remainder",
    @"reached-unreachable",
    @"unwrap-null",
    @"cast-to-null",
    @"incorrect-alignment",
    @"invalid-error-code",
    @"integer-out-of-bounds",
    @"integer-overflow",
    @"shl-overflow",
    @"shr-overflow",
    @"divide-by-zero",
    @"exact-division-remainder",
    @"integer-part-out-of-bounds",
    @"corrupt-switch",
    @"shift-rhs-too-big",
    @"invalid-enum-value",
    @"for-len-mismatch",
    @"copy-len-mismatch",
    @"memcpy-alias",
    @"noreturn-returned",
    @"segmentation-fault",
    @"illegal-instruction",
    @"unaligned-access",
    @"stack-overflow",
    @"bus-error",
    @"arithmetic-exception",
    @"general-protection",
    @"other-exception",
    /// The serve loop or its teardown returned an error; `error` names it.
    @"serving-failed",
};

pub const Record = struct {
    kind: Kind,
    reason: Reason,
    /// A source-defined error name, for `unwrap-error` and `exit` records only. Anything other
    /// than a plain identifier is left out.
    error_name: ?[]const u8 = null,
    thread: ThreadKind,
    /// The faulting return or instruction address as an offset into the executable's image;
    /// null when the address is unknown or lies in another module.
    module_offset: ?usize,
    process_id: u32,
    unix_milliseconds: u64,
};

/// Encodes `record` as one line of JSON:
/// `{"format":1,"role":"terminal-host","kind":"panic","reason":"integer-overflow","error":null,
/// "thread":"dispatch","module_offset":4198400,"process_id":4242,"time_unix_ms":1763100000000}`.
pub fn encode(buffer: *[max_record_bytes]u8, record: Record) error{WriteFailed}![]const u8 {
    var writer: std.Io.Writer = .fixed(buffer);
    try writer.print(
        "{{\"format\":{d},\"role\":\"terminal-host\",\"kind\":\"{s}\",\"reason\":\"{s}\",\"error\":",
        .{ format_version, @tagName(record.kind), @tagName(record.reason) },
    );
    if (plainIdentifier(record.error_name)) |name| {
        try writer.print("\"{s}\"", .{name});
    } else {
        try writer.writeAll("null");
    }
    try writer.print(",\"thread\":\"{s}\",\"module_offset\":", .{@tagName(record.thread)});
    if (record.module_offset) |offset| try writer.print("{d}", .{offset}) else try writer.writeAll("null");
    try writer.print(
        ",\"process_id\":{d},\"time_unix_ms\":{d}}}\n",
        .{ record.process_id, record.unix_milliseconds },
    );
    return writer.buffered();
}

fn plainIdentifier(name: ?[]const u8) ?[]const u8 {
    const value = name orelse return null;
    if (value.len == 0 or value.len > max_error_name_bytes) return null;
    for (value) |byte| switch (byte) {
        'a'...'z', 'A'...'Z', '0'...'9', '_' => {},
        else => return null,
    };
    return value;
}

threadlocal var current_thread: ThreadKind = .other;
/// Set while this thread handles a fault, so a fault inside the handler ends the process at once.
threadlocal var handling_fault = false;
/// Written once by `arm` before `armed` is published; read by any faulting thread after it.
var armed_path: record_file.Path = .{};
var executable: record_file.ExecutableRange = .{};
var armed = std.atomic.Value(bool).init(false);
/// The first fault or fatal exit claims the one record.
var record_claimed = std.atomic.Value(bool).init(false);
/// The code a fault exits with: `exit_code`, or the code of a fatal exit already recorded, so a
/// fault in that exit's teardown does not hide why the Host was ending.
var kept_exit_code = std.atomic.Value(u8).init(exit_code);
/// Set once a fatal exit's record is written; a later fault then has nothing to wait for.
var exit_recorded = std.atomic.Value(bool).init(false);

/// Names the calling thread's kind for any record written on it.
pub fn markThread(kind: ThreadKind) void {
    current_thread = kind;
}

/// Arms the record in `runtime_directory`, which this Host must already hold exclusively. Called
/// once, by the dispatch thread, before the Host starts any other thread.
pub fn arm(runtime_directory: []const u8) void {
    if (armed.load(.acquire)) return;
    armed_path = record_file.Path.init(runtime_directory, file_name) orelse return;
    executable = record_file.ExecutableRange.capture();
    armed.store(true, .release);
}

/// Writes the record for a serving or shutdown failure that ends the Host through `main`, while
/// the runtime directory is still held. The caller then exits with `code`, and a fault in the
/// teardown before that exit uses `code` as well.
pub fn recordExit(reason: Reason, err: anyerror, code: u8) void {
    if (record_claimed.swap(true, .acq_rel)) return;
    kept_exit_code.store(code, .release);
    writeRecord(.exit, reason, @errorName(err), null);
    exit_recorded.store(true, .release);
}

fn writeRecord(kind: Kind, reason: Reason, error_name: ?[]const u8, address: ?usize) void {
    if (!armed.load(.acquire)) return;
    var buffer: [max_record_bytes]u8 = undefined;
    const bytes = encode(&buffer, .{
        .kind = kind,
        .reason = reason,
        .error_name = error_name,
        .thread = current_thread,
        .module_offset = if (address) |value| executable.offset(value) else null,
        .process_id = record_file.processId(),
        .unix_milliseconds = record_file.unixMilliseconds(),
    }) catch return;
    _ = armed_path.write(bytes, .replace);
}

fn fail(
    kind: Kind,
    reason: Reason,
    error_name: ?[]const u8,
    address: usize,
    context: ?std.debug.CpuContextPtr,
) noreturn {
    @branchHint(.cold);
    if (handling_fault) record_file.exitNow(kept_exit_code.load(.acquire));
    handling_fault = true;
    if (record_claimed.swap(true, .acq_rel)) {
        // Another fault owns the record and ends the process once it is written; a recorded
        // fatal exit is already written and keeps its own code.
        if (!exit_recorded.load(.acquire)) record_file.waitMilliseconds(concurrent_fault_grace_ms);
        record_file.exitNow(kept_exit_code.load(.acquire));
    }
    writeRecord(kind, reason, error_name, address);
    if (record_file.stderrAttached()) reportToStderr(kind, reason, address, context);
    record_file.exitNow(exit_code);
}

/// A developer who runs the Host from a console still sees the class and a stack trace. A Host
/// its Worker started has no standard error, so this is skipped there.
fn reportToStderr(
    kind: Kind,
    reason: Reason,
    address: usize,
    context: ?std.debug.CpuContextPtr,
) void {
    var buffer: [160]u8 = undefined;
    const line = std.fmt.bufPrint(&buffer, "orchbee-terminal-host: fatal {s} {s} on the {s} thread\n", .{
        @tagName(kind), @tagName(reason), @tagName(current_thread),
    }) catch return;
    record_file.writeStderr(line);
    std.Options.debug_io.vtable.crashHandler(std.Options.debug_io.userdata);
    std.debug.dumpCurrentStackTrace(.{
        .first_address = if (context == null) address else null,
        .context = context,
        .allow_unsafe_unwind = true,
    });
}

/// The root `panic` namespace. Every safety check maps to its class; an explicit panic keeps
/// only `explicit-panic`, because its message may carry any bytes.
pub const panic = struct {
    pub fn call(_: []const u8, return_address: ?usize) noreturn {
        fail(.panic, .@"explicit-panic", null, return_address orelse @returnAddress(), null);
    }
    pub fn sentinelMismatch(expected: anytype, found: @TypeOf(expected)) noreturn {
        _ = .{ expected, found };
        fail(.panic, .@"sentinel-mismatch", null, @returnAddress(), null);
    }
    pub fn unwrapError(err: anyerror) noreturn {
        fail(.panic, .@"unwrap-error", @errorName(err), @returnAddress(), null);
    }
    pub fn outOfBounds(_: usize, _: usize) noreturn {
        fail(.panic, .@"index-out-of-bounds", null, @returnAddress(), null);
    }
    pub fn startGreaterThanEnd(_: usize, _: usize) noreturn {
        fail(.panic, .@"start-greater-than-end", null, @returnAddress(), null);
    }
    pub fn inactiveUnionField(active: anytype, accessed: @TypeOf(active)) noreturn {
        _ = .{ active, accessed };
        fail(.panic, .@"inactive-union-field", null, @returnAddress(), null);
    }
    pub fn sliceCastLenRemainder(_: usize) noreturn {
        fail(.panic, .@"slice-cast-remainder", null, @returnAddress(), null);
    }
    pub fn reachedUnreachable() noreturn {
        fail(.panic, .@"reached-unreachable", null, @returnAddress(), null);
    }
    pub fn unwrapNull() noreturn {
        fail(.panic, .@"unwrap-null", null, @returnAddress(), null);
    }
    pub fn castToNull() noreturn {
        fail(.panic, .@"cast-to-null", null, @returnAddress(), null);
    }
    pub fn incorrectAlignment() noreturn {
        fail(.panic, .@"incorrect-alignment", null, @returnAddress(), null);
    }
    pub fn invalidErrorCode() noreturn {
        fail(.panic, .@"invalid-error-code", null, @returnAddress(), null);
    }
    pub fn integerOutOfBounds() noreturn {
        fail(.panic, .@"integer-out-of-bounds", null, @returnAddress(), null);
    }
    pub fn integerOverflow() noreturn {
        fail(.panic, .@"integer-overflow", null, @returnAddress(), null);
    }
    pub fn shlOverflow() noreturn {
        fail(.panic, .@"shl-overflow", null, @returnAddress(), null);
    }
    pub fn shrOverflow() noreturn {
        fail(.panic, .@"shr-overflow", null, @returnAddress(), null);
    }
    pub fn divideByZero() noreturn {
        fail(.panic, .@"divide-by-zero", null, @returnAddress(), null);
    }
    pub fn exactDivisionRemainder() noreturn {
        fail(.panic, .@"exact-division-remainder", null, @returnAddress(), null);
    }
    pub fn integerPartOutOfBounds() noreturn {
        fail(.panic, .@"integer-part-out-of-bounds", null, @returnAddress(), null);
    }
    pub fn corruptSwitch() noreturn {
        fail(.panic, .@"corrupt-switch", null, @returnAddress(), null);
    }
    pub fn shiftRhsTooBig() noreturn {
        fail(.panic, .@"shift-rhs-too-big", null, @returnAddress(), null);
    }
    pub fn invalidEnumValue() noreturn {
        fail(.panic, .@"invalid-enum-value", null, @returnAddress(), null);
    }
    pub fn forLenMismatch() noreturn {
        fail(.panic, .@"for-len-mismatch", null, @returnAddress(), null);
    }
    pub fn copyLenMismatch() noreturn {
        fail(.panic, .@"copy-len-mismatch", null, @returnAddress(), null);
    }
    pub fn memcpyAlias() noreturn {
        fail(.panic, .@"memcpy-alias", null, @returnAddress(), null);
    }
    pub fn noreturnReturned() noreturn {
        fail(.panic, .@"noreturn-returned", null, @returnAddress(), null);
    }
};

/// The root `debug.handleSegfault` override: the standard library's exception handler (a
/// vectored handler on Windows, signal handlers elsewhere) calls it with one of its fixed names.
pub fn handleSegfault(
    address: ?usize,
    name: []const u8,
    context: ?std.debug.CpuContextPtr,
) noreturn {
    _ = address;
    const instruction: usize = if (comptime std.debug.CpuContextPtr == noreturn)
        @returnAddress()
    else if (context) |registers| @intCast(registers.getPc()) else @returnAddress();
    fail(.exception, exceptionReason(name), null, instruction, context);
}

fn exceptionReason(name: []const u8) Reason {
    const known = [_]struct { []const u8, Reason }{
        .{ "Segmentation fault", .@"segmentation-fault" },
        .{ "Illegal instruction", .@"illegal-instruction" },
        .{ "Unaligned memory access", .@"unaligned-access" },
        .{ "Stack overflow", .@"stack-overflow" },
        .{ "Bus error", .@"bus-error" },
        .{ "Arithmetic exception", .@"arithmetic-exception" },
        .{ "General protection exception", .@"general-protection" },
    };
    for (known) |entry| if (std.mem.eql(u8, entry[0], name)) return entry[1];
    return .@"other-exception";
}
