//! The development protocol fingerprint that `schemas/connection/README.md` defines, computed by
//! `build.zig` exactly as `crates/connection-handshake/build/fingerprint.rs` computes it for the
//! Rust binding, so a Worker and a Host built from one checkout agree. It reads only the
//! registry's listed directories and never follows a symbolic link; a violation stops the build.

const std = @import("std");

const registry_path = "schemas/connection/v1/connections.json";
const domain = "orchbee-protocol-fingerprint-v1\n";
const input_extensions = [_][]const u8{ ".rs", ".zig", ".json" };
const excluded_components = [_][]const u8{ "tests", "__tests__" };
const excluded_file = "tests.rs";
const maximum_registry_bytes = 1 << 20;

/// The lowercase hexadecimal fingerprint of `connection`'s registry `fingerprintInputs`, with
/// paths relative to the repository root two directories above this package.
pub fn forConnection(builder: *std.Build, connection: []const u8) [64]u8 {
    const io = builder.graph.io;
    var repository = std.Io.Dir.cwd().openDir(io, builder.pathFromRoot("../.."), .{}) catch |err|
        fail("open the repository root: {s}", .{@errorName(err)});
    defer repository.close(io);
    const source = repository.readFileAlloc(io, registry_path, builder.allocator, .limited(maximum_registry_bytes)) catch |err|
        fail("read {s}: {s}", .{ registry_path, @errorName(err) });
    const registry = std.json.parseFromSliceLeaky(std.json.Value, builder.allocator, source, .{}) catch |err|
        fail("parse {s}: {s}", .{ registry_path, @errorName(err) });
    const inputs = inputs: {
        const connections = member(registry, "negotiatedConnections") orelse break :inputs null;
        const entry = member(connections, connection) orelse break :inputs null;
        const list = member(entry, "fingerprintInputs") orelse break :inputs null;
        if (list != .array) break :inputs null;
        const directories = builder.allocator.alloc([]const u8, list.array.items.len) catch @panic("OOM");
        for (list.array.items, directories) |item, *directory| {
            if (item != .string) break :inputs null;
            directory.* = item.string;
        }
        break :inputs directories;
    } orelse fail("{s} {s} must list fingerprintInputs strings", .{ registry_path, connection });
    return std.fmt.bytesToHex(compute(builder.allocator, io, repository, inputs), .lower);
}

fn member(value: std.json.Value, name: []const u8) ?std.json.Value {
    if (value != .object) return null;
    return value.object.get(name);
}

/// Hashes the domain, then for each input file in bytewise path order its `/`-joined
/// repository-relative path, a NUL, the lowercase hexadecimal SHA-256 of its raw bytes, and a
/// line feed. `directories` must be strictly ascending plain relative paths, none inside another.
fn compute(
    gpa: std.mem.Allocator,
    io: std.Io,
    repository: std.Io.Dir,
    directories: []const []const u8,
) [32]u8 {
    for (directories, 0..) |directory, index| {
        if (!isPlain(directory)) fail("{s} is not a plain relative path", .{directory});
        const earlier = directories[0..index];
        if (index > 0 and !std.mem.lessThan(u8, earlier[index - 1], directory)) {
            fail("{s} must follow {s} in strictly bytewise order", .{ directory, earlier[index - 1] });
        }
        // `a-b` sorts between `a` and `a/b`, so nesting is checked against every earlier entry.
        for (earlier) |outer| {
            if (directory.len > outer.len and std.mem.startsWith(u8, directory, outer) and directory[outer.len] == '/') {
                fail("{s} is inside {s}", .{ directory, outer });
            }
        }
        const stat = repository.statFile(io, directory, .{ .follow_symlinks = false }) catch |err|
            fail("{s}: {s}", .{ directory, @errorName(err) });
        if (stat.kind != .directory) fail("{s} is not a directory", .{directory});
    }

    var files: std.ArrayList([]const u8) = .empty;
    for (directories) |directory| collect(gpa, io, repository, directory, &files);
    std.mem.sort([]const u8, files.items, {}, lessThan);

    var fingerprint = std.crypto.hash.sha2.Sha256.init(.{});
    fingerprint.update(domain);
    for (files.items) |path| {
        const bytes = repository.readFileAlloc(io, path, gpa, .unlimited) catch |err|
            fail("read {s}: {s}", .{ path, @errorName(err) });
        defer gpa.free(bytes);
        var digest: [32]u8 = undefined;
        std.crypto.hash.sha2.Sha256.hash(bytes, &digest, .{});
        fingerprint.update(path);
        fingerprint.update(&.{0});
        fingerprint.update(&std.fmt.bytesToHex(digest, .lower));
        fingerprint.update("\n");
    }
    return fingerprint.finalResult();
}

fn collect(
    gpa: std.mem.Allocator,
    io: std.Io,
    repository: std.Io.Dir,
    directory: []const u8,
    files: *std.ArrayList([]const u8),
) void {
    var handle = repository.openDir(io, directory, .{ .iterate = true, .follow_symlinks = false }) catch |err|
        fail("open {s}: {s}", .{ directory, @errorName(err) });
    defer handle.close(io);
    var entries = handle.iterate();
    while (entries.next(io) catch |err| fail("list {s}: {s}", .{ directory, @errorName(err) })) |entry| {
        if (!std.unicode.utf8ValidateSlice(entry.name)) fail("{s}/{s} is not Unicode", .{ directory, entry.name });
        const path = std.fmt.allocPrint(gpa, "{s}/{s}", .{ directory, entry.name }) catch @panic("OOM");
        const kind = switch (entry.kind) {
            .unknown => (repository.statFile(io, path, .{ .follow_symlinks = false }) catch |err|
                fail("{s}: {s}", .{ path, @errorName(err) })).kind,
            else => |kind| kind,
        };
        const excluded = isExcluded(path);
        switch (kind) {
            .sym_link => fail("{s} is a symbolic link", .{path}),
            .directory => if (!excluded) collect(gpa, io, repository, path, files),
            .file => {
                const input = for (input_extensions) |extension| {
                    if (std.mem.endsWith(u8, entry.name, extension)) break true;
                } else false;
                if (input and !excluded and !std.mem.eql(u8, entry.name, excluded_file)) {
                    files.append(gpa, path) catch @panic("OOM");
                }
            },
            else => fail("{s} is neither a file nor a directory", .{path}),
        }
    }
}

fn isExcluded(path: []const u8) bool {
    var components = std.mem.splitScalar(u8, path, '/');
    while (components.next()) |component| {
        for (excluded_components) |name| if (std.mem.eql(u8, component, name)) return true;
    }
    return false;
}

fn isPlain(directory: []const u8) bool {
    if (directory.len == 0 or std.mem.indexOfScalar(u8, directory, '\\') != null) return false;
    var components = std.mem.splitScalar(u8, directory, '/');
    while (components.next()) |component| {
        if (component.len == 0 or std.mem.eql(u8, component, ".") or std.mem.eql(u8, component, "..")) {
            return false;
        }
    }
    return true;
}

fn lessThan(_: void, left: []const u8, right: []const u8) bool {
    return std.mem.lessThan(u8, left, right);
}

fn fail(comptime format: []const u8, arguments: anytype) noreturn {
    std.debug.panic("development fingerprint: " ++ format, arguments);
}
