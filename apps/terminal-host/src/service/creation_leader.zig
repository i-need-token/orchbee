//! Who starts a new terminal's leader. On macOS the newest attached Keeper that parents leaders
//! does, through its link, which the creation pins from its begin until its build returned so the
//! link outlives the call. Elsewhere, and while no such Keeper is attached, the Host forks every
//! leader itself.

const target_builtin = @import("builtin");
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const identifiers = @import("terminal-host-model").identifiers;
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");

const macos = target_builtin.os.tag == .macos;

pub const LeaderStart = struct {
    spawner: if (macos) ?keeper_link.Spawner else void = if (macos) null else {},

    /// The dispatch thread: pins the Keeper link that starts the leader of `identity`, if any.
    pub fn pin(
        keepers: ?*keeper_link.Attachments,
        identity: host_runtime_aggregate.TerminalIdentity,
        host_epoch: identifiers.HostEpoch,
    ) LeaderStart {
        if (comptime !macos) return .{};
        const attachments = keepers orelse return .{};
        return .{ .spawner = attachments.spawner(.{
            .creation_id = identity.creation_id.bytes,
            .host_epoch = host_epoch.bytes,
            .terminal_id = identity.terminal_id.bytes,
        }) };
    }

    /// The creation thread: points `launch` at the pinned Keeper. The hook refers to this value,
    /// which stays where it is until `unpin`.
    pub fn apply(self: *LeaderStart, launch: *platform.process.Launch) void {
        if (comptime !macos) return;
        if (self.spawner) |*spawner| launch.keeper = spawner.hook();
    }

    pub fn unpin(self: *LeaderStart) void {
        if (comptime !macos) return;
        if (self.spawner) |*spawner| spawner.unpin();
        self.spawner = null;
    }
};
