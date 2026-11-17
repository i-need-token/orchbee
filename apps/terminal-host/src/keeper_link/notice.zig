//! What a link thread reports to the dispatch thread, which answers requests and sends events from
//! it. A notice names its Keeper, so the dispatch thread pairs it with what it is waiting for.

const keeper_protocol = @import("terminal-host-keeper-protocol");

pub const Keeper = keeper_protocol.message.Process;

pub const Notice = union(enum) {
    /// The link connected and the Keeper answered `hello`: `AttachKeeper` succeeds.
    attached: Keeper,
    /// The Keeper check failed: `AttachKeeper` fails `keeper-unavailable`.
    unavailable: Keeper,
    /// A reply missed its deadline or the connection ended; reported once per attachment.
    unresponsive: Keeper,
    /// The Keeper process ended while attached.
    lost: Keeper,
    /// A terminal runs unheld in this Keeper.
    unheld: Unheld,
    /// A `DetachKeeper` released every holding and closed the link.
    detached: Keeper,

    pub fn keeper(self: Notice) Keeper {
        return switch (self) {
            .unheld => |unheld| unheld.keeper,
            inline else => |value| value,
        };
    }
};

pub const Unheld = struct {
    keeper: Keeper,
    terminal_id: [16]u8,
    /// A Keeper refusal's wire name, or `unsupported`, `unanswered`, `hold-failed`, or
    /// `system-console`. Static text.
    reason: []const u8,
};
