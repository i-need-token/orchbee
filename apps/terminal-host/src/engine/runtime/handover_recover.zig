//! Builds a recovered terminal: a fresh engine, at the size and with the profile the Worker names,
//! for a running program the Keeper kept after its Host ended. Nothing of the lost Host's screen
//! survives, so the engine starts empty. `handover_restore.start` then adopts the held console and
//! starts every worker behind the startup gate, as for a restored record, and only the claim opens
//! it.
const std = @import("std");
const ghostty_vt = @import("ghostty-vt");
const model = @import("terminal-host-model");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const handover = @import("../handover/root.zig");
const png_decoder = @import("../png_decoder.zig");
const terminal_profile = @import("../profile.zig");
const terminal_stream = @import("../terminal_stream.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const creation = @import("creation.zig");
const handover_restore = @import("handover_restore.zig");

/// What the Worker names for a recovery: the terminal the program ran as, the holding and mark it
/// is claimed under, and the size, profile, and scrollback of its fresh engine.
pub const Recovery = struct {
    identity: handover.host_sections.TerminalIdentity,
    holding: [16]u8,
    mark: [16]u8,
    geometry: model.geometry.Geometry,
    profile: model.profile.Profile,
    scrollback_bytes: usize,
};

/// The recovered terminal's runtime, running no worker yet, and the facts its commit and claim
/// read, with `console` and `launch_kind` as the Keeper reported them; the runtime borrows neither
/// `recovery` nor `console`.
pub fn prepare(
    allocator: std.mem.Allocator,
    recovery: Recovery,
    console: handover.ConsoleFacts,
    launch_kind: platform.process.LaunchKind,
) !handover_restore.Prepared {
    png_decoder.install();
    const geometry = recovery.geometry;
    if (!model.geometry.isValid(geometry)) return error.InvalidGeometry;
    const colors = try terminal_profile.terminalColors(recovery.profile);
    const held_attachments = try allocator.alloc(contracts.AttachmentId, 0);
    errdefer allocator.free(held_attachments);
    const self = try allocator.create(state.State);
    errdefer allocator.destroy(self);
    const resources = resources: {
        var profile = try contracts.OwnedProfile.init(allocator, recovery.profile);
        errdefer profile.deinit(allocator);
        const kitty_temp_directory = try platform.temp_directory.resolveOwned(allocator);
        errdefer allocator.free(kitty_temp_directory);
        var terminal: ghostty_vt.Terminal = try .init(platform.thread_sync.io(), allocator, .{
            .cols = geometry.columns,
            .colors = colors,
            .default_cursor_blink = profile.default_cursor_blinking,
            .default_cursor_style = profile.cursorStyle(),
            .default_modes = .{
                .cursor_blinking = profile.default_cursor_blinking,
                .grapheme_cluster = true,
            },
            .kitty_image_loading_limits = creation.kittyLoadingLimits(kitty_temp_directory),
            .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
            .max_scrollback_bytes = recovery.scrollback_bytes,
            .rows = geometry.rows,
        });
        errdefer terminal.deinit(allocator);
        terminal.width_px = @as(u32, geometry.columns) * geometry.cell_pixel_width;
        terminal.height_px = @as(u32, geometry.rows) * geometry.cell_pixel_height;
        profile.applyCursorDefaults(&terminal);
        break :resources .{
            .kitty_temp_directory = kitty_temp_directory,
            .profile = profile,
            .terminal = terminal,
        };
    };
    self.* = .{
        .allocator = allocator,
        .cell_pixel_height = geometry.cell_pixel_height,
        .cell_pixel_width = geometry.cell_pixel_width,
        .command_timer = platform.monotonic.Timer.start() catch null,
        .input_queue = .init(allocator),
        .input_ownership = .init(allocator),
        .kitty_temp_directory = resources.kitty_temp_directory,
        .viewport_geometry = geometry,
        .process = undefined,
        .profile = resources.profile,
        .projection = .init(allocator),
        .history = .init(allocator),
        .stream = undefined,
        .stream_allocator = .init(allocator, terminal_stream.osc_allocation_limit_bytes),
        .scrollback_bytes = recovery.scrollback_bytes,
        .terminal = resources.terminal,
    };
    creation.wireStream(self);
    const no_digest = [_]u8{0} ** 32;
    return .{
        .runtime = self,
        .facts = .{
            .product_version = "",
            .terminal = .{
                .inputs = .{
                    .holding_id = recovery.holding,
                    .mark_id = recovery.mark,
                    .identity = recovery.identity,
                    .lifecycle = .{ .running = .none },
                    .start_geometry = geometry,
                    // No launch travels with a recovery, so no creation replay can match it.
                    .launch_fingerprint = no_digest,
                    .environment_overlay_digest = no_digest,
                },
                .revision = self.revision.load(.monotonic),
                .generation = self.generation,
                .geometry = geometry,
                .viewport_geometry = geometry,
                .launch_kind = launch_kind,
                .interactive_shell = interactive(launch_kind),
            },
            .configuration = .{
                .configuration = .{ .scrollback_bytes = recovery.scrollback_bytes },
                .revision = 0,
                .theme = .{ .terminal = model.profile.theme(recovery.profile) },
            },
            .console = console,
            .held_attachments = held_attachments,
        },
    };
}

/// A managed shell or cmd is an interactive shell; any other program is not, so a recovered
/// terminal admits no prompt-gated input it could not confirm.
pub fn interactive(kind: platform.process.LaunchKind) bool {
    return kind != .other;
}
