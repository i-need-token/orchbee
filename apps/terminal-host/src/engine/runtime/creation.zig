const std = @import("std");
const target_builtin = @import("builtin");
const ghostty_vt = @import("ghostty-vt");
const pty_process = @import("terminal-host-platform").process;
const terminal_profile = @import("../profile.zig");
const terminal_runtime_launch = @import("../runtime_launch.zig");
const contracts = @import("contracts.zig");
const state = @import("state.zig");
const output_ops = @import("output.zig");
const writer_ops = @import("writer.zig");
const wait_thread_ops = @import("wait_thread.zig");

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
    const checked = try terminal_runtime_launch.prepare(allocator, launch, options);
    const resolved_launch = checked.process_launch;
    const resolved_options = checked.runtime_options;

    const colors = try terminal_profile.terminalColors(resolved_options.profile);
    const self = try allocator.create(state.State);
    errdefer allocator.destroy(self);

    const resources = resources: {
        var profile = try contracts.OwnedProfile.init(allocator, resolved_options.profile);
        errdefer profile.deinit(allocator);

        var terminal: ghostty_vt.Terminal = try .init(allocator, .{
            .cols = resolved_launch.columns,
            .colors = colors,
            .max_scrollback = resolved_options.scrollback_bytes,
            .rows = resolved_launch.rows,
        });
        errdefer terminal.deinit(allocator);
        break :resources .{
            .profile = profile,
            .terminal = terminal,
        };
    };

    self.* = .{
        .allocator = allocator,
        .cell_pixel_height = resolved_options.cell_pixel_height,
        .input_queue = .init(allocator),
        .input_ownership = .init(allocator),
        .cell_pixel_width = resolved_options.cell_pixel_width,
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
    self.stream = .initAlloc(
        self.stream_allocator.allocator(),
        .init(&self.terminal),
    );
    const effects = &self.stream.handler.terminal_handler.effects;
    effects.bell = output_ops.bellEffect;
    effects.write_pty = output_ops.writePtyEffect;
    effects.color_scheme = output_ops.colorSchemeEffect;
    effects.device_attributes = output_ops.deviceAttributesEffect;
    effects.enquiry = output_ops.enquiryEffect;
    effects.size = output_ops.sizeEffect;
    effects.xtversion = output_ops.versionEffect;
    self.stream.handler.column_mode = output_ops.columnModeEffect;

    self.wait_thread = try spawner.spawn(wait_thread_ops.waitThread, self);
    self.reader_thread = try spawner.spawn(output_ops.readerThread, self);
    self.writer_thread = try spawner.spawn(writer_ops.writerThread, self);
    self.process = try pty_process.Process.spawn(allocator, resolved_launch);
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
    self.stream.deinit();
    self.stream_allocator.deinit();
    self.history.deinit(&self.terminal);
    self.input_ownership.deinit();
    self.terminal.deinit(self.allocator);
    self.input_queue.deinit();
    self.profile.deinit(self.allocator);
    self.projection.deinit();
}
