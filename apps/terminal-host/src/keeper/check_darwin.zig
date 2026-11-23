//! The check run of a macOS Keeper update: before it executes another program in its place, the
//! Keeper runs that program as `keep --check` with a copy of the state on its standard input, and
//! keeps its own image unless the child exits 0 within 5 seconds. `serve` is that check mode: it
//! reads the state and exits 0 only when it can take it over.

const std = @import("std");
const platform = @import("terminal-host-platform");
const saved = @import("saved_darwin.zig");

const posix = std.posix;
const c = std.c;
const posix_compat = platform.posix_compat;
const monotonic = platform.monotonic;

const deadline_ns = 5 * std.time.ns_per_s;
const wait_slice_ns = 5 * std.time.ns_per_ms;
const F_SETNOSIGPIPE: c_int = 73;
const O_RDONLY: c_int = 0;
const O_NONBLOCK: c_int = 0x0004;
const WNOHANG: c_int = 1;

/// Whether `program` reads `state` in its check mode within the deadline. A child still running at
/// the deadline is ended and reaped.
pub fn passes(program: [:0]const u8, state: []const u8) bool {
    var timer = monotonic.Timer.start() catch return false;
    var ends: [2]posix.fd_t = undefined;
    if (c.pipe(&ends) != 0) return false;
    var read_end: ?posix.fd_t = ends[0];
    defer if (read_end) |descriptor| posix_compat.close(descriptor);
    const write_end = ends[1];
    defer posix_compat.close(write_end);
    for (ends) |descriptor| _ = posix_compat.fcntl(descriptor, posix.F.SETFD, posix.FD_CLOEXEC) catch return false;
    // A child that exits before it read everything fails the check by its status, not by a signal.
    _ = posix_compat.fcntl(write_end, F_SETNOSIGPIPE, 1) catch return false;
    const flags = posix_compat.fcntl(write_end, posix.F.GETFL, 0) catch return false;
    _ = posix_compat.fcntl(write_end, posix.F.SETFL, flags | O_NONBLOCK) catch return false;

    const child = spawn(program, ends[0]) orelse return false;
    posix_compat.close(ends[0]);
    read_end = null;
    const fed = feed(write_end, state, &timer);
    posix_compat.close(write_end);
    return awaitExit(child, &timer) and fed;
}

fn spawn(program: [:0]const u8, input: posix.fd_t) ?c.pid_t {
    var actions: c.posix_spawn_file_actions_t = undefined;
    if (c.posix_spawn_file_actions_init(&actions) != 0) return null;
    defer _ = c.posix_spawn_file_actions_destroy(&actions);
    var attributes: c.posix_spawnattr_t = undefined;
    if (c.posix_spawnattr_init(&attributes) != 0) return null;
    defer _ = c.posix_spawnattr_destroy(&attributes);
    // Only the three standard streams pass to the child.
    if (c.posix_spawnattr_setflags(&attributes, .{ .CLOEXEC_DEFAULT = true }) != 0) return null;
    if (c.posix_spawn_file_actions_adddup2(&actions, input, 0) != 0) return null;
    for ([_]posix.fd_t{ 1, 2 }) |stream| {
        if (c.posix_spawn_file_actions_addopen(&actions, stream, "/dev/null", O_RDONLY, 0) != 0) return null;
    }
    const argv = [_:null]?[*:0]const u8{ program.ptr, "keep", "--check" };
    var pid: c.pid_t = 0;
    if (c.posix_spawn(&pid, program.ptr, &actions, &attributes, &argv, @ptrCast(c.environ)) != 0) return null;
    return pid;
}

/// Writes all of `state` before the deadline; false when the child stopped reading.
fn feed(descriptor: posix.fd_t, state: []const u8, timer: *monotonic.Timer) bool {
    var written: usize = 0;
    while (written < state.len) {
        const count = c.write(descriptor, state[written..].ptr, state.len - written);
        if (count > 0) {
            written += @intCast(count);
            continue;
        }
        if (posix.errno(count) != .AGAIN) return false;
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
