const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

pub const idle_interval_ns: i128 = 250 * std.time.ns_per_ms;

/// Offers one bounded compression opportunity without ever waiting for the
/// canonical terminal lock. Ghostty owns compression eligibility and the
/// incremental cursor; the Host owns only idle scheduling around that API.
pub fn maintain(
    runtime: *state.State,
    now_ns: i128,
) contracts.ScrollbackCompressionOutcome {
    if (!runtime.terminal_mutex.tryLock()) return .lock_busy;
    defer runtime.terminal_mutex.unlock();

    const activity = runtime.terminal.compressionActivity();
    const schedule = &runtime.scrollback_compression;
    if (schedule.activity == null or schedule.activity.? != activity) {
        schedule.activity = activity;
        schedule.eligible_at_ns = std.math.add(
            i128,
            now_ns,
            idle_interval_ns,
        ) catch std.math.maxInt(i128);
        schedule.phase = .waiting;
        return .idle_wait;
    }

    switch (schedule.phase) {
        .dormant, .unsupported => return .dormant,
        .waiting, .pending => {},
    }
    if (now_ns < schedule.eligible_at_ns) return .idle_wait;

    return switch (runtime.terminal.compress(.incremental)) {
        .pending => outcome: {
            schedule.phase = .pending;
            break :outcome .pending;
        },
        .complete => outcome: {
            schedule.phase = .dormant;
            break :outcome .complete;
        },
        .unsupported => outcome: {
            schedule.phase = .unsupported;
            break :outcome .unsupported;
        },
    };
}
