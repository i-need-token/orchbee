const std = @import("std");

const terminal_configuration = @import("terminal-host-model").configuration;

pub const Error = error{
    InvalidConfiguration,
    RevisionOverflow,
};

pub const PreparedDefaultsUpdate = struct {
    baseline_revision: u64,
    changed: bool,
    configuration: terminal_configuration.Configuration,
    revision: u64,
};

/// Host-owned process defaults. Preparing computes every fallible precondition;
/// commit is an allocation-free state transition after the service has reserved
/// and encoded the response that makes the new revision observable.
pub const State = struct {
    defaults: terminal_configuration.Configuration,
    revision: u64 = 1,

    pub fn init(defaults: terminal_configuration.Configuration) State {
        std.debug.assert(terminal_configuration.valid(defaults));
        return .{ .defaults = defaults };
    }

    pub fn prepareDefaultsUpdate(
        self: *const State,
        configuration: terminal_configuration.Configuration,
    ) Error!PreparedDefaultsUpdate {
        if (!terminal_configuration.valid(configuration)) return error.InvalidConfiguration;
        const changed = configuration.scrollback_bytes != self.defaults.scrollback_bytes;
        if (changed and self.revision == std.math.maxInt(u64)) return error.RevisionOverflow;
        return .{
            .baseline_revision = self.revision,
            .changed = changed,
            .configuration = configuration,
            .revision = if (changed) self.revision + 1 else self.revision,
        };
    }

    pub fn commitDefaultsUpdate(
        self: *State,
        prepared: PreparedDefaultsUpdate,
    ) void {
        std.debug.assert(prepared.baseline_revision == self.revision);
        if (prepared.changed) {
            std.debug.assert(prepared.revision == self.revision + 1);
            std.debug.assert(
                prepared.configuration.scrollback_bytes != self.defaults.scrollback_bytes,
            );
        } else {
            std.debug.assert(prepared.revision == self.revision);
            std.debug.assert(
                prepared.configuration.scrollback_bytes == self.defaults.scrollback_bytes,
            );
        }
        self.defaults = prepared.configuration;
        self.revision = prepared.revision;
    }
};
