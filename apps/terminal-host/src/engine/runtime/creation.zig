const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const platform = @import("terminal-host-platform");
const monotonic = platform.monotonic;
const pty_process = platform.process;
const thread_sync = platform.thread_sync;
const temp_directory = platform.temp_directory;
const protocol = @import("terminal-protocol");
const terminal_profile = @import("../profile.zig");
const terminal_runtime_launch = @import("../runtime_launch.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const output_ops = @import("output.zig");
const feedback_ops = @import("feedback.zig");
const png_decoder = @import("../png_decoder.zig");
const output_seed = @import("../output_seed.zig");
const writer_ops = @import("writer.zig");
const wait_thread_ops = @import("wait_thread.zig");
const animation_ops = @import("animation.zig");

const SystemThreadSpawner = struct {
    fn spawn(
        _: *SystemThreadSpawner,
        comptime thread_function: anytype,
        runtime: anytype,
    ) !std.Thread {
        return std.Thread.spawn(.{}, thread_function, .{runtime});
    }
};

pub fn create(
    allocator: std.mem.Allocator,
    launch: pty_process.Launch,
) !*state.State {
    return createWithOptions(allocator, launch, .{});
}

pub fn createWithOptions(
    allocator: std.mem.Allocator,
    launch: pty_process.Launch,
    options: contracts.Options,
) !*state.State {
    var spawner: SystemThreadSpawner = .{};
    return createWithOptionsUsingSpawner(allocator, launch, options, &spawner);
}

pub fn createWithOptionsUsingSpawner(
    allocator: std.mem.Allocator,
    launch: pty_process.Launch,
    options: contracts.Options,
    spawner: anytype,
) !*state.State {
    png_decoder.install();
    const checked = try terminal_runtime_launch.prepare(allocator, launch, options);
    const resolved_launch = checked.process_launch;
    const resolved_options = checked.runtime_options;

    const colors = try terminal_profile.terminalColors(resolved_options.profile);
    const self = try allocator.create(state.State);
    errdefer allocator.destroy(self);

    const resources = resources: {
        var profile = try contracts.OwnedProfile.init(allocator, resolved_options.profile);
        errdefer profile.deinit(allocator);

        const kitty_temp_directory = try temp_directory.resolveOwned(allocator);
        errdefer allocator.free(kitty_temp_directory);

        const kitty_loading_limits: ghostty_vt.kitty.graphics.LoadingImage.Limits = .{
            .file = true,
            .temporary_file = .{ .enabled = .{ .directory = kitty_temp_directory } },
            .shared_memory = target_builtin.os.tag != .windows,
        };

        var terminal: ghostty_vt.Terminal = try .init(thread_sync.io(), allocator, .{
            .cols = resolved_launch.columns,
            .colors = colors,
            .default_cursor_blink = profile.default_cursor_blinking,
            .default_cursor_style = profile.cursorStyle(),
            .default_modes = .{ .cursor_blinking = profile.default_cursor_blinking },
            .kitty_image_loading_limits = kitty_loading_limits,
            .kitty_image_storage_limit = protocol.Limits.terminal_graphics_resource_bytes,
            .max_scrollback_bytes = resolved_options.scrollback_bytes,
            .rows = resolved_launch.rows,
        });
        errdefer terminal.deinit(allocator);
        terminal.width_px = @as(u32, resolved_launch.columns) * resolved_options.cell_pixel_width;
        terminal.height_px = @as(u32, resolved_launch.rows) * resolved_options.cell_pixel_height;
        profile.applyCursorDefaults(&terminal);
        break :resources .{
            .kitty_temp_directory = kitty_temp_directory,
            .profile = profile,
            .terminal = terminal,
        };
    };

    self.* = .{
        .allocator = allocator,
        .cell_pixel_height = resolved_options.cell_pixel_height,
        .command_timer = monotonic.Timer.start() catch null,
        .input_queue = .init(allocator),
        .input_ownership = .init(allocator),
        .kitty_temp_directory = resources.kitty_temp_directory,
        .cell_pixel_width = resolved_options.cell_pixel_width,
        .viewport_geometry = .{
            .cell_pixel_height = resolved_options.cell_pixel_height,
            .cell_pixel_width = resolved_options.cell_pixel_width,
            .columns = resolved_launch.columns,
            .rows = resolved_launch.rows,
        },
        .process = undefined,
        .profile = resources.profile,
        .projection = .init(allocator),
        .history = .init(allocator),
        .stream = undefined,
        .stream_allocator = .init(allocator),
        .scrollback_bytes = resolved_options.scrollback_bytes,
        .terminal = resources.terminal,
    };
    if (comptime target_builtin.is_test and
        @hasDecl(@TypeOf(spawner.*), "terminalRuntimeTestingHooks"))
    {
        self.testing_hooks = spawner.terminalRuntimeTestingHooks();
    }
    errdefer cleanupFailedThreadStartup(self);
    self.stream = .init(.{
        .allocator = self.stream_allocator.allocator(),
        .handler = .init(&self.terminal),
    });
    const effects = &self.stream.handler.terminal_handler.effects;
    effects.bell = output_ops.bellEffect;
    effects.write_pty = output_ops.writePtyEffect;
    effects.color_scheme = output_ops.colorSchemeEffect;
    effects.device_attributes = output_ops.deviceAttributesEffect;
    effects.enquiry = output_ops.enquiryEffect;
    effects.size = output_ops.sizeEffect;
    effects.title_changed = output_ops.metadataChangedEffect;
    effects.pwd_changed = output_ops.metadataChangedEffect;
    effects.xtversion = output_ops.versionEffect;
    self.stream.handler.column_mode = output_ops.columnModeEffect;
    self.stream.handler.mode_changed = output_ops.modeChangedEffect;
    self.stream.handler.full_reset = output_ops.fullResetEffect;
    self.stream.handler.clipboard_contents = feedback_ops.clipboardContentsEffect;
    self.stream.handler.progress_report = feedback_ops.progressReportEffect;
    self.stream.handler.semantic_prompt = feedback_ops.semanticPromptEffect;
    self.stream.handler.metadata_reset = output_ops.metadataResetEffect;
    self.stream.handler.terminal_handler.apc_handler.unknown_max_bytes =
        feedback_ops.max_private_application_signal_apc_bytes;
    self.stream.handler.terminal_handler.unknown_sequence =
        feedback_ops.privateApplicationSignalEffect;
    if (resolved_options.output_seed) |seed| output_seed.apply(&self.stream, seed);

    self.wait_thread = try spawner.spawn(wait_thread_ops.waitThread, self);
    self.reader_thread = try spawner.spawn(output_ops.readerThread, self);
    self.writer_thread = try spawner.spawn(writer_ops.writerThread, self);
    self.animation_thread = try spawner.spawn(animation_ops.animationThread, self);
    self.process = try pty_process.Process.spawn(allocator, resolved_launch);
    if (resolved_options.editor_document) |document| {
        self.editor_document = document.take();
    }
    self.process.registerReaderThread(self.reader_thread.?.getHandle());
    self.startup.ready();
    return self;
}

pub fn cleanupFailedThreadStartup(self: *state.State) void {
    // Workers are deliberately gated before process creation. A failed
    // thread start or process spawn therefore has no child owned by this
    // partially-constructed runtime; wake and join only the workers that
    // were successfully created.
    self.startup.abort();
    if (self.wait_thread) |thread| thread.join();
    if (self.reader_thread) |thread| thread.join();
    if (self.writer_thread) |thread| thread.join();
    if (self.animation_thread) |thread| thread.join();
    self.stream.deinit();
    self.stream_allocator.deinit();
    feedback_ops.deinit(self);
    self.history.deinit(&self.terminal);
    self.input_ownership.deinit();
    self.terminal.deinit(self.allocator);
    if (self.kitty_temp_directory) |path| self.allocator.free(path);
    self.input_queue.deinit();
    self.profile.deinit(self.allocator);
    self.projection.deinit();
}
