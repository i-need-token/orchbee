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
        /// No person made this input, so the screen gate does not count it: pointer
        /// motion without a button, or a cleanup that releases no held key or button.
        unattended: bool = false,
        /// Prompt-gated input publishes only while this gate serial still holds.
        prompt_gate_serial: ?u64 = null,
        /// A screen-gated key publishes only while this frame is still the screen.
        screen_gate: ?input_commit_ops.ScreenGate = null,
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
                .unattended = self.unattended,
                .prompt_gate_serial = self.prompt_gate_serial,
                .screen_gate = self.screen_gate,
            };
            var ownership = self.ownership;
            const reservation = self.reservation;
            self.bytes = null;
            self.focus_report = false;
            self.focus_reporting_enabled = null;
            self.mouse_input_epoch = null;
            self.ownership = null;
            self.unattended = false;
            self.prompt_gate_serial = null;
            self.screen_gate = null;
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
                input_commit_ops.validatePreparedInputCommit(runtime, &ownership, expectations) catch |err| {
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
