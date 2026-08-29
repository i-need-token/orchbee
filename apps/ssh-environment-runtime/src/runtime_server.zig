const std = @import("std");

const build_options = @import("build-options");
const private_ipc = @import("ssh-runtime-private-ipc");
const project_root = @import("ssh-project-root-service");
const protocol = @import("ssh-environment-protocol");
const terminal_host = @import("ssh-terminal-host-relay");

pub const runtime_version = build_options.runtime_version;
pub const runtime_platform = build_options.runtime_platform;

const poll_interval_ms = 100;
const heartbeat_timeout_ns =
    protocol.heartbeat_interval_ms * protocol.heartbeat_miss_limit * std.time.ns_per_ms;
const maximum_poll_descriptors = 4 + project_root.maximum_concurrent_operations;

const AttachmentPhase = enum {
    awaiting_hello,
    active,
};

const ChannelPhase = enum {
    opened,
    running,
    result_published,
};

const Channel = struct {
    phase: ChannelPhase = .opened,
    request_credit: u32 = protocol.project_root_initial_channel_credit_bytes,
    result_credit: u32 = protocol.project_root_initial_channel_credit_bytes,
};

pub const TerminalHostConfiguration = struct {
    build_digest: []const u8,
    endpoint_id: []const u8,
    executable_path: []const u8,
    runtime_directory: []const u8,
    version: []const u8,
};

const TerminalHostRelayPhase = enum { starting, open };

const TerminalHostRelay = struct {
    desktop_credit: u32,
    host_input: [protocol.initial_channel_credit_bytes]u8 = undefined,
    host_input_credit: u32 = protocol.initial_channel_credit_bytes,
    host_input_length: usize = 0,
    host_input_offset: usize = 0,
    phase: TerminalHostRelayPhase = .starting,
    worker: terminal_host.Worker,

    fn terminate(self: *TerminalHostRelay, io: std.Io) void {
        self.worker.terminate(io);
        self.* = undefined;
    }
};

const Attachment = struct {
    channels: std.AutoHashMap(u32, Channel),
    generation: u64,
    last_heartbeat: std.Io.Timestamp,
    phase: AttachmentPhase = .awaiting_hello,
    terminal_host_relay: ?TerminalHostRelay = null,
    stream: std.Io.net.Stream,
    used_channels: std.AutoHashMap(u32, void),

    fn init(
        allocator: std.mem.Allocator,
        stream: std.Io.net.Stream,
        generation: u64,
        now: std.Io.Timestamp,
    ) Attachment {
        return .{
            .channels = .init(allocator),
            .generation = generation,
            .last_heartbeat = now,
            .stream = stream,
            .used_channels = .init(allocator),
        };
    }

    fn close(self: *Attachment, io: std.Io) void {
        if (self.terminal_host_relay) |*relay| relay.terminate(io);
        self.stream.close(io);
        self.channels.deinit();
        self.used_channels.deinit();
        self.* = undefined;
    }
};

const WorkerPoll = struct {
    descriptor_index: usize,
    slot_index: usize,
};

pub fn serve(
    io: std.Io,
    allocator: std.mem.Allocator,
    home_directory: []const u8,
    runtime_directory: []const u8,
    runtime_instance_id: []const u8,
    runtime_build_digest: []const u8,
    release_cohort_digest: []const u8,
    terminal_host_configuration: TerminalHostConfiguration,
) !void {
    const executable_path = try std.process.executablePathAlloc(io, allocator);
    defer allocator.free(executable_path);
    try private_ipc.prepareRuntimeDirectory(runtime_directory);
    const lock_descriptor = try private_ipc.acquireLifetimeLock(
        runtime_directory,
        runtime_instance_id,
    );
    defer _ = std.c.close(lock_descriptor);

    var directory = try std.Io.Dir.openDirAbsolute(io, runtime_directory, .{
        .follow_symlinks = false,
    });
    defer directory.close(io);
    if (std.c.fchdir(directory.handle) != 0) return error.RuntimeDirectoryUnavailable;

    const endpoint_name = try endpointName(allocator, runtime_instance_id);
    defer allocator.free(endpoint_name);
    const unpublished_endpoint_name = try unpublishedEndpointName(allocator, runtime_instance_id);
    defer allocator.free(unpublished_endpoint_name);
    directory.deleteFile(io, endpoint_name) catch |err| switch (err) {
        error.FileNotFound => {},
        else => return err,
    };
    directory.deleteFile(io, unpublished_endpoint_name) catch |err| switch (err) {
        error.FileNotFound => {},
        else => return err,
    };
    defer directory.deleteFile(io, endpoint_name) catch {};
    defer directory.deleteFile(io, unpublished_endpoint_name) catch {};

    const address = std.Io.net.UnixAddress.init(unpublished_endpoint_name) catch
        return error.EndpointPathTooLong;
    var listener = try address.listen(io, .{ .kernel_backlog = 8 });
    defer listener.deinit(io);
    try private_ipc.secureEndpoint(unpublished_endpoint_name);
    try directory.rename(
        unpublished_endpoint_name,
        directory,
        endpoint_name,
        io,
    );
    try private_ipc.validateEndpoint(endpoint_name);

    var epoch_bytes: [16]u8 = undefined;
    io.random(&epoch_bytes);
    const runtime_epoch = std.fmt.bytesToHex(epoch_bytes, .lower);
    var attachment: ?Attachment = null;
    var next_attachment_generation: u64 = 1;
    var workers = [_]?project_root.Worker{null} ** project_root.maximum_concurrent_operations;
    defer terminateAllWorkers(io, &workers);
    defer if (attachment) |*active| active.close(io);

    while (true) {
        if (attachment == null) {
            const accepted = try listener.accept(io);
            private_ipc.requirePeerEffectiveUid(accepted.socket.handle) catch {
                accepted.close(io);
                continue;
            };
            attachment = Attachment.init(
                allocator,
                accepted,
                next_attachment_generation,
                .now(io, .awake),
            );
            next_attachment_generation +%= 1;
            if (next_attachment_generation == 0) next_attachment_generation = 1;
            continue;
        }

        var descriptors: [maximum_poll_descriptors]std.posix.pollfd = undefined;
        descriptors[0] = .{
            .fd = listener.socket.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        };
        descriptors[1] = .{
            .fd = attachment.?.stream.socket.handle,
            .events = std.posix.POLL.IN,
            .revents = 0,
        };
        var descriptor_count: usize = 2;
        var terminal_host_output_descriptor_index: ?usize = null;
        var terminal_host_input_descriptor_index: ?usize = null;
        if (attachment.?.terminal_host_relay) |*relay| {
            if (relay.phase == .starting or relay.desktop_credit > 0) {
                terminal_host_output_descriptor_index = descriptor_count;
                descriptors[descriptor_count] = .{
                    .fd = relay.worker.outputDescriptor(),
                    .events = std.posix.POLL.IN,
                    .revents = 0,
                };
                descriptor_count += 1;
            }
            if (relay.host_input_offset < relay.host_input_length) {
                terminal_host_input_descriptor_index = descriptor_count;
                descriptors[descriptor_count] = .{
                    .fd = relay.worker.inputDescriptor(),
                    .events = std.posix.POLL.OUT,
                    .revents = 0,
                };
                descriptor_count += 1;
            }
        }
        var worker_polls: [project_root.maximum_concurrent_operations]WorkerPoll = undefined;
        var worker_poll_count: usize = 0;
        for (&workers, 0..) |*worker_slot, slot_index| {
            if (worker_slot.*) |*worker| {
                descriptors[descriptor_count] = .{
                    .fd = worker.outputDescriptor(),
                    .events = std.posix.POLL.IN,
                    .revents = 0,
                };
                worker_polls[worker_poll_count] = .{
                    .descriptor_index = descriptor_count,
                    .slot_index = slot_index,
                };
                descriptor_count += 1;
                worker_poll_count += 1;
            }
        }
        _ = try std.posix.poll(descriptors[0..descriptor_count], poll_interval_ms);

        if (descriptors[0].revents & std.posix.POLL.IN != 0) {
            const competing = try listener.accept(io);
            defer competing.close(io);
            if (private_ipc.requirePeerEffectiveUid(competing.socket.handle)) |_| {
                protocol.writeFailure(
                    competing.socket.handle,
                    "runtime-attachment-conflict",
                ) catch {};
            } else |_| {}
        }

        const active_events = descriptors[1].revents;
        if (active_events & std.posix.POLL.IN != 0) {
            const keep_attachment = dispatch(
                io,
                allocator,
                &attachment.?,
                &workers,
                executable_path,
                home_directory,
                runtime_instance_id,
                &runtime_epoch,
                runtime_build_digest,
                release_cohort_digest,
                terminal_host_configuration,
            ) catch |err| failed: {
                std.log.warn(
                    "retiring SSH Runtime attachment after dispatch failure: {s}",
                    .{@errorName(err)},
                );
                break :failed false;
            };
            if (!keep_attachment) {
                retireAttachment(io, &attachment, &workers);
                continue;
            }
        }
        if (active_events & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
            retireAttachment(io, &attachment, &workers);
            continue;
        }

        if (terminal_host_output_descriptor_index) |descriptor_index| {
            if (attachment.?.terminal_host_relay != null and
                descriptors[descriptor_index].revents != 0)
            {
                processTerminalHostOutputEvents(
                    io,
                    allocator,
                    &attachment.?,
                    &runtime_epoch,
                    descriptors[descriptor_index].revents,
                ) catch {
                    retireAttachment(io, &attachment, &workers);
                    continue;
                };
            }
        }
        if (terminal_host_input_descriptor_index) |descriptor_index| {
            if (attachment.?.terminal_host_relay != null and
                descriptors[descriptor_index].revents != 0)
            {
                processTerminalHostInputEvents(
                    io,
                    allocator,
                    &attachment.?,
                    &runtime_epoch,
                    descriptors[descriptor_index].revents,
                ) catch {
                    retireAttachment(io, &attachment, &workers);
                    continue;
                };
            }
        }

        var worker_failed_attachment = false;
        for (worker_polls[0..worker_poll_count]) |worker_poll| {
            const worker_events = descriptors[worker_poll.descriptor_index].revents;
            if (worker_events == 0 or workers[worker_poll.slot_index] == null) continue;
            _ = processWorkerEvents(
                io,
                allocator,
                &attachment.?,
                &workers,
                worker_poll.slot_index,
                worker_events,
            ) catch {
                worker_failed_attachment = true;
                break;
            };
        }
        if (worker_failed_attachment) {
            retireAttachment(io, &attachment, &workers);
            continue;
        }
        processWorkerTimeouts(io, allocator, &attachment.?, &workers) catch {
            retireAttachment(io, &attachment, &workers);
            continue;
        };

        if (attachment) |*active| {
            const elapsed = active.last_heartbeat.durationTo(.now(io, .awake));
            const timeout_ns: @TypeOf(elapsed.nanoseconds) = if (active.phase == .active)
                heartbeat_timeout_ns
            else
                protocol.heartbeat_interval_ms * std.time.ns_per_ms;
            if (elapsed.nanoseconds > timeout_ns) {
                retireAttachment(io, &attachment, &workers);
            }
        }
    }
}

fn dispatch(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    executable_path: []const u8,
    home_directory: []const u8,
    runtime_instance_id: []const u8,
    runtime_epoch: []const u8,
    runtime_build_digest: []const u8,
    release_cohort_digest: []const u8,
    terminal_host_configuration: TerminalHostConfiguration,
) !bool {
    var frame = protocol.readFrame(io, allocator, attachment.stream.socket.handle) catch |err| {
        const code = if (err == error.UnsupportedProtocolVersion)
            "protocol-version-unsupported"
        else
            "protocol-invalid";
        protocol.writeFailure(attachment.stream.socket.handle, code) catch {};
        return false;
    };
    defer frame.deinit();

    switch (attachment.phase) {
        .awaiting_hello => {
            if (frame.kind != .hello) {
                try protocol.writeFailure(attachment.stream.socket.handle, "protocol-invalid");
                return false;
            }
            var hello = protocol.decodeHello(allocator, frame.payload) catch |err| {
                const code = if (err == error.UnsupportedProtocolVersion)
                    "protocol-version-unsupported"
                else
                    "protocol-invalid";
                protocol.writeFailure(attachment.stream.socket.handle, code) catch {};
                return false;
            };
            defer hello.deinit();
            if (!std.mem.eql(u8, hello.value.runtimeInstanceId, runtime_instance_id)) {
                try protocol.writeFailure(
                    attachment.stream.socket.handle,
                    "runtime-instance-mismatch",
                );
                return false;
            }
            try protocol.writeHelloAccepted(
                allocator,
                attachment.stream.socket.handle,
                runtime_instance_id,
                runtime_epoch,
                runtime_version,
                runtime_build_digest,
                release_cohort_digest,
                terminal_host_configuration.version,
                terminal_host_configuration.build_digest,
                runtime_platform,
            );
            attachment.phase = .active;
            attachment.last_heartbeat = .now(io, .awake);
            return true;
        },
        .active => switch (frame.kind) {
            .heartbeat => return dispatchHeartbeat(
                io,
                allocator,
                attachment,
                runtime_epoch,
                frame.payload,
            ),
            .channel_open => return dispatchChannelOpen(
                io,
                allocator,
                attachment,
                runtime_epoch,
                executable_path,
                terminal_host_configuration,
                frame.payload,
            ),
            .channel_data => return dispatchChannelData(
                io,
                allocator,
                attachment,
                workers,
                executable_path,
                home_directory,
                frame.payload,
            ),
            .channel_credit => return dispatchChannelCredit(
                attachment,
                frame.payload,
            ),
            .channel_close => return dispatchChannelClose(
                io,
                allocator,
                attachment,
                workers,
                runtime_epoch,
                frame.payload,
            ),
            .channel_opened,
            .channel_closed,
            => return false,
            else => return false,
        },
    }
}

fn dispatchHeartbeat(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    runtime_epoch: []const u8,
    payload: []const u8,
) !bool {
    var heartbeat = protocol.decodeHeartbeat(allocator, payload) catch return false;
    defer heartbeat.deinit();
    if (!std.mem.eql(u8, heartbeat.value.runtimeEpoch, runtime_epoch)) return false;
    try protocol.writeHeartbeatAcknowledged(
        allocator,
        attachment.stream.socket.handle,
        runtime_epoch,
        heartbeat.value.sequence,
    );
    attachment.last_heartbeat = .now(io, .awake);
    return true;
}

fn dispatchChannelOpen(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    runtime_epoch: []const u8,
    executable_path: []const u8,
    terminal_host_configuration: TerminalHostConfiguration,
    payload: []const u8,
) !bool {
    var opened = protocol.decodeChannelOpen(allocator, payload) catch return false;
    defer opened.deinit();
    if (!std.mem.eql(u8, opened.value.runtimeEpoch, runtime_epoch) or
        attachment.used_channels.contains(opened.value.channelId))
    {
        return false;
    }
    try attachment.used_channels.put(opened.value.channelId, {});
    if (std.mem.eql(u8, opened.value.serviceId, protocol.project_root_service_id)) {
        if (attachment.channels.count() >= project_root.maximum_concurrent_operations) {
            try protocol.writeChannelClosed(
                allocator,
                attachment.stream.socket.handle,
                runtime_epoch,
                opened.value.channelId,
                "service-busy",
            );
            return true;
        }
        try attachment.channels.put(opened.value.channelId, .{});
        try protocol.writeChannelOpened(
            allocator,
            attachment.stream.socket.handle,
            runtime_epoch,
            opened.value.channelId,
            opened.value.initialCreditBytes,
        );
        return true;
    }
    if (!std.mem.eql(u8, opened.value.serviceId, protocol.terminal_host_service_id)) return false;
    if (attachment.terminal_host_relay != null) {
        try protocol.writeChannelClosed(
            allocator,
            attachment.stream.socket.handle,
            runtime_epoch,
            opened.value.channelId,
            "service-busy",
        );
        return true;
    }
    const worker = terminal_host.Worker.spawn(
        io,
        executable_path,
        attachment.generation,
        opened.value.channelId,
        terminal_host_configuration.executable_path,
        terminal_host_configuration.runtime_directory,
        terminal_host_configuration.endpoint_id,
    ) catch {
        try protocol.writeChannelClosed(
            allocator,
            attachment.stream.socket.handle,
            runtime_epoch,
            opened.value.channelId,
            "service-unavailable",
        );
        return true;
    };
    attachment.terminal_host_relay = .{
        .desktop_credit = opened.value.initialCreditBytes,
        .worker = worker,
    };
    return true;
}

fn dispatchChannelData(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    executable_path: []const u8,
    home_directory: []const u8,
    payload: []const u8,
) !bool {
    const data = protocol.decodeChannelData(payload) catch return false;
    if (attachment.terminal_host_relay) |*relay| {
        if (relay.worker.channel_id == data.channel_id) {
            if (relay.phase != .open or
                data.data.len == 0 or
                data.data.len > protocol.terminal_host_maximum_chunk_bytes or
                data.data.len > relay.host_input_credit)
            {
                return false;
            }
            appendTerminalHostInput(relay, data.data) catch return false;
            relay.host_input_credit -= @intCast(data.data.len);
            return true;
        }
    }
    const channel = attachment.channels.getPtr(data.channel_id) orelse return false;
    if (channel.phase != .opened or data.data.len > channel.request_credit) return false;
    channel.request_credit = 0;

    var request = project_root.decodeRequest(allocator, data.data) catch {
        try publishResult(
            allocator,
            attachment,
            data.channel_id,
            .{ .failed = .invalid_path },
        );
        return true;
    };
    defer request.deinit();

    const worker_slot = firstAvailableWorkerSlot(workers) orelse {
        try publishResult(allocator, attachment, data.channel_id, .{ .failed = .busy });
        return true;
    };
    workers[worker_slot] = project_root.Worker.spawn(
        io,
        executable_path,
        attachment.generation,
        data.channel_id,
        project_root.requestKind(request.value),
        request.value.path orelse home_directory,
    ) catch {
        try publishResult(
            allocator,
            attachment,
            data.channel_id,
            .{ .failed = .unavailable },
        );
        return true;
    };
    channel.phase = .running;
    return true;
}

fn dispatchChannelCredit(
    attachment: *Attachment,
    payload: []const u8,
) !bool {
    const credit = protocol.decodeChannelCredit(payload) catch return false;
    const relay = if (attachment.terminal_host_relay) |*active| active else return false;
    if (relay.worker.channel_id != credit.channel_id or relay.phase != .open or
        credit.bytes > protocol.initial_channel_credit_bytes - relay.desktop_credit)
    {
        return false;
    }
    relay.desktop_credit += credit.bytes;
    return true;
}

fn dispatchChannelClose(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    runtime_epoch: []const u8,
    payload: []const u8,
) !bool {
    var closed = protocol.decodeChannelClose(allocator, payload) catch return false;
    defer closed.deinit();
    if (!std.mem.eql(u8, closed.value.runtimeEpoch, runtime_epoch)) return false;
    if (attachment.terminal_host_relay) |*relay| {
        if (relay.worker.channel_id == closed.value.channelId) {
            try protocol.writeChannelClosed(
                allocator,
                attachment.stream.socket.handle,
                runtime_epoch,
                closed.value.channelId,
                closed.value.reason,
            );
            relay.terminate(io);
            attachment.terminal_host_relay = null;
            return true;
        }
    }
    const channel = attachment.channels.get(closed.value.channelId) orelse return false;
    const cancelled = std.mem.eql(u8, closed.value.reason, "cancelled");
    const completed = std.mem.eql(u8, closed.value.reason, "completed");
    const protocol_failure = std.mem.eql(u8, closed.value.reason, "protocol-failure");
    if (protocol_failure or
        (cancelled and channel.phase != .result_published) or
        (completed and channel.phase == .result_published))
    {
        if (cancelled or protocol_failure) {
            terminateWorker(io, workers, attachment.generation, closed.value.channelId);
        }
        try protocol.writeChannelClosed(
            allocator,
            attachment.stream.socket.handle,
            runtime_epoch,
            closed.value.channelId,
            closed.value.reason,
        );
        _ = attachment.channels.remove(closed.value.channelId);
        return true;
    }
    return false;
}

fn appendTerminalHostInput(relay: *TerminalHostRelay, bytes: []const u8) !void {
    if (relay.host_input.len - relay.host_input_length < bytes.len) {
        const queued_length = relay.host_input_length - relay.host_input_offset;
        std.mem.copyForwards(
            u8,
            relay.host_input[0..queued_length],
            relay.host_input[relay.host_input_offset..relay.host_input_length],
        );
        relay.host_input_offset = 0;
        relay.host_input_length = queued_length;
    }
    if (relay.host_input.len - relay.host_input_length < bytes.len) {
        return error.TerminalHostInputOverflow;
    }
    @memcpy(relay.host_input[relay.host_input_length..][0..bytes.len], bytes);
    relay.host_input_length += bytes.len;
}

fn processTerminalHostOutputEvents(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    runtime_epoch: []const u8,
    events: i16,
) !void {
    if (events & std.posix.POLL.NVAL != 0) {
        try closeTerminalHostRelay(io, allocator, attachment, runtime_epoch, "service-unavailable");
        return;
    }
    if (events & std.posix.POLL.IN != 0) {
        const relay = if (attachment.terminal_host_relay) |*active| active else return;
        const was_starting = relay.phase == .starting;
        const maximum_read_length: usize = if (was_starting)
            @min(protocol.terminal_host_maximum_chunk_bytes, relay.desktop_credit + 1)
        else
            @min(protocol.terminal_host_maximum_chunk_bytes, relay.desktop_credit);
        if (maximum_read_length > 0) {
            var buffer: [protocol.terminal_host_maximum_chunk_bytes]u8 = undefined;
            const result = relay.worker.readAvailable(buffer[0..maximum_read_length]) catch {
                try closeTerminalHostRelay(
                    io,
                    allocator,
                    attachment,
                    runtime_epoch,
                    "service-unavailable",
                );
                return;
            };
            switch (result) {
                .pending => {},
                .ready => try openTerminalHostRelay(allocator, attachment, runtime_epoch),
                .data => |length| {
                    if (was_starting) try openTerminalHostRelay(allocator, attachment, runtime_epoch);
                    const open_relay = if (attachment.terminal_host_relay) |*active| active else return error.TerminalHostRelayUnavailable;
                    if (open_relay.phase != .open or length == 0 or
                        length > open_relay.desktop_credit)
                    {
                        return error.TerminalHostRelayCreditInvalid;
                    }
                    try protocol.writeChannelData(
                        allocator,
                        attachment.stream.socket.handle,
                        open_relay.worker.channel_id,
                        buffer[0..length],
                    );
                    open_relay.desktop_credit -= @intCast(length);
                },
                .unavailable, .closed => {
                    try closeTerminalHostRelay(
                        io,
                        allocator,
                        attachment,
                        runtime_epoch,
                        "service-unavailable",
                    );
                    return;
                },
            }
        }
    }
    if (attachment.terminal_host_relay != null and
        events & (std.posix.POLL.ERR | std.posix.POLL.HUP) != 0)
    {
        try closeTerminalHostRelay(io, allocator, attachment, runtime_epoch, "service-unavailable");
    }
}

fn processTerminalHostInputEvents(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    runtime_epoch: []const u8,
    events: i16,
) !void {
    if (events & (std.posix.POLL.ERR | std.posix.POLL.HUP | std.posix.POLL.NVAL) != 0) {
        try closeTerminalHostRelay(io, allocator, attachment, runtime_epoch, "service-unavailable");
        return;
    }
    if (events & std.posix.POLL.OUT == 0) return;
    const relay = if (attachment.terminal_host_relay) |*active| active else return;
    const written = relay.worker.writeAvailable(
        relay.host_input[relay.host_input_offset..relay.host_input_length],
    ) catch {
        try closeTerminalHostRelay(
            io,
            allocator,
            attachment,
            runtime_epoch,
            "service-unavailable",
        );
        return;
    };
    if (written == 0) return;
    relay.host_input_offset += written;
    relay.host_input_credit += @intCast(written);
    if (relay.host_input_offset == relay.host_input_length) {
        relay.host_input_offset = 0;
        relay.host_input_length = 0;
    }
    try protocol.writeChannelCredit(
        attachment.stream.socket.handle,
        relay.worker.channel_id,
        @intCast(written),
    );
}

fn openTerminalHostRelay(
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    runtime_epoch: []const u8,
) !void {
    const relay = if (attachment.terminal_host_relay) |*active| active else return error.TerminalHostRelayUnavailable;
    if (relay.phase != .starting) return error.TerminalHostRelayStateInvalid;
    relay.phase = .open;
    try protocol.writeChannelOpened(
        allocator,
        attachment.stream.socket.handle,
        runtime_epoch,
        relay.worker.channel_id,
        protocol.initial_channel_credit_bytes,
    );
}

fn closeTerminalHostRelay(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    runtime_epoch: []const u8,
    reason: []const u8,
) !void {
    const relay = if (attachment.terminal_host_relay) |*active| active else return;
    const channel_id = relay.worker.channel_id;
    try protocol.writeChannelClosed(
        allocator,
        attachment.stream.socket.handle,
        runtime_epoch,
        channel_id,
        reason,
    );
    relay.terminate(io);
    attachment.terminal_host_relay = null;
}

fn processWorkerEvents(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    slot_index: usize,
    events: i16,
) !bool {
    const worker = &workers[slot_index].?;
    if (events & std.posix.POLL.NVAL != 0) {
        return completeUnavailableWorker(io, allocator, attachment, workers, slot_index);
    }
    var finished = false;
    if (events & std.posix.POLL.IN != 0) {
        finished = worker.readAvailable() catch
            return completeUnavailableWorker(io, allocator, attachment, workers, slot_index);
    }
    if (!finished and events & (std.posix.POLL.ERR | std.posix.POLL.HUP) != 0) {
        worker.drainAfterHangup() catch
            return completeUnavailableWorker(io, allocator, attachment, workers, slot_index);
        finished = true;
    }
    if (!finished) return false;

    const outcome = worker.finish(io) catch
        return completeUnavailableWorker(io, allocator, attachment, workers, slot_index);
    defer workers[slot_index] = null;
    if (worker.attachment_generation == attachment.generation) {
        const channel = attachment.channels.get(worker.channel_id);
        if (channel != null and channel.?.phase == .running) {
            try publishResult(allocator, attachment, worker.channel_id, outcome);
        }
    }
    return true;
}

fn completeUnavailableWorker(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    slot_index: usize,
) !bool {
    const generation = workers[slot_index].?.attachment_generation;
    const channel_id = workers[slot_index].?.channel_id;
    workers[slot_index].?.terminate(io);
    workers[slot_index] = null;
    if (generation == attachment.generation) {
        const channel = attachment.channels.get(channel_id);
        if (channel != null and channel.?.phase == .running) {
            try publishResult(
                allocator,
                attachment,
                channel_id,
                .{ .failed = .unavailable },
            );
        }
    }
    return true;
}

fn processWorkerTimeouts(
    io: std.Io,
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
) !void {
    const now: std.Io.Timestamp = .now(io, .awake);
    for (workers) |*worker_slot| {
        if (worker_slot.*) |*worker| {
            if (worker.started_at.durationTo(now).nanoseconds <= project_root.operation_timeout_ns) {
                continue;
            }
            const generation = worker.attachment_generation;
            const channel_id = worker.channel_id;
            worker.terminate(io);
            worker_slot.* = null;
            if (generation == attachment.generation) {
                const channel = attachment.channels.get(channel_id);
                if (channel != null and channel.?.phase == .running) {
                    try publishResult(
                        allocator,
                        attachment,
                        channel_id,
                        .{ .failed = .timed_out },
                    );
                }
            }
        }
    }
}

fn publishResult(
    allocator: std.mem.Allocator,
    attachment: *Attachment,
    channel_id: u32,
    outcome: project_root.Outcome,
) !void {
    const channel = attachment.channels.getPtr(channel_id) orelse return error.ChannelUnavailable;
    if (channel.phase == .result_published or
        channel.result_credit < protocol.project_root_maximum_message_bytes)
    {
        return error.ChannelCreditUnavailable;
    }
    try project_root.writeResult(
        allocator,
        attachment.stream.socket.handle,
        channel_id,
        outcome,
    );
    channel.result_credit = 0;
    channel.phase = .result_published;
}

fn firstAvailableWorkerSlot(
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
) ?usize {
    for (workers, 0..) |worker, index| if (worker == null) return index;
    return null;
}

fn terminateWorker(
    io: std.Io,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    attachment_generation: u64,
    channel_id: u32,
) void {
    for (workers) |*worker_slot| {
        if (worker_slot.*) |*worker| {
            if (worker.attachment_generation == attachment_generation and
                worker.channel_id == channel_id)
            {
                worker.terminate(io);
                worker_slot.* = null;
                return;
            }
        }
    }
}

fn terminateGenerationWorkers(
    io: std.Io,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
    attachment_generation: u64,
) void {
    for (workers) |*worker_slot| {
        if (worker_slot.*) |*worker| {
            if (worker.attachment_generation == attachment_generation) {
                worker.terminate(io);
                worker_slot.* = null;
            }
        }
    }
}

fn terminateAllWorkers(
    io: std.Io,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
) void {
    for (workers) |*worker_slot| {
        if (worker_slot.*) |*worker| worker.terminate(io);
        worker_slot.* = null;
    }
}

fn retireAttachment(
    io: std.Io,
    attachment: *?Attachment,
    workers: *[project_root.maximum_concurrent_operations]?project_root.Worker,
) void {
    if (attachment.*) |*active| {
        terminateGenerationWorkers(io, workers, active.generation);
        active.close(io);
    }
    attachment.* = null;
}

pub fn endpointName(
    allocator: std.mem.Allocator,
    runtime_instance_id: []const u8,
) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}.s", .{runtime_instance_id});
}

fn unpublishedEndpointName(
    allocator: std.mem.Allocator,
    runtime_instance_id: []const u8,
) ![]u8 {
    return std.fmt.allocPrint(allocator, "{s}.p", .{runtime_instance_id});
}
