const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

pub const idle_interval_ns: i128 = 250 * std.time.ns_per_ms;

/// Records interactive terminal work while the caller owns terminal_mutex.
/// Wrapping is safe because the scheduler only compares serial equality.
pub fn noteInteractionLocked(runtime: *state.State) void {
    runtime.scrollback_compression.interaction_serial +%= 1;
}

/// Offers one bounded compression opportunity without ever waiting for the
/// canonical terminal lock. Ghostty owns compression eligibility and the
/// incremental cursor; the Host owns only idle scheduling around that API.
pub fn maintain(
    runtime: *state.State,
    now_ns: i128,
) contracts.ScrollbackCompressionOutcome {
    if (!runtime.terminal_mutex.tryLock()) return .lock_busy;
    defer runtime.terminal_mutex.unlock();

    const compression_activity = runtime.terminal.compressionActivity();
    const schedule = &runtime.scrollback_compression;
    if (schedule.compression_activity == null or
        schedule.compression_activity.? != compression_activity)
    {
        schedule.compression_activity = compression_activity;
        schedule.observed_interaction_serial = schedule.interaction_serial;
        restartIdle(schedule, now_ns);
        return .idle_wait;
    }

    if (schedule.observed_interaction_serial != schedule.interaction_serial) {
        schedule.observed_interaction_serial = schedule.interaction_serial;
        switch (schedule.phase) {
            .waiting, .pending => {
                restartIdle(schedule, now_ns);
                return .idle_wait;
            },
            .dormant, .unsupported => return .dormant,
        }
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

fn restartIdle(schedule: *contracts.ScrollbackCompressionSchedule, now_ns: i128) void {
    schedule.eligible_at_ns = std.math.add(
        i128,
        now_ns,
        idle_interval_ns,
    ) catch std.math.maxInt(i128);
    schedule.phase = .waiting;
}
