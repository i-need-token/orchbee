const std = @import("std");

const control_encode = @import("terminal-host-control").encode;
const engine = @import("terminal-host-engine");
const host_outgoing = @import("outgoing.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const protocol = @import("terminal-protocol");

const max_schedule_scan_per_poll = 64;
const max_scheduled_metadata_per_poll = 8;

pub const Scheduler = struct {
    scan_index: usize = 0,

    pub fn schedule(
        self: *Scheduler,
        allocator: std.mem.Allocator,
        aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection_id: u64,
        outgoing: *host_outgoing.Outgoing,
    ) !host_outgoing.ScheduleOutcome {
        const core = aggregate.core();
        if (core.connectionClientId(connection_id) == null) return .complete;
        const count = core.terminalCount();
        if (count == 0) return .complete;
        const start = self.scan_index % count;
        var scanned: usize = 0;
        var scheduled: usize = 0;
        var outcome: host_outgoing.ScheduleOutcome = .complete;
        const Pending = struct {
            event: engine.runtime.MetadataEvent,
            runtime: *engine.runtime.TerminalRuntime,
        };
        var pending: [max_scheduled_metadata_per_poll]Pending = undefined;
        var pending_count: usize = 0;
        defer {
            for (pending[0..pending_count]) |*entry| entry.event.deinit();
        }

        scan: while (scanned < @min(count, max_schedule_scan_per_poll) and
            scheduled < max_scheduled_metadata_per_poll)
        {
            const index = (start + scanned) % count;
            scanned += 1;
            const terminal = core.terminalValues()[index];
            const runtime = aggregate.runtimeFor(terminal.terminal_id) orelse continue;
            var event = runtime.prepareMetadataEvent(allocator, connection_id) catch continue;
            if (event == null) continue;
            const payload = try control_encode.terminalMetadataChanged(
                allocator,
                core.epoch,
                terminal.creation_id,
                terminal.terminal_id,
                event.?,
            );
            defer allocator.free(payload);
            switch (try outgoing.append(
                protocol.MessageKind.terminal_metadata_changed,
                .control_json,
                payload,
                .{ .event = core.epoch },
            )) {
                .appended => {
                    std.debug.assert(pending_count < pending.len);
                    pending[pending_count] = .{ .event = event.?, .runtime = runtime };
                    pending_count += 1;
                    scheduled += 1;
                },
                .full => {
                    event.?.deinit();
                    scanned -= 1;
                    outcome = .batch_full;
                    break :scan;
                },
            }
        }

        self.scan_index = if (outcome == .batch_full)
            (start + scanned) % count
        else if (start + 1 == count)
            0
        else
            start + 1;
        for (pending[0..pending_count]) |entry| {
            entry.runtime.commitMetadataEvent(entry.event);
        }
        return outcome;
    }
};
