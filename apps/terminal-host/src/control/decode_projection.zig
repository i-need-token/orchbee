const std = @import("std");

const control_wire = @import("wire.zig");
const decode_common = @import("decode_common.zig");
const model = @import("terminal-host-model");
const attachment_model = model.attachment;
const identifiers = model.identifiers;
pub const FrameApplied = struct {
    attachment_id: identifiers.AttachmentId,
    generation: u64,
    revision: u64,
    sequence: u64,
};

pub const SnapshotReason = enum {
    activity_stale,
    base_mismatch,
    decoder_rejected,
    dimensions_mismatch,
    generation_mismatch,
    identity_mismatch,
    sequence_gap,
    store_reset,
};

pub const NeedSnapshot = struct {
    attachment_id: identifiers.AttachmentId,
    reason: SnapshotReason,
    sequence: u64,
};

pub const ProjectionDemand = struct {
    attachment_id: identifiers.AttachmentId,
    demand: attachment_model.ProjectionDemand,
};
pub fn decodeFrameApplied(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.FrameApplied, FrameApplied) {
    const parsed = try control_wire.decode(control_wire.FrameApplied, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .generation = try decode_common.parsePositiveCanonicalU64(parsed.value.generation),
            .revision = try decode_common.parseCanonicalU64(parsed.value.revision),
            .sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.sequence),
        },
    };
}

pub fn decodeNeedSnapshot(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.NeedSnapshot, NeedSnapshot) {
    const parsed = try control_wire.decode(control_wire.NeedSnapshot, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .reason = try parseSnapshotReason(parsed.value.reason),
            .sequence = try decode_common.parsePositiveCanonicalU64(parsed.value.sequence),
        },
    };
}

pub fn decodeProjectionDemand(
    allocator: std.mem.Allocator,
    payload: []const u8,
) decode_common.Error!decode_common.Decoded(control_wire.SetProjectionDemand, ProjectionDemand) {
    const parsed = try control_wire.decode(control_wire.SetProjectionDemand, allocator, payload);
    errdefer parsed.deinit();
    return .{
        .parsed = parsed,
        .value = .{
            .attachment_id = try identifiers.parseLowerHex(
                identifiers.AttachmentId,
                parsed.value.attachmentId,
            ),
            .demand = try parseProjectionDemand(parsed.value.demand),
        },
    };
}
fn parseSnapshotReason(value: []const u8) error{InvalidField}!SnapshotReason {
    inline for (.{
        .{ "activity-stale", SnapshotReason.activity_stale },
        .{ "base-mismatch", SnapshotReason.base_mismatch },
        .{ "decoder-rejected", SnapshotReason.decoder_rejected },
        .{ "dimensions-mismatch", SnapshotReason.dimensions_mismatch },
        .{ "generation-mismatch", SnapshotReason.generation_mismatch },
        .{ "identity-mismatch", SnapshotReason.identity_mismatch },
        .{ "sequence-gap", SnapshotReason.sequence_gap },
        .{ "store-reset", SnapshotReason.store_reset },
    }) |candidate| {
        if (std.mem.eql(u8, value, candidate[0])) return candidate[1];
    }
    return error.InvalidField;
}

fn parseProjectionDemand(value: []const u8) error{InvalidField}!attachment_model.ProjectionDemand {
    if (std.mem.eql(u8, value, "active")) return .active;
    if (std.mem.eql(u8, value, "paused")) return .paused;
    return error.InvalidField;
}
