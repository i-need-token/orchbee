//! How a Windows launch finds its executable: the candidates `CreateProcess` would try, in its
//! order, without letting the Host's own `PATH` leak into a launch that supplied one.

const std = @import("std");

pub fn windowsExecutableCandidate(
    arena: std.mem.Allocator,
    candidate: []const u8,
    executable: []const u8,
) ![:0]u16 {
    // CreateProcess supplies the implicit .exe suffix only for a bare
    // executable name. A qualified name such as `tools\\probe` is an
    // exact path candidate and must not silently become `tools\\probe.exe`.
    const can_append_extension = std.mem.indexOfAny(u8, executable, "\\/") == null;
    const with_extension = if (can_append_extension and
        std.mem.indexOfScalar(u8, executable, '.') == null)
        try std.fmt.allocPrint(arena, "{s}.exe", .{candidate})
    else
        candidate;
    return std.unicode.utf8ToUtf16LeAllocZ(arena, with_extension);
}

pub fn joinWindowsPath(arena: std.mem.Allocator, directory: []const u8, name: []const u8) ![]u8 {
    if (directory.len == 0) return arena.dupe(u8, name);
    // A trailing drive designator keeps drive-relative syntax: `C:` joined with
    // `probe` is `C:probe` (the drive's current directory), never `C:\probe`.
    const separator = if (directory[directory.len - 1] == '\\' or
        directory[directory.len - 1] == '/' or
        directory[directory.len - 1] == ':')
        ""
    else
        "\\";
    return std.fmt.allocPrint(arena, "{s}{s}{s}", .{ directory, separator, name });
}

const WindowsPathKind = enum { empty, drive_relative, ordinary_relative, absolute };

fn windowsPathKind(directory: []const u8) WindowsPathKind {
    if (directory.len == 0) return .empty;
    if (std.fs.path.isAbsoluteWindows(directory)) return .absolute;
    if (directory.len >= 2 and std.ascii.isAlphabetic(directory[0]) and directory[1] == ':')
        return .drive_relative;
    return .ordinary_relative;
}

pub fn windowsSearchCandidates(
    arena: std.mem.Allocator,
    executable: []const u8,
    application_directory: ?[]const u8,
    parent_current_directory: ?[]const u8,
    system_directory: ?[]const u8,
    include_current_directory: bool,
    path: ?[]const u8,
) ![][:0]u16 {
    var candidates: std.ArrayList([:0]u16) = .empty;
    if (application_directory) |directory| {
        try candidates.append(arena, try windowsExecutableCandidate(
            arena,
            try joinWindowsPath(arena, directory, executable),
            executable,
        ));
    }
    if (include_current_directory) {
        if (parent_current_directory) |directory| {
            try candidates.append(arena, try windowsExecutableCandidate(
                arena,
                try joinWindowsPath(arena, directory, executable),
                executable,
            ));
        }
    }
    if (system_directory) |directory| {
        try candidates.append(arena, try windowsExecutableCandidate(
            arena,
            try joinWindowsPath(arena, directory, executable),
            executable,
        ));
        if (std.mem.lastIndexOfAny(u8, directory, "\\/")) |separator| {
            const windows_directory = directory[0..separator];
            try candidates.append(arena, try windowsExecutableCandidate(
                arena,
                try joinWindowsPath(arena, windows_directory, "System"),
                executable,
            ));
            try candidates.append(arena, try windowsExecutableCandidate(
                arena,
                try joinWindowsPath(arena, windows_directory, executable),
                executable,
            ));
        }
    }
    // A missing PATH contributes no PATH candidates. Its absence has no
    // current-directory semantics of its own, so a policy-excluded current
    // directory cannot be reintroduced through a synthesized empty component.
    if (path) |effective_path| {
        var iterator = std.mem.splitScalar(u8, effective_path, ';');
        while (iterator.next()) |directory| {
            const path_kind = windowsPathKind(directory);
            if (path_kind == .empty) {
                // An explicitly empty PATH component means the current
                // directory, matching CreateProcess's PATH search. It is a
                // supplied entry, not the policy-controlled implicit slot.
                if (parent_current_directory) |parent| {
                    const path_candidate = try joinWindowsPath(arena, parent, executable);
                    try candidates.append(arena, try windowsExecutableCandidate(arena, path_candidate, executable));
                }
                continue;
            }
            const base: []const u8 = switch (path_kind) {
                // A drive-relative entry such as `C:tools` is relative to the
                // drive's current directory, never to the parent process
                // current directory, so it must not be prefixed with it.
                .drive_relative, .absolute => directory,
                .ordinary_relative => if (parent_current_directory) |parent|
                    try joinWindowsPath(arena, parent, directory)
                else
                    continue,
                .empty => unreachable,
            };
            const path_candidate = try joinWindowsPath(arena, base, executable);
            try candidates.append(arena, try windowsExecutableCandidate(arena, path_candidate, executable));
        }
    }
    return candidates.toOwnedSlice(arena);
}
