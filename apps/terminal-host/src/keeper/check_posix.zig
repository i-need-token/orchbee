//! The check run of a Keeper update: before it executes another program in its place, the
//! Keeper runs that program as `keep --check` with a copy of the state on its standard input, and
//! keeps its own image unless the child exits 0 within 5 seconds. `serve` is that check mode: it
//! reads the state and exits 0 only when it can take it over.

const std = @import("std");
const platform = @import("terminal-host-platform");
const saved = @import("saved_posix.zig");

const posix = std.posix;
const c = std.c;
const native = platform.keeper.native;
const posix_compat = platform.posix_compat;
const monotonic = platform.monotonic;

const deadline_ns = 5 * std.time.ns_per_s;
const wait_slice_ns = 5 * std.time.ns_per_ms;
const WNOHANG: c_int = 1;

/// Whether `program` reads `state` in its check mode within the deadline. A child still running at
/// the deadline is ended and reaped.
pub fn passes(program: [:0]const u8, state: []const u8) bool {
    var timer = monotonic.Timer.start() catch return false;
    // A child that exits before it read everything fails the check by its status, not by a signal.
    const channel = native.checkChannel() orelse return false;
    const child = native.spawnCheck(program.ptr, channel.read_end);
    posix_compat.close(channel.read_end);
    const pid = child orelse {
        posix_compat.close(channel.write_end);
        return false;
    };
    const fed = feed(channel.write_end, state, &timer);
    // The end of input, so the child decides on what it read.
    posix_compat.close(channel.write_end);
    return awaitExit(pid, &timer) and fed;
}

/// Writes all of `state` before the deadline; false when the child stopped reading.
fn feed(descriptor: posix.fd_t, state: []const u8, timer: *monotonic.Timer) bool {
    var written: usize = 0;
    while (written < state.len) {
        if (native.checkWrite(descriptor, state[written..])) |count| {
            written += count;
            continue;
        } else |err| if (err != error.WouldBlock) return false;
        const elapsed = timer.read();
        if (elapsed >= deadline_ns) return false;
        var ready = [_]posix.pollfd{.{ .fd = descriptor, .events = posix.POLL.OUT, .revents = 0 }};
        const remaining_ms: i32 = @intCast(@max(1, @min(1000, (deadline_ns - elapsed) / std.time.ns_per_ms)));
        _ = posix.poll(&ready, remaining_ms) catch return false;
        if (ready[0].revents & (posix.POLL.ERR | posix.POLL.HUP | posix.POLL.NVAL) != 0) return false;
    }
    return true;
}

/// Whether `child` exited 0 before the deadline; otherwise it is ended. It is reaped either way.
fn awaitExit(child: c.pid_t, timer: *monotonic.Timer) bool {
    while (true) {
        var status: c_int = 0;
        const reaped = c.waitpid(child, &status, WNOHANG);
        if (reaped == child) return posix.W.IFEXITED(@bitCast(status)) and posix.W.EXITSTATUS(@bitCast(status)) == 0;
        if (reaped < 0 and posix.errno(reaped) != .INTR) return false;
        if (timer.read() >= deadline_ns) {
            _ = c.kill(child, .KILL);
            while (true) {
                const ended = c.waitpid(child, &status, 0);
                if (ended >= 0 or posix.errno(ended) != .INTR) return false;
            }
        }
        monotonic.sleep(wait_slice_ns);
    }
}

/// `keep --check`: reads a state from standard input and exits 0 when this build can take it over.
pub fn serve(allocator: std.mem.Allocator) noreturn {
    var arena_state = std.heap.ArenaAllocator.init(allocator);
    const arena = arena_state.allocator();
    const bytes = arena.alloc(u8, saved.maximum_bytes + 1) catch std.process.exit(1);
    var filled: usize = 0;
    while (filled < bytes.len) {
        const count = c.read(0, bytes[filled..].ptr, bytes.len - filled);
        if (count == 0) break;
        if (count < 0) {
            if (posix.errno(count) == .INTR) continue;
            std.process.exit(1);
        }
        filled += @intCast(count);
    }
    _ = saved.decode(arena, bytes[0..filled]) catch std.process.exit(1);
    std.process.exit(0);
}
