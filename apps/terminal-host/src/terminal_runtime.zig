const std = @import("std");

const ghostty_vt = @import("ghostty-vt");
const pty_process = @import("pty_process.zig");
const terminal_projection = @import("terminal_projection.zig");

pub const scrollback_bytes = 10_000_000;
pub const max_pending_input_bytes = 4 * 1024 * 1024;
pub const max_pty_read_batch_bytes = 256 * 1024;

pub const max_key_encoded_bytes = 8 * 1024;
pub const max_mouse_encoded_bytes = 128;
pub const max_focus_encoded_bytes = ghostty_vt.input.max_focus_encode_size;

pub const ColorScheme = enum {
    dark,
    light,
};

pub const TerminalProfile = struct {
    color_scheme: ColorScheme,
    default_background_rgb: u32,
    default_cursor_rgb: ?u32,
    default_foreground_rgb: u32,
    device_primary_conformance_level: u16,
    device_primary_features: []const u16,
    device_secondary_type: u16,
    device_secondary_firmware_version: u16,
    device_secondary_rom_cartridge: u16,
    device_tertiary_unit_id: u32,
    enquiry_response: []const u8,
    palette: []const u32,
    version_response: []const u8,
};

pub const Options = struct {
    cell_pixel_height: u16 = 1,
    cell_pixel_width: u16 = 1,
    profile: ?TerminalProfile = null,
};

const OwnedProfile = struct {
    color_scheme: ColorScheme,
    device_primary_conformance_level: u16,
    device_primary_features: []u16,
    device_secondary_type: u16,
    device_secondary_firmware_version: u16,
    device_secondary_rom_cartridge: u16,
    device_tertiary_unit_id: u32,
    enquiry_response: []u8,
    version_response: []u8,

    fn init(allocator: std.mem.Allocator, source: ?TerminalProfile) !OwnedProfile {
        const default_profile: TerminalProfile = .{
            .color_scheme = .dark,
            .default_background_rgb = 0,
            .default_cursor_rgb = null,
            .default_foreground_rgb = 0xffffff,
            .device_primary_conformance_level = 62,
            .device_primary_features = &.{22},
            .device_secondary_type = 1,
            .device_secondary_firmware_version = 0,
            .device_secondary_rom_cartridge = 0,
            .device_tertiary_unit_id = 0,
            .enquiry_response = "",
            .palette = &.{},
            .version_response = "Orchbee",
        };
        const profile = source orelse default_profile;
        const features = try allocator.dupe(u16, profile.device_primary_features);
        errdefer allocator.free(features);
        const enquiry = try allocator.dupe(u8, profile.enquiry_response);
        errdefer allocator.free(enquiry);
        const version = try allocator.dupe(u8, profile.version_response);
        return .{
            .color_scheme = profile.color_scheme,
            .device_primary_conformance_level = profile.device_primary_conformance_level,
            .device_primary_features = features,
            .device_secondary_type = profile.device_secondary_type,
            .device_secondary_firmware_version = profile.device_secondary_firmware_version,
            .device_secondary_rom_cartridge = profile.device_secondary_rom_cartridge,
            .device_tertiary_unit_id = profile.device_tertiary_unit_id,
            .enquiry_response = enquiry,
            .version_response = version,
        };
    }

    fn deinit(self: *OwnedProfile, allocator: std.mem.Allocator) void {
        allocator.free(self.device_primary_features);
        allocator.free(self.enquiry_response);
        allocator.free(self.version_response);
        self.* = undefined;
    }
};

const InputEntry = struct {
    bytes: ?[]u8 = null,
    identifier: u64,
    offset: usize = 0,
    reserved_bytes: usize,
};

const InputReservation = struct {
    identifier: u64,
    reserved_bytes: usize,
};

pub const KeyInput = struct {
    action: u8,
    composing: bool,
    consumed_modifiers: u16,
    key: []const u8,
    modifiers: u16,
    text: []const u8,
    unshifted_codepoint: u32,
};

pub const MouseInput = struct {
    action: u8,
    button: ?u8,
    modifiers: u16,
    pressed_buttons: u16,
    x: f32,
    y: f32,
};

const DeviceAttributesCallback = @typeInfo(@FieldType(
    ghostty_vt.TerminalStream.Handler.Effects,
    "device_attributes",
)).optional.child;
const DeviceAttributes = @typeInfo(@typeInfo(DeviceAttributesCallback).pointer.child).@"fn".return_type.?;
const DeviceFeature = std.meta.Elem(@FieldType(@FieldType(DeviceAttributes, "primary"), "features"));

pub const ProjectionCounters = terminal_projection.Counters;
pub const ProjectionEncodeRequest = terminal_projection.EncodeRequest;
pub const ProjectionFrame = terminal_projection.EncodedFrame;

pub const Geometry = struct {
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    columns: u16,
    generation: u64,
    revision: u64,
    rows: u16,
};

pub const Completion = struct {
    child_exited: bool,
    exit_result: ?pty_process.ExitResult,
    final_revision: u64,
    pty_drained: bool,
    pty_read_failed: bool,
};

pub const TerminalRuntime = struct {
    allocator: std.mem.Allocator,
    cell_pixel_height: u16,
    cell_pixel_width: u16,
    child_exited: std.atomic.Value(bool) = .init(false),
    exit_result: ?pty_process.ExitResult = null,
    generation: u64 = 1,
    input_charged_bytes: usize = 0,
    input_condition: std.Thread.Condition = .{},
    input_entries: std.ArrayList(InputEntry) = .empty,
    input_mutex: std.Thread.Mutex = .{},
    input_next_identifier: u64 = 1,
    reader_failed: std.atomic.Value(bool) = .init(false),
    reader_thread: ?std.Thread = null,
    revision: std.atomic.Value(u64) = .init(0),
    output_batches_since_sync: std.atomic.Value(u64) = .init(0),
    process: pty_process.Process,
    profile: OwnedProfile,
    projection: terminal_projection.Projection,
    projection_mutex: std.Thread.Mutex = .{},
    pty_drained: std.atomic.Value(bool) = .init(false),
    pty_write_mutex: std.Thread.Mutex = .{},
    stream: ghostty_vt.TerminalStream,
    terminal: ghostty_vt.Terminal,
    terminal_mutex: std.Thread.Mutex = .{},
    threads_joined: bool = false,
    wait_thread: ?std.Thread = null,
    writer_stop: bool = false,
    writer_thread: ?std.Thread = null,
    written_since_sync: std.atomic.Value(usize) = .init(0),

    pub fn create(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
    ) !*TerminalRuntime {
        return createWithOptions(allocator, launch, .{});
    }

    pub fn createWithOptions(
        allocator: std.mem.Allocator,
        launch: pty_process.Launch,
        options: Options,
    ) !*TerminalRuntime {
        const self = try allocator.create(TerminalRuntime);
        errdefer allocator.destroy(self);

        var profile = try OwnedProfile.init(allocator, options.profile);
        errdefer profile.deinit(allocator);

        var colors: ghostty_vt.Terminal.Colors = .default;
        if (options.profile) |source| {
            colors.background = .init(rgb(source.default_background_rgb));
            colors.foreground = .init(rgb(source.default_foreground_rgb));
            colors.cursor = if (source.default_cursor_rgb) |value| .init(rgb(value)) else .unset;
            var palette: ghostty_vt.color.Palette = undefined;
            for (&palette, source.palette) |*target, value| target.* = rgb(value);
            colors.palette = .init(palette);
        }

        var terminal: ghostty_vt.Terminal = try .init(allocator, .{
            .cols = launch.columns,
            .colors = colors,
            .max_scrollback = scrollback_bytes,
            .rows = launch.rows,
        });
        errdefer terminal.deinit(allocator);
        var process = try pty_process.Process.spawn(allocator, launch);
        errdefer process.deinit();

        self.* = .{
            .allocator = allocator,
            .cell_pixel_height = options.cell_pixel_height,
            .cell_pixel_width = options.cell_pixel_width,
            .process = process,
            .profile = profile,
            .projection = .init(allocator),
            .stream = undefined,
            .terminal = terminal,
        };
        self.stream = .init(self.terminal.vtHandler());
        self.stream.handler.effects.write_pty = writePtyEffect;
        self.stream.handler.effects.color_scheme = colorSchemeEffect;
        self.stream.handler.effects.device_attributes = deviceAttributesEffect;
        self.stream.handler.effects.enquiry = enquiryEffect;
        self.stream.handler.effects.size = sizeEffect;
        self.stream.handler.effects.xtversion = versionEffect;

        self.wait_thread = std.Thread.spawn(.{}, waitThread, .{self}) catch |err| {
            self.process.terminate() catch {};
            _ = self.process.wait();
            self.stream.deinit();
            self.process.deinit();
            self.terminal.deinit(allocator);
            self.projection.deinit();
            return err;
        };
        self.reader_thread = std.Thread.spawn(.{}, readerThread, .{self}) catch |err| {
            self.process.terminate() catch {};
            self.wait_thread.?.join();
            self.stream.deinit();
            self.process.deinit();
            self.terminal.deinit(allocator);
            self.projection.deinit();
            return err;
        };
        self.writer_thread = std.Thread.spawn(.{}, writerThread, .{self}) catch |err| {
            self.process.terminate() catch {};
            self.wait_thread.?.join();
            self.reader_thread.?.join();
            self.stream.deinit();
            self.process.deinit();
            self.terminal.deinit(allocator);
            self.projection.deinit();
            return err;
        };
        return self;
    }

    pub fn destroy(self: *TerminalRuntime) void {
        self.joinThreads();
        self.stream.deinit();
        self.process.deinit();
        self.terminal.deinit(self.allocator);
        self.input_mutex.lock();
        for (self.input_entries.items) |entry| if (entry.bytes) |bytes| self.allocator.free(bytes);
        self.input_entries.deinit(self.allocator);
        self.input_mutex.unlock();
        self.profile.deinit(self.allocator);
        self.projection.deinit();
        const allocator = self.allocator;
        self.* = undefined;
        allocator.destroy(self);
    }

    pub fn completion(self: *const TerminalRuntime) Completion {
        const child_exited = self.child_exited.load(.acquire);
        return .{
            .child_exited = child_exited,
            .exit_result = if (child_exited) self.exit_result else null,
            .final_revision = self.revision.load(.acquire),
            .pty_drained = self.pty_drained.load(.acquire),
            .pty_read_failed = self.reader_failed.load(.acquire),
        };
    }

    pub fn joinThreads(self: *TerminalRuntime) void {
        if (self.threads_joined) return;
        if (self.wait_thread) |thread| thread.join();
        if (self.reader_thread) |thread| thread.join();
        self.input_mutex.lock();
        self.writer_stop = true;
        self.input_condition.broadcast();
        self.input_mutex.unlock();
        if (self.writer_thread) |thread| thread.join();
        self.threads_joined = true;
    }

    pub fn enqueueInput(self: *TerminalRuntime, bytes: []const u8) !void {
        _ = try self.enqueueRawInput(bytes);
    }

    pub fn enqueueRawInput(self: *TerminalRuntime, bytes: []const u8) !usize {
        if (bytes.len == 0) return error.EmptyInput;
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        const reservation = self.reserveInput(bytes.len, false) catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        self.terminal_mutex.unlock();
        const copy = self.allocator.dupe(u8, bytes) catch |err| {
            self.cancelInput(reservation.identifier);
            return err;
        };
        self.publishInput(reservation, copy);
        return copy.len;
    }

    pub fn enqueueKeyInput(self: *TerminalRuntime, input: KeyInput) !usize {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        const reservation = self.reserveInput(max_key_encoded_bytes, false) catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        const options = ghostty_vt.input.KeyEncodeOptions.fromTerminal(&self.terminal);
        self.terminal_mutex.unlock();
        errdefer self.cancelInput(reservation.identifier);

        const key = ghostty_vt.input.Key.fromW3C(input.key) orelse return error.InvalidKey;
        const buffer = try self.allocator.alloc(u8, max_key_encoded_bytes);
        errdefer self.allocator.free(buffer);
        var writer: std.Io.Writer = .fixed(buffer);
        try ghostty_vt.input.encodeKey(&writer, .{
            .action = @enumFromInt(input.action),
            .composing = input.composing,
            .consumed_mods = modifiers(input.consumed_modifiers),
            .key = key,
            .mods = modifiers(input.modifiers),
            .unshifted_codepoint = @intCast(input.unshifted_codepoint),
            .utf8 = input.text,
        }, options);
        const encoded = try self.allocator.dupe(u8, writer.buffered());
        self.allocator.free(buffer);
        self.publishInput(reservation, encoded);
        return encoded.len;
    }

    pub fn enqueueTextInput(self: *TerminalRuntime, text: []const u8) !usize {
        if (text.len == 0) return error.EmptyInput;
        return self.enqueueOrderedBytes(text, text.len);
    }

    pub fn enqueueMouseInput(self: *TerminalRuntime, input: MouseInput) !usize {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        const reservation = self.reserveInput(max_mouse_encoded_bytes, false) catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        var options = ghostty_vt.input.MouseEncodeOptions.fromTerminal(&self.terminal, .{
            .screen = .{
                .height = @as(u32, self.terminal.rows) * self.cell_pixel_height,
                .width = @as(u32, self.terminal.cols) * self.cell_pixel_width,
            },
            .cell = .{
                .height = self.cell_pixel_height,
                .width = self.cell_pixel_width,
            },
            .padding = .{ .top = 0, .bottom = 0, .left = 0, .right = 0 },
        });
        options.any_button_pressed = input.pressed_buttons != 0;
        self.terminal_mutex.unlock();
        errdefer self.cancelInput(reservation.identifier);

        const buffer = try self.allocator.alloc(u8, max_mouse_encoded_bytes);
        errdefer self.allocator.free(buffer);
        var writer: std.Io.Writer = .fixed(buffer);
        try ghostty_vt.input.encodeMouse(&writer, .{
            .action = @enumFromInt(input.action),
            .button = if (input.button) |button| @enumFromInt(button) else null,
            .mods = modifiers(input.modifiers),
            .pos = .{ .x = input.x, .y = input.y },
        }, options);
        const encoded = try self.allocator.dupe(u8, writer.buffered());
        self.allocator.free(buffer);
        self.publishInput(reservation, encoded);
        return encoded.len;
    }

    pub fn enqueueFocusInput(self: *TerminalRuntime, focused: bool) !usize {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        const reservation = self.reserveInput(ghostty_vt.input.max_focus_encode_size, false) catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        const report = self.terminal.modes.get(.focus_event);
        self.terminal_mutex.unlock();
        errdefer self.cancelInput(reservation.identifier);

        var buffer: [ghostty_vt.input.max_focus_encode_size]u8 = undefined;
        var writer: std.Io.Writer = .fixed(&buffer);
        if (report) try ghostty_vt.input.encodeFocus(&writer, if (focused) .gained else .lost);
        const encoded = try self.allocator.dupe(u8, writer.buffered());
        self.publishInput(reservation, encoded);
        return encoded.len;
    }

    pub fn enqueuePasteInput(self: *TerminalRuntime, text: []const u8) !usize {
        if (text.len == 0) return error.EmptyInput;
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        const maximum = std.math.add(usize, text.len, 12) catch return error.InputBackpressure;
        self.terminal_mutex.lock();
        const reservation = self.reserveInput(maximum, false) catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        const options = ghostty_vt.input.PasteOptions.fromTerminal(&self.terminal);
        self.terminal_mutex.unlock();
        errdefer self.cancelInput(reservation.identifier);

        const mutable = try self.allocator.dupe(u8, text);
        defer self.allocator.free(mutable);
        const parts = ghostty_vt.input.encodePaste(mutable, options);
        const total = parts[0].len + parts[1].len + parts[2].len;
        const encoded = try self.allocator.alloc(u8, total);
        var offset: usize = 0;
        for (parts) |part| {
            @memcpy(encoded[offset..][0..part.len], part);
            offset += part.len;
        }
        self.publishInput(reservation, encoded);
        return encoded.len;
    }

    pub fn pendingInputBytes(self: *TerminalRuntime) usize {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        return self.input_charged_bytes;
    }

    pub fn takeWrittenBytes(self: *TerminalRuntime) usize {
        return self.written_since_sync.swap(0, .acq_rel);
    }

    pub fn takeOutputBatches(self: *TerminalRuntime) u64 {
        return self.output_batches_since_sync.swap(0, .acq_rel);
    }

    pub fn resize(self: *TerminalRuntime, columns: u16, rows: u16) !void {
        _ = try self.resizeGeometry(columns, rows, self.cell_pixel_width, self.cell_pixel_height);
    }

    pub fn resizeGeometry(
        self: *TerminalRuntime,
        columns: u16,
        rows: u16,
        cell_pixel_width: u16,
        cell_pixel_height: u16,
    ) !u64 {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        const old_columns = self.terminal.cols;
        const old_rows = self.terminal.rows;
        const cells_changed = old_columns != columns or old_rows != rows;
        const pixels_changed = self.cell_pixel_width != cell_pixel_width or
            self.cell_pixel_height != cell_pixel_height;
        if (!cells_changed and !pixels_changed) return self.revision.load(.acquire);
        try self.process.resizePixels(columns, rows, cell_pixel_width, cell_pixel_height);
        if (cells_changed) {
            self.terminal.resize(self.allocator, columns, rows) catch |err| {
                self.process.resizePixels(
                    old_columns,
                    old_rows,
                    self.cell_pixel_width,
                    self.cell_pixel_height,
                ) catch {};
                return err;
            };
        }
        self.cell_pixel_width = cell_pixel_width;
        self.cell_pixel_height = cell_pixel_height;
        self.generation = std.math.add(u64, self.generation, 1) catch return error.RevisionOverflow;
        const revision = std.math.add(u64, self.revision.load(.monotonic), 1) catch
            return error.RevisionOverflow;
        self.revision.store(revision, .release);
        return revision;
    }

    pub fn terminate(self: *const TerminalRuntime) !void {
        try self.process.terminate();
    }

    pub fn plainString(self: *TerminalRuntime, allocator: std.mem.Allocator) ![]const u8 {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        return self.terminal.plainString(allocator);
    }

    pub fn geometry(self: *TerminalRuntime) Geometry {
        self.terminal_mutex.lock();
        defer self.terminal_mutex.unlock();
        return .{
            .cell_pixel_height = self.cell_pixel_height,
            .cell_pixel_width = self.cell_pixel_width,
            .columns = self.terminal.cols,
            .generation = self.generation,
            .revision = self.revision.load(.monotonic),
            .rows = self.terminal.rows,
        };
    }

    pub fn projectionCounters(self: *TerminalRuntime) ProjectionCounters {
        self.projection_mutex.lock();
        defer self.projection_mutex.unlock();
        return self.projection.counters;
    }

    pub fn encodeProjectionFrame(
        self: *TerminalRuntime,
        request: ProjectionEncodeRequest,
    ) !ProjectionFrame {
        self.projection_mutex.lock();
        defer self.projection_mutex.unlock();

        self.terminal_mutex.lock();
        const revision = self.revision.load(.monotonic);
        const generation = self.generation;
        if (self.projection.needsCapture(revision, generation)) {
            self.projection.beginCapture(
                &self.terminal,
                revision,
                generation,
                self.cell_pixel_width,
                self.cell_pixel_height,
            ) catch |err| {
                self.terminal_mutex.unlock();
                self.projection.resetAfterCaptureFailure();
                return err;
            };
            self.terminal_mutex.unlock();
            self.projection.endCapture();
        } else {
            self.terminal_mutex.unlock();
        }
        return self.projection.encode(request);
    }

    fn enqueueOrderedBytes(self: *TerminalRuntime, bytes: []const u8, reserved_bytes: usize) !usize {
        if (self.child_exited.load(.acquire)) return error.TerminalExited;
        self.terminal_mutex.lock();
        const reservation = self.reserveInput(reserved_bytes, false) catch |err| {
            self.terminal_mutex.unlock();
            return err;
        };
        self.terminal_mutex.unlock();
        const copy = self.allocator.dupe(u8, bytes) catch |err| {
            self.cancelInput(reservation.identifier);
            return err;
        };
        self.publishInput(reservation, copy);
        return copy.len;
    }

    fn reserveInput(self: *TerminalRuntime, byte_count: usize, wait: bool) !InputReservation {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        while (byte_count > max_pending_input_bytes -| self.input_charged_bytes) {
            if (!wait) return error.InputBackpressure;
            if (self.writer_stop) return error.TerminalExited;
            self.input_condition.wait(&self.input_mutex);
        }
        const identifier = self.input_next_identifier;
        self.input_next_identifier = std.math.add(u64, identifier, 1) catch
            return error.InputBackpressure;
        try self.input_entries.append(self.allocator, .{
            .identifier = identifier,
            .reserved_bytes = byte_count,
        });
        self.input_charged_bytes += byte_count;
        return .{ .identifier = identifier, .reserved_bytes = byte_count };
    }

    fn publishInput(self: *TerminalRuntime, reservation: InputReservation, bytes: []u8) void {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        for (self.input_entries.items) |*entry| {
            if (entry.identifier != reservation.identifier) continue;
            std.debug.assert(entry.bytes == null);
            std.debug.assert(bytes.len <= reservation.reserved_bytes);
            self.input_charged_bytes -= reservation.reserved_bytes - bytes.len;
            entry.reserved_bytes = bytes.len;
            entry.bytes = bytes;
            self.input_condition.broadcast();
            return;
        }
        self.allocator.free(bytes);
    }

    fn cancelInput(self: *TerminalRuntime, identifier: u64) void {
        self.input_mutex.lock();
        defer self.input_mutex.unlock();
        for (self.input_entries.items, 0..) |entry, index| {
            if (entry.identifier != identifier) continue;
            self.input_charged_bytes -= entry.reserved_bytes;
            if (entry.bytes) |bytes| self.allocator.free(bytes);
            _ = self.input_entries.orderedRemove(index);
            self.input_condition.broadcast();
            return;
        }
    }

    fn writeAll(self: *TerminalRuntime, bytes: []const u8) !void {
        self.pty_write_mutex.lock();
        defer self.pty_write_mutex.unlock();
        var offset: usize = 0;
        while (offset < bytes.len) {
            const written = try self.process.write(bytes[offset..]);
            if (written == 0) return error.WriteZero;
            offset += written;
        }
    }

    fn writePtyEffect(handler: *ghostty_vt.TerminalStream.Handler, bytes: [:0]const u8) void {
        const self = runtimeFromHandler(handler);
        const reservation = self.reserveInput(bytes.len, true) catch return;
        const copy = self.allocator.dupe(u8, bytes) catch {
            self.cancelInput(reservation.identifier);
            return;
        };
        self.publishInput(reservation, copy);
    }

    fn colorSchemeEffect(
        handler: *ghostty_vt.TerminalStream.Handler,
    ) ?ghostty_vt.device_status.ColorScheme {
        return switch (runtimeFromHandler(handler).profile.color_scheme) {
            .dark => .dark,
            .light => .light,
        };
    }

    fn deviceAttributesEffect(handler: *ghostty_vt.TerminalStream.Handler) DeviceAttributes {
        const profile = &runtimeFromHandler(handler).profile;
        return .{
            .primary = .{
                .conformance_level = @enumFromInt(profile.device_primary_conformance_level),
                .features = @ptrCast(profile.device_primary_features),
            },
            .secondary = .{
                .device_type = @enumFromInt(profile.device_secondary_type),
                .firmware_version = profile.device_secondary_firmware_version,
                .rom_cartridge = profile.device_secondary_rom_cartridge,
            },
            .tertiary = .{ .unit_id = profile.device_tertiary_unit_id },
        };
    }

    fn enquiryEffect(handler: *ghostty_vt.TerminalStream.Handler) []const u8 {
        return runtimeFromHandler(handler).profile.enquiry_response;
    }

    fn sizeEffect(
        handler: *ghostty_vt.TerminalStream.Handler,
    ) ?ghostty_vt.size_report.Size {
        const self = runtimeFromHandler(handler);
        return .{
            .cell_height = self.cell_pixel_height,
            .cell_width = self.cell_pixel_width,
            .columns = self.terminal.cols,
            .rows = self.terminal.rows,
        };
    }

    fn versionEffect(handler: *ghostty_vt.TerminalStream.Handler) []const u8 {
        return runtimeFromHandler(handler).profile.version_response;
    }

    fn readerThread(self: *TerminalRuntime) void {
        var buffer: [max_pty_read_batch_bytes]u8 = undefined;
        while (true) {
            const length = self.process.read(&buffer) catch {
                self.reader_failed.store(true, .release);
                return;
            };
            if (length == 0) break;
            self.terminal_mutex.lock();
            const previous_screen = self.terminal.screens.active_key;
            self.stream.nextSlice(buffer[0..length]);
            if (previous_screen != self.terminal.screens.active_key) {
                self.generation = std.math.add(u64, self.generation, 1) catch {
                    self.terminal_mutex.unlock();
                    self.reader_failed.store(true, .release);
                    return;
                };
            }
            const revision = std.math.add(u64, self.revision.load(.monotonic), 1) catch {
                self.terminal_mutex.unlock();
                self.reader_failed.store(true, .release);
                return;
            };
            self.revision.store(revision, .release);
            self.terminal_mutex.unlock();
            _ = self.output_batches_since_sync.fetchAdd(1, .release);
        }
        self.pty_drained.store(true, .release);
    }

    fn waitThread(self: *TerminalRuntime) void {
        self.exit_result = self.process.wait();
        self.child_exited.store(true, .release);
        self.input_mutex.lock();
        self.writer_stop = true;
        self.input_condition.broadcast();
        self.input_mutex.unlock();
    }

    fn writerThread(self: *TerminalRuntime) void {
        var batch: [64 * 1024]u8 = undefined;
        while (true) {
            self.input_mutex.lock();
            while (!self.writer_stop and
                (self.input_entries.items.len == 0 or self.input_entries.items[0].bytes == null))
            {
                self.input_condition.wait(&self.input_mutex);
            }
            if (self.writer_stop) {
                self.input_mutex.unlock();
                return;
            }
            const entry = &self.input_entries.items[0];
            const bytes = entry.bytes.?;
            if (bytes.len == 0) {
                self.allocator.free(bytes);
                _ = self.input_entries.orderedRemove(0);
                self.input_condition.broadcast();
                self.input_mutex.unlock();
                continue;
            }
            const available = bytes[entry.offset..];
            const length = @min(batch.len, available.len);
            @memcpy(batch[0..length], available[0..length]);
            self.input_mutex.unlock();

            self.writeAll(batch[0..length]) catch {
                self.input_mutex.lock();
                self.writer_stop = true;
                self.input_condition.broadcast();
                self.input_mutex.unlock();
                return;
            };

            self.input_mutex.lock();
            const current = &self.input_entries.items[0];
            current.offset += length;
            if (current.offset == current.bytes.?.len) {
                self.input_charged_bytes -= current.reserved_bytes;
                self.allocator.free(current.bytes.?);
                _ = self.input_entries.orderedRemove(0);
                self.input_condition.broadcast();
            }
            self.input_mutex.unlock();
            _ = self.written_since_sync.fetchAdd(length, .release);
        }
    }
};

fn runtimeFromHandler(handler: *ghostty_vt.TerminalStream.Handler) *TerminalRuntime {
    const stream: *ghostty_vt.TerminalStream = @fieldParentPtr("handler", handler);
    return @fieldParentPtr("stream", stream);
}

fn modifiers(flags: u16) ghostty_vt.input.KeyMods {
    return @bitCast(flags);
}

fn rgb(value: u32) ghostty_vt.color.RGB {
    return .{
        .r = @truncate(value >> 16),
        .g = @truncate(value >> 8),
        .b = @truncate(value),
    };
}

test "runtime parses PTY output before exposing canonical terminal state" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf '\\033[31mhost\\033[0m:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =host:!value!",
    };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    try runtime.enqueueInput(if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n");
    runtime.joinThreads();
    const completion = runtime.completion();
    try std.testing.expect(completion.child_exited);
    try std.testing.expect(completion.pty_drained);
    try std.testing.expect(!completion.pty_read_failed);
    try std.testing.expect(completion.final_revision > 0);
    try std.testing.expectEqual(pty_process.ExitResult{ .exit_code = 0 }, completion.exit_result.?);

    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "host:orchbee") != null);
}

test "runtime queue applies bounded backpressure before allocation" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{ "-c", "sleep 1" };
    const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "ping -n 3 127.0.0.1 >nul" };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const bytes = try std.testing.allocator.alloc(u8, max_pending_input_bytes + 1);
    defer std.testing.allocator.free(bytes);
    @memset(bytes, 'x');
    try std.testing.expectError(error.InputBackpressure, runtime.enqueueInput(bytes));
    try runtime.terminate();
}

test "ordered input reservations prevent overtaking and cancellation leaves no gap" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "read value; printf '<%s>' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "set /p value=& <nul set /p =^<!value!^>",
    };
    const arguments = if (builtin.os.tag == .windows) &windows_arguments else &posix_arguments;
    const runtime = try TerminalRuntime.create(std.testing.allocator, .{
        .arguments = arguments,
        .columns = 80,
        .current_directory = null,
        .environment = &.{},
        .executable = executable,
        .rows = 24,
    });
    defer runtime.destroy();

    const first = try runtime.reserveInput(1, false);
    const cancelled = try runtime.reserveInput(1, false);
    const third = try runtime.reserveInput(2, false);
    runtime.publishInput(third, try std.testing.allocator.dupe(u8, if (builtin.os.tag == .windows) "C\r" else "C\n"));
    runtime.cancelInput(cancelled.identifier);
    std.Thread.sleep(10 * std.time.ns_per_ms);
    try std.testing.expectEqual(@as(usize, 3), runtime.pendingInputBytes());
    runtime.publishInput(first, try std.testing.allocator.dupe(u8, "A"));

    runtime.joinThreads();
    const plain = try runtime.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "<AC>") != null);
}
