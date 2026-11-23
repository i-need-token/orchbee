const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const creation = @import("creation.zig");
const reader_pause = @import("reader_pause.zig");
const handover_export = @import("handover_export.zig");
const handover_move = @import("handover_move.zig");
const handover_restore = @import("handover_restore.zig");
const handover_recover = @import("handover_recover.zig");
const resize_ops = @import("resize.zig");
const handover = @import("../handover/root.zig");
const platform = @import("terminal-host-platform");

// Handover adapters preserve the opaque public runtime boundary.
pub fn Type(comptime Runtime: type) type {
    return struct {
        pub const Imported = struct {
            runtime: *Runtime,
            facts: handover_restore.Facts,
        };

        /// The reader's undoable stop, for a quiesce alone: one caller at a time pauses, awaits
        /// the park, and resumes or withdraws.
        pub fn pauseReader(self: *Runtime) !void {
            return reader_pause.pause(stateFrom(self));
        }

        pub fn awaitReaderParked(self: *Runtime, timeout_ns: u64) contracts.ReaderParkWait {
            return reader_pause.awaitParked(stateFrom(self), timeout_ns);
        }

        /// Resumes a parked reader, or withdraws a pause it has not reached.
        pub fn resumeReader(self: *Runtime) !void {
            return reader_pause.resumeReader(stateFrom(self));
        }

        /// Holds the terminal for a quiesce: from now on nothing ends or contains it, and an
        /// observed program exit waits for the move's result.
        pub fn beginMove(self: *Runtime) handover_move.BeginError!void {
            return handover_move.begin(stateFrom(self));
        }

        /// Quiesce steps 3 to 5, off the dispatch thread.
        pub fn parkForMove(self: *Runtime) handover_move.Park {
            return handover_move.park(stateFrom(self));
        }

        /// Ends a held move without leaving: the reader reads on, and an observed exit and the
        /// deferred input cleanups run.
        pub fn endMove(self: *Runtime) void {
            handover_move.end(stateFrom(self));
        }

        /// Lets a held terminal go without touching its program, and frees the runtime.
        pub fn dropMoved(self: *Runtime) void {
            handover_move.drop(stateFrom(self));
        }

        /// Opens a restored terminal's startup gate after its claim.
        pub fn claimRestored(self: *Runtime) void {
            handover_move.claim(stateFrom(self));
        }

        /// Encodes the terminal's record while its reader is parked at ground. A refusal or failure
        /// changes nothing.
        pub fn exportHandover(
            self: *Runtime,
            allocator: std.mem.Allocator,
            inputs: handover_export.Inputs,
        ) handover_export.Error![]u8 {
            return handover_export.exportRecord(stateFrom(self), allocator, inputs);
        }

        /// Encodes a settled terminal's record, which needs no park: no byte follows.
        pub fn exportSettledHandover(
            self: *Runtime,
            allocator: std.mem.Allocator,
            inputs: handover_export.Inputs,
        ) handover_export.Error![]u8 {
            return handover_export.exportSettledRecord(stateFrom(self), allocator, inputs);
        }

        /// Decodes and checks `record` and builds its engine, which runs no worker yet.
        pub fn prepareImport(allocator: std.mem.Allocator, record: []const u8) !handover_restore.Prepared {
            return handover_restore.prepare(allocator, record);
        }

        /// Builds a recovered terminal's fresh engine, which runs no worker yet; `startImport`
        /// then adopts the held console.
        pub fn prepareRecovery(
            allocator: std.mem.Allocator,
            recovery: handover_recover.Recovery,
            console: ?handover.ConsoleFacts,
            launch_kind: platform.process.LaunchKind,
        ) !handover_restore.Prepared {
            return handover_recover.prepare(allocator, recovery, console, launch_kind);
        }

        /// Resizes a claimed terminal's console one row and back, so the console repaints its
        /// screen and a full-screen program redraws.
        pub fn nudgeConsole(self: *Runtime) !void {
            const runtime = stateFrom(self);
            runtime.terminal_mutex.lock();
            const columns: u16 = @intCast(runtime.terminal.cols);
            const rows: u16 = @intCast(runtime.terminal.rows);
            const width = runtime.cell_pixel_width;
            const height = runtime.cell_pixel_height;
            runtime.terminal_mutex.unlock();
            const nudged = if (rows > 1) rows - 1 else rows + 1;
            _ = try resize_ops.resizeGeometry(runtime, columns, nudged, width, height);
            _ = try resize_ops.resizeGeometry(runtime, columns, rows, width, height);
        }

        /// Starts a prepared import: a running one behind its startup gate, a settled one with no
        /// worker. On failure nothing remains of it.
        pub fn startImport(prepared: *handover_restore.Prepared, program: handover_restore.Program) !*Runtime {
            var spawner: creation.SystemThreadSpawner = .{};
            try handover_restore.start(prepared, program, &spawner);
            return @ptrCast(prepared.runtime);
        }

        /// For tests: restores `record` with a stand-in process at the record's geometry.
        pub fn importHandoverUsingSpawner(
            allocator: std.mem.Allocator,
            record: []const u8,
            launch: contracts.ProcessLaunch,
            spawner: anytype,
        ) !Imported {
            const restored = try handover_restore.restoreStandIn(allocator, record, launch, spawner);
            return .{ .runtime = @ptrCast(restored.runtime), .facts = restored.facts };
        }

        fn stateFrom(runtime: *Runtime) *state.State {
            return @ptrCast(@alignCast(runtime));
        }
    };
}
