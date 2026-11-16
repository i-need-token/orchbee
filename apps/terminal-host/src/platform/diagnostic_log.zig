//! The Host's diagnostic log: closed JSON lines in `<runtime_directory>/host-diagnostics.jsonl`,
//! for faults the Host survives. A line names its event and carries numbers only; never program
//! output, titles, paths or peer bytes. The file starts over when it would pass
//! `max_file_bytes`, so it stays bounded for the life of the Session.
//!
//! The dispatch thread owns the log: it arms it and writes every line. Other threads only count,
//! through `noteDroppedTerminalReply`, and the dispatch thread records the count when it next
//! maintains its terminals.
const std = @import("std");
const monotonic = @import("monotonic.zig");
const record_file = @import("record_file.zig");

pub const file_name = "host-diagnostics.jsonl";
pub const max_file_bytes: u32 = 256 * 1024;
/// A dispatch turn longer than this is recorded.
pub const slow_turn_threshold_ns: u64 = 250 * std.time.ns_per_ms;
/// Each slow-turn kind, the dropped-reply count, accept failures, refused runtime observations,
/// and deferred event-delivery passes are recorded at most once per window.
pub const rate_window_ns: u64 = 60 * std.time.ns_per_s;
const max_line_bytes = 320;
const rate_slots = 32;

/// One dispatch turn: a Worker request of the given message kind, or one pass of event delivery.
pub const Turn = union(enum) {
    request: u16,
    events,

    fn key(self: Turn) u32 {
        return switch (self) {
            .request => |kind| kind,
            .events => 0x1_0000,
        };
    }
};

const RateSlot = struct {
    key: u32,
    recorded_at_ns: u64,
};

/// One event written at most once per window; its next line counts the events since the
/// previous line that were not written, so a repeating fault cannot fill the log.
const Windowed = struct {
    recorded_at_ns: ?u64 = null,
    suppressed: u64 = 0,

    /// Without a clock every event is written; the file's own bound still holds.
    fn allows(self: *Windowed, now: ?u64) bool {
        const current = now orelse return true;
        const recorded_at = self.recorded_at_ns orelse return true;
        if (current -| recorded_at >= rate_window_ns) return true;
        self.suppressed +|= 1;
        return false;
    }

    fn recorded(self: *Windowed, now: ?u64) void {
        self.recorded_at_ns = now;
        self.suppressed = 0;
    }
};

var path: ?record_file.Path = null;
var clock: ?monotonic.Timer = null;
var slow_turns: [rate_slots]?RateSlot = @splat(null);
var dropped_replies = std.atomic.Value(u64).init(0);
var recorded_dropped_replies: u64 = 0;
var dropped_recorded_at_ns: ?u64 = null;
var accept_failures: Windowed = .{};
var observation_refusals: Windowed = .{};
var deferred_event_passes: Windowed = .{};

/// Arms the log in `runtime_directory`, which this Host must already hold exclusively.
pub fn arm(runtime_directory: []const u8) void {
    if (path != null) return;
    clock = monotonic.Timer.start() catch null;
    path = record_file.Path.init(runtime_directory, file_name);
}

/// An accept failure the serve loop survives.
pub const AcceptFailure = struct {
    /// The Host's own error name.
    error_name: []const u8,
    /// The accept's operating-system code, where the platform keeps one.
    system_code: ?u32,
    /// The code of a failed endpoint replacement; null when it was replaced or never attempted.
    renew_system_code: ?u32,
    /// The wait before the next attempt.
    retry_after_ms: u32,
};

/// Records an accept failure, at most once per window; a line counts the failures since the
/// previous line that were not written, so a failing endpoint cannot fill the log.
pub fn acceptFailure(failure: AcceptFailure) void {
    if (path == null) return;
    const now = nowNs();
    if (!accept_failures.allows(now)) return;
    var buffer: [max_line_bytes]u8 = undefined;
    var writer: std.Io.Writer = .fixed(&buffer);
    writer.print(
        "{{\"time_unix_ms\":{d},\"event\":\"accept-failure\",\"error\":\"{s}\",\"system_code\":",
        .{ record_file.unixMilliseconds(), identifierOrUnnamed(failure.error_name) },
    ) catch return;
    writeOptionalCode(&writer, failure.system_code) catch return;
    writer.writeAll(",\"renew_system_code\":") catch return;
    writeOptionalCode(&writer, failure.renew_system_code) catch return;
    writer.print(
        ",\"retry_after_ms\":{d},\"suppressed\":{d}}}\n",
        .{ failure.retry_after_ms, accept_failures.suppressed },
    ) catch return;
    accept_failures.recorded(now);
    writeLine(writer.buffered());
}

/// Records a terminal the Host failed and closed while its other terminals kept running.
/// `failure` is the runtime failure class the Worker also receives.
pub fn terminalFailed(failure: []const u8) void {
    if (path == null) return;
    var buffer: [max_line_bytes]u8 = undefined;
    const line = std.fmt.bufPrint(
        &buffer,
        "{{\"time_unix_ms\":{d},\"event\":\"terminal-failed\",\"failure\":\"{s}\"}}\n",
        .{ record_file.unixMilliseconds(), identifierOrUnnamed(failure) },
    ) catch return;
    writeLine(line);
}

/// Records a runtime observation Core refused, at most once per window. `error` is Core's
/// refusal and `phase` the terminal's lifecycle phase, which decides whether the terminal was
/// closed or kept its last accepted observation.
pub fn observationRefused(error_name: []const u8, phase: []const u8) void {
    if (path == null) return;
    const now = nowNs();
    if (!observation_refusals.allows(now)) return;
    var buffer: [max_line_bytes]u8 = undefined;
    const line = std.fmt.bufPrint(
        &buffer,
        "{{\"time_unix_ms\":{d},\"event\":\"observation-refused\",\"error\":\"{s}\",\"phase\":\"{s}\",\"suppressed\":{d}}}\n",
        .{
            record_file.unixMilliseconds(),
            identifierOrUnnamed(error_name),
            identifierOrUnnamed(phase),
            observation_refusals.suppressed,
        },
    ) catch return;
    observation_refusals.recorded(now);
    writeLine(line);
}

/// Records an event-delivery pass that failed and left its events for the next pass, at most once
/// per window. `error` is the Host's own error name.
pub fn eventsDeferred(error_name: []const u8) void {
    if (path == null) return;
    const now = nowNs();
    if (!deferred_event_passes.allows(now)) return;
    var buffer: [max_line_bytes]u8 = undefined;
    const line = std.fmt.bufPrint(
        &buffer,
        "{{\"time_unix_ms\":{d},\"event\":\"events-deferred\",\"error\":\"{s}\",\"suppressed\":{d}}}\n",
        .{
            record_file.unixMilliseconds(),
            identifierOrUnnamed(error_name),
            deferred_event_passes.suppressed,
        },
    ) catch return;
    deferred_event_passes.recorded(now);
    writeLine(line);
}

fn identifierOrUnnamed(name: []const u8) []const u8 {
    return if (isIdentifier(name)) name else "Unnamed";
}

fn writeOptionalCode(writer: *std.Io.Writer, code: ?u32) std.Io.Writer.Error!void {
    if (code) |value| return writer.print("{d}", .{value});
    return writer.writeAll("null");
}

/// Error names come from the Host's own code; anything else is dropped rather than written.
fn isIdentifier(name: []const u8) bool {
    if (name.len == 0 or name.len > 64) return false;
    for (name) |byte| switch (byte) {
        'A'...'Z', 'a'...'z', '0'...'9', '_' => {},
        else => return false,
    };
    return true;
}

/// Records a dispatch turn over `slow_turn_threshold_ns`, once per kind per window.
pub fn slowTurn(turn: Turn, elapsed_ns: u64) void {
    if (elapsed_ns <= slow_turn_threshold_ns or path == null) return;
    const now = nowNs() orelse return;
    if (!claimSlowTurn(turn.key(), now)) return;
    var buffer: [max_line_bytes]u8 = undefined;
    const elapsed_ms = elapsed_ns / std.time.ns_per_ms;
    const line = switch (turn) {
        .request => |kind| std.fmt.bufPrint(
            &buffer,
            "{{\"time_unix_ms\":{d},\"event\":\"slow-turn\",\"turn\":\"request\",\"request_kind\":{d},\"elapsed_ms\":{d}}}\n",
            .{ record_file.unixMilliseconds(), kind, elapsed_ms },
        ),
        .events => std.fmt.bufPrint(
            &buffer,
            "{{\"time_unix_ms\":{d},\"event\":\"slow-turn\",\"turn\":\"events\",\"request_kind\":null,\"elapsed_ms\":{d}}}\n",
            .{ record_file.unixMilliseconds(), elapsed_ms },
        ),
    } catch return;
    writeLine(line);
}

/// Counts one terminal-generated reply that was dropped; any thread, no I/O.
pub fn noteDroppedTerminalReply() void {
    _ = dropped_replies.fetchAdd(1, .monotonic);
}

/// Records replies dropped since the last line, at most once per window.
pub fn flush() void {
    if (path == null) return;
    const total = dropped_replies.load(.monotonic);
    if (total == recorded_dropped_replies) return;
    const now = nowNs() orelse return;
    if (dropped_recorded_at_ns) |recorded_at| {
        if (now -| recorded_at < rate_window_ns) return;
    }
    var buffer: [max_line_bytes]u8 = undefined;
    const line = std.fmt.bufPrint(
        &buffer,
        "{{\"time_unix_ms\":{d},\"event\":\"terminal-replies-dropped\",\"count\":{d},\"total\":{d}}}\n",
        .{ record_file.unixMilliseconds(), total - recorded_dropped_replies, total },
    ) catch return;
    recorded_dropped_replies = total;
    dropped_recorded_at_ns = now;
    writeLine(line);
}

fn writeLine(line: []const u8) void {
    const target = if (path) |*value| value else return;
    _ = target.write(line, .{ .append_bounded = max_file_bytes });
}

fn nowNs() ?u64 {
    const timer = if (clock) |*value| value else return null;
    return timer.read();
}

/// Whether a slow turn of `key` may be recorded now; taking the slot of the oldest kind when every
/// slot is in use keeps the table bounded.
fn claimSlowTurn(key: u32, now: u64) bool {
    var oldest: usize = 0;
    var oldest_ns: u64 = std.math.maxInt(u64);
    for (&slow_turns, 0..) |*slot, index| {
        const entry = slot.* orelse {
            slot.* = .{ .key = key, .recorded_at_ns = now };
            return true;
        };
        if (entry.key == key) {
            if (now -| entry.recorded_at_ns < rate_window_ns) return false;
            slot.* = .{ .key = key, .recorded_at_ns = now };
            return true;
        }
        if (entry.recorded_at_ns < oldest_ns) {
            oldest = index;
            oldest_ns = entry.recorded_at_ns;
        }
    }
    slow_turns[oldest] = .{ .key = key, .recorded_at_ns = now };
    return true;
}
