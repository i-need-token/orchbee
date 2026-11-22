//! This binding's mirror of the `worker-host` and `keeper` entries in
//! `schemas/connection/v1/connections.json`. The generations and capability names are checked
//! against the registry by the conformance tests; the development fingerprints are computed by
//! `build.zig` from the registry's inputs.

const std = @import("std");
const builtin = @import("builtin");
const options = @import("handshake-options");

const admission = @import("admission.zig");
const message = @import("message.zig");

pub const worker_host = struct {
    pub const name = "worker-host";
    pub const generations: message.GenerationRange = .{ .lowest = 1, .highest = 1 };
    /// Every capability the registry declares for `worker-host`, in its order.
    pub const declared_capabilities: []const []const u8 = &.{ "handover", "keeper", "prompt-gated-input", "screen-gated-input", "screen-gated-send" };
    /// The declared capabilities this Host implements and offers. `keeper`, on Windows only: this
    /// Host attaches Keepers, holds its running terminals there, and releases them first.
    /// `prompt-gated-input`: this Host types a `PromptLineInput` only at a confirmed idle prompt.
    /// `screen-gated-input`: this Host types a `ScreenGatedKeyInput` only on the frame it names.
    /// `screen-gated-send`: and an attachment that goes away holding no key and no mouse button
    /// leaves no person's input, which the Hosts of 0.1.0-beta.22 recorded; a Worker sends a gated
    /// key only under this name.
    pub const capabilities: []const []const u8 = if (builtin.os.tag == .windows)
        &.{ "keeper", "prompt-gated-input", "screen-gated-input", "screen-gated-send" }
    else
        &.{ "prompt-gated-input", "screen-gated-input", "screen-gated-send" };
    /// `capabilities` with `handover`, on Windows only: this Host moves running terminals to and
    /// from another Host through the Keeper.
    pub const capabilities_with_handover: []const []const u8 = if (builtin.os.tag == .windows)
        &.{ "handover", "keeper", "prompt-gated-input", "screen-gated-input", "screen-gated-send" }
    else
        capabilities;

    /// What this Host offers; `handover` only where its conpty can take a moved terminal.
    pub fn offered(handover: bool) []const []const u8 {
        return if (handover) capabilities_with_handover else capabilities;
    }
    pub const development_fingerprint: []const u8 = options.worker_host_fingerprint;
};

/// The Keeper's `OBKP` connection, which the Keeper accepts from its Hosts and its Worker.
pub const keeper = struct {
    pub const name = "keeper";
    pub const generations: message.GenerationRange = .{ .lowest = 1, .highest = 1 };
    /// Before the Worker may ask the Keeper to `assign` an orphaned holding to a Host that claims
    /// it, or to `discard` one.
    pub const recovery = "recovery";
    pub const declared_capabilities: []const []const u8 = &.{recovery};
    pub const capabilities: []const []const u8 = &.{recovery};
    pub const development_fingerprint: []const u8 = options.keeper_fingerprint;
};

/// This build's acceptor profile for `worker-host`: the registry's generations, the implemented
/// capability names, and the fingerprint exactly when `environment` is not `orchbee`.
pub fn workerHostProfile(
    product_version: []const u8,
    build: message.Build,
    environment: []const u8,
    handover: bool,
) admission.Profile {
    var result = profile(worker_host, product_version, build, environment);
    result.capabilities = worker_host.offered(handover);
    return result;
}

/// This build's acceptor profile for `keeper`, on the same terms.
pub fn keeperProfile(
    product_version: []const u8,
    build: message.Build,
    environment: []const u8,
) admission.Profile {
    return profile(keeper, product_version, build, environment);
}

fn profile(
    comptime entry: type,
    product_version: []const u8,
    build: message.Build,
    environment: []const u8,
) admission.Profile {
    const release = std.mem.eql(u8, environment, message.release_environment);
    return .{
        .connection = entry.name,
        .generations = entry.generations,
        .capabilities = entry.capabilities,
        .product_version = product_version,
        .build = build,
        .environment = environment,
        .fingerprint = if (release) null else entry.development_fingerprint,
    };
}
