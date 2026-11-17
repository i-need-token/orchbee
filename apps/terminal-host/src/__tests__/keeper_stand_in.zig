//! The stand-in Host of the Keeper's real-process test, and its Low-integrity probe. The stand-in
//! holds real terminals on the pinned conpty in kill-on-close Jobs, keeps a synchronous read of
//! each terminal's output pending as a Host's reader does, and opens Keeper connections as itself
//! for the driver to take over; it never sends a frame itself.
//!
//! The driver writes one command per line and reads one answer line:
//! `terminal` starts cmd and answers `terminal` with its seven handle values, the program's and the
//! console's pid and creation time, and the launch name; `connect` answers `connected <handle>`;
//! `probe <n>` waits until terminal n's output is idle, starts a pipe query behind its pending read,
//! and answers whether that query is still blocked; `probe-state <n>` answers the same again;
//! `quit` ends the process.

const std = @import("std");
const platform = @import("terminal-host-platform");
const support = @import("keeper_black_box_support.zig");

const HANDLE = support.HANDLE;
const BOOL = support.BOOL;
const DWORD = support.DWORD;
const monotonic = support.monotonic;

extern "kernel32" fn ReadFile(file: HANDLE, buffer: [*]u8, length: DWORD, read: ?*DWORD, overlapped: ?*anyopaque) callconv(.winapi) BOOL;
extern "kernel32" fn WriteFile(file: HANDLE, buffer: [*]const u8, length: DWORD, written: ?*DWORD, overlapped: ?*anyopaque) callconv(.winapi) BOOL;
extern "kernel32" fn GetNamedPipeInfo(pipe: HANDLE, flags: ?*DWORD, out_size: ?*DWORD, in_size: ?*DWORD, instances: ?*DWORD) callconv(.winapi) BOOL;

/// One terminal the stand-in holds as a Host would: the program in its kill-on-close Job, the
/// pseudoconsole, and a reader that keeps a synchronous read of the output pending.
const Held = struct {
    process: platform.process.Process,
    clock: monotonic.Timer,
    last_read_ns: std.atomic.Value(u64) = .init(0),
    probe_started: bool = false,
    probe_done: std.atomic.Value(bool) = .init(false),

    fn read(self: *Held) void {
        var buffer: [4096]u8 = undefined;
        while (true) {
            var count: DWORD = 0;
            if (!ReadFile(self.process.pty.output, &buffer, buffer.len, &count, null).toBool() or count == 0) return;
            self.last_read_ns.store(self.clock.read(), .monotonic);
        }
    }

    fn probe(self: *Held) void {
        _ = GetNamedPipeInfo(self.process.pty.output, null, null, null, null);
        self.probe_done.store(true, .release);
    }
};

pub fn run(allocator: std.mem.Allocator, io: std.Io, endpoint: []const u8) !void {
    var input_buffer: [256]u8 = undefined;
    var input = std.Io.File.stdin().readerStreaming(io, &input_buffer);
    var output_buffer: [512]u8 = undefined;
    var output = std.Io.File.stdout().writerStreaming(io, &output_buffer);
    const out = &output.interface;
    var terminals: std.ArrayList(*Held) = .empty;
    while (true) {
        // The driver's end of the pipe closing ends the stand-in and every terminal it holds.
        const line = input.interface.takeDelimiterInclusive('\n') catch return;
        const command = std.mem.trim(u8, line, " \r\n");
        if (std.mem.eql(u8, command, "terminal")) {
            const held = try startTerminal(allocator);
            try terminals.append(allocator, held);
            const process = held.process;
            const pseudo_console: *const [3]HANDLE = @ptrCast(@alignCast(process.pty.pseudoConsole()));
            const program = try support.processOf(process.process_handle);
            const console = try support.processOf(pseudo_console[2]);
            var launch_buffer: [64]u8 = undefined;
            const launch = launchName(&launch_buffer, process.launch_kind);
            try out.print("terminal {d} {d} {d} {d} {d} {d} {d} {d} {d} {d} {d} {s}\n", .{
                support.handleValue(process.job_handle),
                support.handleValue(process.process_handle),
                support.handleValue(pseudo_console[2]),
                support.handleValue(pseudo_console[0]),
                support.handleValue(pseudo_console[1]),
                support.handleValue(process.pty.input),
                support.handleValue(process.pty.output),
                program.pid,
                program.created,
                console.pid,
                console.created,
                launch,
            });
        } else if (std.mem.eql(u8, command, "connect")) {
            switch (support.openEndpoint(endpoint, support.GENERIC_READ | support.GENERIC_WRITE, true)) {
                .handle => |handle| try out.print("connected {d}\n", .{@intFromPtr(handle)}),
                .failed => |code| try out.print("failed {d}\n", .{code}),
            }
        } else if (std.mem.startsWith(u8, command, "probe ")) {
            const held = terminals.items[try std.fmt.parseInt(usize, command["probe ".len..], 10)];
            waitIdle(held);
            if (!held.probe_started) {
                held.probe_started = true;
                const thread = try std.Thread.spawn(.{}, Held.probe, .{held});
                thread.detach();
            }
            monotonic.sleep(300 * std.time.ns_per_ms);
            try out.print("probe {s}\n", .{if (held.probe_done.load(.acquire)) "completed" else "blocked"});
        } else if (std.mem.startsWith(u8, command, "probe-state ")) {
            const held = terminals.items[try std.fmt.parseInt(usize, command["probe-state ".len..], 10)];
            try out.print("probe {s}\n", .{if (held.probe_done.load(.acquire)) "completed" else "blocked"});
        } else if (std.mem.eql(u8, command, "quit")) {
            std.process.exit(0);
        } else {
            try out.print("unknown\n", .{});
        }
        try out.flush();
    }
}

fn startTerminal(allocator: std.mem.Allocator) !*Held {
    const held = try allocator.create(Held);
    held.* = .{
        .process = try platform.process.Process.spawn(allocator, .{
            .arguments = &.{},
            .columns = 80,
            .rows = 25,
            .current_directory = null,
            .environment = &.{},
            .executable = "cmd.exe",
        }),
        .clock = try monotonic.Timer.start(),
    };
    const reader = try std.Thread.spawn(.{}, Held.read, .{held});
    reader.detach();
    return held;
}

/// Waits until the terminal has written nothing for half a second, so its reader's synchronous
/// read is the one pending on the output pipe.
fn waitIdle(held: *Held) void {
    const quiet_ns = 500 * std.time.ns_per_ms;
    const deadline_ns = held.clock.read() + 10 * std.time.ns_per_s;
    while (held.clock.read() < deadline_ns) {
        const last = held.last_read_ns.load(.monotonic);
        if (last != 0 and held.clock.read() - last >= quiet_ns) return;
        monotonic.sleep(50 * std.time.ns_per_ms);
    }
}

fn launchName(buffer: *[64]u8, kind: platform.process.LaunchKind) []const u8 {
    const tag = @tagName(kind);
    for (tag, 0..) |byte, index| buffer[index] = if (byte == '_') '-' else byte;
    return buffer[0..tag.len];
}

/// The Low-integrity probe: whether this process can open the endpoint for writing, open it for
/// reading, and write a frame through a read-only open. Bit 0: the read-write open was denied;
/// bit 1: the read-only open was denied; bit 2: the write through the read-only open failed;
/// bit 4: the read-write open succeeded; bit 5: a frame was written.
pub fn lowProbe(endpoint: []const u8) u8 {
    var result: u8 = 0;
    switch (support.openEndpoint(endpoint, support.GENERIC_READ | support.GENERIC_WRITE, false)) {
        .handle => |handle| {
            result |= 0x10;
            _ = support.CloseHandle(handle);
        },
        .failed => |code| if (code == support.ERROR_ACCESS_DENIED) {
            result |= 0x1;
        },
    }
    switch (support.openEndpoint(endpoint, support.GENERIC_READ, false)) {
        .handle => |handle| {
            const frame = "OBHS\x00\x00\x00\x02{}";
            var written: DWORD = 0;
            if (WriteFile(handle, frame, frame.len, &written, null).toBool() and written > 0) result |= 0x20 else result |= 0x4;
            _ = support.CloseHandle(handle);
        },
        .failed => |code| if (code == support.ERROR_ACCESS_DENIED) {
            result |= 0x2;
        },
    }
    return result;
}
