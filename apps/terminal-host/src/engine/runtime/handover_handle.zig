const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const creation = @import("creation.zig");
const handover_export = @import("handover_export.zig");
const handover_restore = @import("handover_restore.zig");

// Handover adapters preserve the opaque public runtime boundary.
pub fn Type(comptime Runtime: type) type {
    return struct {
        pub const Imported = struct {
            runtime: *Runtime,
            facts: handover_restore.Facts,
        };

        /// Encodes the terminal's record while its reader is parked at ground. A refusal or failure
        /// changes nothing.
        pub fn exportHandover(
            self: *Runtime,
            allocator: std.mem.Allocator,
            inputs: handover_export.Inputs,
        ) handover_export.Error![]u8 {
            return handover_export.exportRecord(stateFrom(self), allocator, inputs);
        }

        /// Restores a terminal from `record`, running `launch` at the record's geometry. A refused
        /// record creates nothing.
        pub fn importHandover(
            allocator: std.mem.Allocator,
            record: []const u8,
            launch: contracts.ProcessLaunch,
        ) !Imported {
            var spawner: creation.SystemThreadSpawner = .{};
            return importHandoverUsingSpawner(allocator, record, launch, &spawner);
        }

        pub fn importHandoverUsingSpawner(
            allocator: std.mem.Allocator,
            record: []const u8,
            launch: contracts.ProcessLaunch,
            spawner: anytype,
        ) !Imported {
            const restored = try handover_restore.restore(allocator, record, launch, spawner);
            return .{ .runtime = @ptrCast(restored.runtime), .facts = restored.facts };
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }
    };
}
