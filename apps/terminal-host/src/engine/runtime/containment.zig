const std = @import("std");
const target_builtin = @import("builtin");
const contracts = @import("contracts.zig");
const state = @import("state.zig");

pub fn processTreeAlreadyContained(self: *state.State) !bool {
    self.containment_mutex.lock();
    defer self.containment_mutex.unlock();
    if (self.containment_confirmed.load(.acquire)) return true;
    for (0..contracts.required_consecutive_containment_observations) |observation| {
        if (!try self.process.processTreeContained()) return false;
        if (observation + 1 < contracts.required_consecutive_containment_observations) {
            std.Thread.sleep(contracts.process_containment_observation_delay_ns);
        }
    }
    self.containment_confirmed.store(true, .release);
    return true;
}

pub fn containProcessTree(self: *state.State) !void {
    if (self.containment_confirmed.load(.acquire)) return;
    self.containment_mutex.lock();
    defer self.containment_mutex.unlock();
    if (self.containment_confirmed.load(.acquire)) return;
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.contain_process_tree) |contain| {
            try contain(self.testing_hooks.context.?);
            self.containment_confirmed.store(true, .release);
            return;
        }
    }
    var consecutive_contained: usize = 0;
    var last_failure: anyerror = error.ProcessContainmentIncomplete;
    for (0..contracts.max_process_containment_observations) |observation| {
        if (self.containment_confirmed.load(.acquire)) return;
        const contained = self.process.processTreeContained() catch |err| query_failed: {
            consecutive_contained = 0;
            last_failure = err;
            break :query_failed false;
        };
        if (contained) {
            consecutive_contained += 1;
            if (consecutive_contained == contracts.required_consecutive_containment_observations) {
                self.containment_confirmed.store(true, .release);
                return;
            }
        } else {
            consecutive_contained = 0;
            self.process.forceTerminate() catch |err| {
                last_failure = err;
                if (observation + 1 < contracts.max_process_containment_observations) {
                    std.Thread.sleep(contracts.process_containment_observation_delay_ns);
                }
                continue;
            };
            last_failure = error.ProcessContainmentIncomplete;
        }
        if (observation + 1 < contracts.max_process_containment_observations) {
            std.Thread.sleep(contracts.process_containment_observation_delay_ns);
        }
    }
    return last_failure;
}

pub fn completeFirstContainmentAttempt(
    self: *state.State,
    expected: contracts.FirstContainmentAttempt,
) void {
    self.first_containment_attempt.complete(expected);
}

pub fn notifyHostContainmentWaiting(context: *anyopaque) void {
    const self: *state.State = @ptrCast(@alignCast(context));
    if (comptime target_builtin.is_test) {
        if (self.testing_hooks.host_containment_waiting) |notify| {
            notify(self.testing_hooks.context.?);
        }
    }
}

pub fn awaitFirstContainmentAttempt(self: *state.State) bool {
    return self.first_containment_attempt.awaitHost(
        self,
        notifyHostContainmentWaiting,
    );
}
