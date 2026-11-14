//! Pure retained terminal lifecycle and event encoding within the Control boundary.
const std = @import("std");
const identifierText = @import("encode_common.zig").identifierText;

pub fn terminalExited(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    notice: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const exit_json = try exitResultJson(allocator, notice.result);
    defer allocator.free(exit_json);
    return std.fmt.allocPrint(
        allocator,
        "{{\"exit\":{s},\"finalRevision\":\"{d}\",\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}}}}",
        .{
            exit_json,
            notice.final_revision,
            identifierText(notice.creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(notice.terminal_id, &terminal_buffer),
        },
    );
}

pub fn terminalFailed(
    allocator: std.mem.Allocator,
    host_epoch: anytype,
    notice: anytype,
) ![]u8 {
    var creation_buffer: [32]u8 = undefined;
    var epoch_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    const failure_json = try runtimeFailureJson(allocator, notice.failure);
    defer allocator.free(failure_json);
    const process_json = try processObservationJson(allocator, notice.process);
    defer allocator.free(process_json);
    return std.fmt.allocPrint(
        allocator,
        "{{\"failure\":{s},\"finalRevision\":\"{d}\",\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"process\":{s}}}",
        .{
            failure_json,
            notice.final_revision,
            identifierText(notice.creation_id, &creation_buffer),
            identifierText(host_epoch, &epoch_buffer),
            identifierText(notice.terminal_id, &terminal_buffer),
            process_json,
        },
    );
}

pub fn optionalExitResultJson(allocator: std.mem.Allocator, result: anytype) ![]u8 {
    const value = result orelse return allocator.dupe(u8, "null");
    return exitResultJson(allocator, value);
}

fn exitResultJson(allocator: std.mem.Allocator, result: anytype) ![]u8 {
    return switch (result) {
        .exit_code => |code| std.fmt.allocPrint(allocator, "{{\"code\":{d},\"kind\":\"exit-code\"}}", .{code}),
        .signal => |signal| {
            std.debug.assert(signal != 0);
            return std.fmt.allocPrint(allocator, "{{\"kind\":\"signal\",\"signal\":{d}}}", .{signal});
        },
        .unknown => |unknown| std.fmt.allocPrint(allocator, "{{\"kind\":\"unknown\",\"value\":{d}}}", .{unknown}),
    };
}

pub fn terminalPhaseText(phase: anytype) []const u8 {
    return switch (phase) {
        .awaiting_start => "awaiting-start",
        .running => "running",
        .finishing => "finishing",
        .containing_failure => "containing-failure",
        .exited => "exited",
        .failed => "failed",
    };
}

fn runtimeFailureKindText(kind: anytype) []const u8 {
    return switch (kind) {
        .process_control => "process-control",
        .process_observation => "process-observation",
        .pty_read => "pty-read",
        .pty_write => "pty-write",
        .resize => "resize",
        .terminal_reply => "terminal-reply",
        .worker_integrity => "worker-integrity",
    };
}

fn runtimeFailureStageText(stage: anytype) []const u8 {
    return switch (stage) {
        .admit_terminal_reply => "admit-terminal-reply",
        .apply_resize => "apply-resize",
        .contain_process => "contain-process",
        .dispatch_termination => "dispatch-termination",
        .observe_process => "observe-process",
        .read_output => "read-output",
        .release_output => "release-output",
        .release_process_identity => "release-process-identity",
        .synchronize_worker => "synchronize-worker",
        .write_input => "write-input",
    };
}

pub fn runtimeFailureJson(allocator: std.mem.Allocator, failure: anytype) ![]u8 {
    const Failure = @TypeOf(failure);
    const kind = if (@hasField(Failure, "kind")) failure.kind else failure.kind();
    const stage = if (@hasField(Failure, "stage")) failure.stage else failure.stage();
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try writer.writer.print(
        "{{\"kind\":\"{s}\",\"stage\":\"{s}\"}}",
        .{ runtimeFailureKindText(kind), runtimeFailureStageText(stage) },
    );
    return writer.toOwnedSlice();
}

pub fn processObservationJson(allocator: std.mem.Allocator, observation: anytype) ![]u8 {
    return switch (observation) {
        .observed => |result| {
            const result_json = try exitResultJson(allocator, result);
            defer allocator.free(result_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"kind\":\"observed\",\"result\":{s}}}",
                .{result_json},
            );
        },
        .unavailable => |failure| {
            const failure_json = try runtimeFailureJson(allocator, failure);
            defer allocator.free(failure_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"failure\":{s},\"kind\":\"unavailable\"}}",
                .{failure_json},
            );
        },
    };
}

pub fn failureContainmentJson(allocator: std.mem.Allocator, containment: anytype) ![]u8 {
    return switch (containment) {
        .stopping => |state| {
            const process_json = if (state.process) |process|
                try processObservationJson(allocator, process)
            else
                try allocator.dupe(u8, "null");
            defer allocator.free(process_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"kind\":\"stopping\",\"process\":{s}}}",
                .{process_json},
            );
        },
        .blocked => |state| {
            const cleanup_json = try runtimeFailureJson(allocator, state.cleanup_failure);
            defer allocator.free(cleanup_json);
            const process_json = if (state.process) |process|
                try processObservationJson(allocator, process)
            else
                try allocator.dupe(u8, "null");
            defer allocator.free(process_json);
            return std.fmt.allocPrint(
                allocator,
                "{{\"cleanupFailure\":{s},\"kind\":\"blocked\",\"process\":{s}}}",
                .{ cleanup_json, process_json },
            );
        },
    };
}
