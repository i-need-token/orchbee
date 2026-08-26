const std = @import("std");

pub const replay_environment = "ORCHBEE_TERMINAL_HOST_PROPERTY_REPLAY";
pub const seed_environment = "ORCHBEE_TERMINAL_HOST_PROPERTY_SEED";

/// One-byte codes make a minimized failure directly replayable without a
/// parser whose own ambiguity could hide the original transition sequence.
pub const Command = enum(u8) {
    native_attach_owner = 'A',
    native_attach_observer = 'B',
    native_attach_third = 'S',
    native_attach_conflict = 'C',
    native_claim_owner = '1',
    native_claim_observer = '2',
    native_claim_third = '3',
    native_demand_active = 'D',
    native_demand_paused = 'd',
    native_input = 'I',
    native_focus_owner_on = 'W',
    native_focus_owner_off = 'w',
    native_focus_observer_on = 'X',
    native_focus_observer_off = 'x',
    native_resize_small = 'R',
    native_resize_large = 'r',
    native_config_small = 'G',
    native_config_default = 'g',
    native_clear = 'L',
    native_reset = 'Z',
    native_detach_owner = 'T',
    native_detach_observer = 't',
    native_detach_third = 's',
    disconnect = 'O',
    connect = 'o',
    native_create_replay = 'P',
    poll = 'Q',
    model_create = 'M',
    model_attach_read = 'N',
    model_detach = 'n',
    model_graceful = 'H',
    model_force = 'F',
    model_child_exit = 'E',
    model_pty_drain = 'Y',
    model_failure = 'V',
    model_contain = 'K',
    model_publish = 'U',
    model_dispose = 'J',

    pub fn simpler(self: Command) ?Command {
        return switch (self) {
            .native_attach_observer, .native_attach_third, .native_attach_conflict => .native_attach_owner,
            .native_claim_observer, .native_claim_third => .native_claim_owner,
            .native_demand_active => .native_demand_paused,
            .native_focus_owner_on => .native_focus_owner_off,
            .native_focus_observer_on => .native_focus_observer_off,
            .native_resize_large => .native_resize_small,
            .native_config_default => .native_config_small,
            .native_reset => .native_clear,
            .native_detach_observer, .native_detach_third => .native_detach_owner,
            .model_force => .model_graceful,
            else => null,
        };
    }
};

const choices = [_]Command{
    .native_attach_owner,
    .native_attach_observer,
    .native_attach_third,
    .native_attach_conflict,
    .native_claim_owner,
    .native_claim_observer,
    .native_claim_third,
    .native_demand_active,
    .native_demand_paused,
    .native_input,
    .native_focus_owner_on,
    .native_focus_owner_off,
    .native_focus_observer_on,
    .native_focus_observer_off,
    .native_resize_small,
    .native_resize_large,
    .native_config_small,
    .native_config_default,
    .native_clear,
    .native_reset,
    .native_detach_owner,
    .native_detach_observer,
    .native_detach_third,
    .disconnect,
    .connect,
    .native_create_replay,
    .poll,
    .model_create,
    .model_create,
    .model_attach_read,
    .model_detach,
    .model_graceful,
    .model_force,
    .model_child_exit,
    .model_pty_drain,
    .model_failure,
    .model_contain,
    .model_publish,
    .model_dispose,
};

pub fn generate(
    allocator: std.mem.Allocator,
    seed: u64,
    length: usize,
) std.mem.Allocator.Error![]Command {
    const commands = try allocator.alloc(Command, length);
    if (length == 0) return commands;
    commands[0] = .model_create;
    if (length > 1) commands[1] = .model_attach_read;
    var random = DeterministicRandom.init(seed);
    for (commands[2..]) |*command| {
        command.* = choices[random.index(choices.len)];
    }
    return commands;
}

pub fn encode(
    allocator: std.mem.Allocator,
    commands: []const Command,
) std.mem.Allocator.Error![]u8 {
    const bytes = try allocator.alloc(u8, commands.len);
    for (commands, bytes) |command, *byte| byte.* = @intFromEnum(command);
    return bytes;
}

pub fn decode(
    allocator: std.mem.Allocator,
    bytes: []const u8,
) (error{InvalidReplayCommand} || std.mem.Allocator.Error)![]Command {
    const commands = try allocator.alloc(Command, bytes.len);
    errdefer allocator.free(commands);
    for (bytes, commands) |byte, *command| {
        command.* = std.enums.fromInt(Command, byte) orelse return error.InvalidReplayCommand;
    }
    return commands;
}

/// Delta-debugging removes ranges first, then simplifies individual commands.
/// The caller supplies a fresh-state predicate, so every candidate is a true
/// replay rather than a continuation from the original failed Host.
pub fn shrink(
    allocator: std.mem.Allocator,
    original: []const Command,
    context: anytype,
    comptime still_fails: fn (@TypeOf(context), []const Command) bool,
) std.mem.Allocator.Error![]Command {
    var current = try allocator.dupe(Command, original);
    var partitions: usize = 2;
    while (current.len >= 2) {
        const chunk_size = std.math.divCeil(usize, current.len, partitions) catch unreachable;
        var removed_any = false;
        var start: usize = 0;
        while (start < current.len) : (start += chunk_size) {
            const end = @min(current.len, start + chunk_size);
            const candidate = try withoutRange(allocator, current, start, end);
            if (still_fails(context, candidate)) {
                allocator.free(current);
                current = candidate;
                partitions = @max(@as(usize, 2), partitions -| 1);
                removed_any = true;
                break;
            }
            allocator.free(candidate);
        }
        if (removed_any) continue;
        if (partitions >= current.len) break;
        partitions = @min(current.len, partitions * 2);
    }

    for (current, 0..) |command, index| {
        const simpler = command.simpler() orelse continue;
        const candidate = try allocator.dupe(Command, current);
        candidate[index] = simpler;
        if (still_fails(context, candidate)) {
            allocator.free(current);
            current = candidate;
        } else {
            allocator.free(candidate);
        }
    }
    return current;
}

fn withoutRange(
    allocator: std.mem.Allocator,
    commands: []const Command,
    start: usize,
    end: usize,
) std.mem.Allocator.Error![]Command {
    const candidate = try allocator.alloc(Command, commands.len - (end - start));
    @memcpy(candidate[0..start], commands[0..start]);
    @memcpy(candidate[start..], commands[end..]);
    return candidate;
}

const DeterministicRandom = struct {
    state: u64,

    fn init(seed: u64) DeterministicRandom {
        return .{ .state = if (seed == 0) 0x9e3779b97f4a7c15 else seed };
    }

    fn index(self: *DeterministicRandom, upper_bound: usize) usize {
        var value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        const mixed = value *% 0x2545f4914f6cdd1d;
        return @intCast(mixed % upper_bound);
    }
};
