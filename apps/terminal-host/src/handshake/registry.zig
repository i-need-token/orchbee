//! This binding's mirror of the `worker-host` entry in `schemas/connection/v1/connections.json`.
//! The generations and capability names are checked against the registry by the conformance
//! tests; the development fingerprint is computed by `build.zig` from the registry's inputs.

const std = @import("std");
const options = @import("handshake-options");

const admission = @import("admission.zig");
const message = @import("message.zig");

pub const worker_host = struct {
    pub const name = "worker-host";
    pub const generations: message.GenerationRange = .{ .lowest = 1, .highest = 1 };
    pub const capabilities: []const []const u8 = &.{};
    pub const development_fingerprint: []const u8 = options.worker_host_fingerprint;
};

/// This build's acceptor profile for `worker-host`: the registry's generations and capability
/// names, and the fingerprint exactly when `environment` is not `orchbee`.
pub fn workerHostProfile(
    product_version: []const u8,
    build: message.Build,
    environment: []const u8,
) admission.Profile {
    const release = std.mem.eql(u8, environment, message.release_environment);
    return .{
        .connection = worker_host.name,
        .generations = worker_host.generations,
        .capabilities = worker_host.capabilities,
        .product_version = product_version,
        .build = build,
        .environment = environment,
        .fingerprint = if (release) null else worker_host.development_fingerprint,
    };
}
