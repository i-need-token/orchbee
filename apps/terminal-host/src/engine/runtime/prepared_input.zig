const std = @import("std");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const input_commit_ops = @import("input_commit.zig");
const PreparedInputState = enum {
    pending,
    consumed,
};

/// Owns an encoded client-input copy and its ordered queue reservation until
/// HostService has prepared the matching success reply. Deinitializing a
/// pending value removes the reservation, so a pre-commit failure consumes no
/// queue position and admits no terminal bytes.
pub fn Type(comptime Runtime: type) type {
    return struct {
        bytes: ?[]u8,
        /// A focus report is not typed input for the prompt gate.
        focus_report: bool = false,
        focus_reporting_enabled: ?bool = null,
        mouse_input_epoch: ?u64 = null,
        ownership: ?contracts.PreparedInputOwnership = null,
        /// Prompt-gated input publishes only while this gate serial still holds.
        prompt_gate_serial: ?u64 = null,
        reservation: ?contracts.InputReservation,
        runtime: *Runtime,
        state: PreparedInputState = .pending,

        const Self = @This();

        fn runtimeState(self: *const Self) *state.State {
            return @ptrCast(@alignCast(self.runtime));
        }

        pub fn byteCount(self: *const Self) usize {
            std.debug.assert(self.state == .pending);
            return if (self.bytes) |bytes| bytes.len else 0;
        }

        pub fn commit(self: *Self) !usize {
            std.debug.assert(self.state == .pending);
            const byte_count = self.byteCount();
            const bytes = self.bytes;
            const expectations: input_commit_ops.Expectations = .{
                .focus_report = self.focus_report,
                .focus_reporting_enabled = self.focus_reporting_enabled,
                .mouse_input_epoch = self.mouse_input_epoch,
                .prompt_gate_serial = self.prompt_gate_serial,
            };
            var ownership = self.ownership;
            const reservation = self.reservation;
            self.bytes = null;
            self.focus_report = false;
            self.focus_reporting_enabled = null;
            self.mouse_input_epoch = null;
            self.ownership = null;
            self.prompt_gate_serial = null;
            self.reservation = null;
            self.state = .consumed;
            const runtime = self.runtimeState();
            if (bytes) |owned| {
                input_commit_ops.publishPreparedInput(
                    runtime,
                    reservation.?,
                    owned,
                    &ownership,
                    expectations,
                ) catch |err| {
                    if (ownership) |*pending| input_commit_ops.cancelPreparedInputOwnership(runtime, pending);
                    return err;
                };
            } else {
                std.debug.assert(reservation == null);
                std.debug.assert(expectations.prompt_gate_serial == null);
                input_commit_ops.validatePreparedInputCommit(
                    runtime,
                    &ownership,
                    expectations.focus_reporting_enabled,
                    expectations.mouse_input_epoch,
                ) catch |err| {
                    if (ownership) |*pending| input_commit_ops.cancelPreparedInputOwnership(runtime, pending);
                    return err;
                };
            }
            std.debug.assert(ownership == null);
            return byte_count;
        }

        pub fn deinit(self: *Self) void {
            if (self.state == .pending) {
                const runtime = self.runtimeState();
                if (self.reservation) |reservation| {
                    input_commit_ops.cancelInput(runtime, reservation.identifier);
                }
                if (self.bytes) |bytes| runtime.allocator.free(bytes);
                if (self.ownership) |*ownership| {
                    input_commit_ops.cancelPreparedInputOwnership(runtime, ownership);
                }
            }
            self.* = undefined;
        }
    };
}

pub const Internal = Type(state.State);
