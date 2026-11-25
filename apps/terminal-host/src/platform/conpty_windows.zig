const std = @import("std");
const builtin = @import("builtin");
const windows = std.os.windows;
const kernel = @import("windows_api.zig");
const pins = @import("windows-conpty-options");

const Create = *const fn (windows.COORD, windows.HANDLE, windows.HANDLE, windows.DWORD, *kernel.HPCON) callconv(.winapi) kernel.HRESULT;
const Resize = *const fn (kernel.HPCON, windows.COORD) callconv(.winapi) kernel.HRESULT;
const Close = *const fn (kernel.HPCON) callconv(.winapi) void;
const Clear = *const fn (kernel.HPCON, windows.BOOL) callconv(.winapi) kernel.HRESULT;
/// `ConptyPackPseudoConsole(process, reference, signal, out)`, from the pinned conpty's winconpty.h.
const Pack = *const fn (windows.HANDLE, windows.HANDLE, windows.HANDLE, *kernel.HPCON) callconv(.winapi) kernel.HRESULT;
const AttributeTag = extern struct { attributes: windows.DWORD, tag: windows.DWORD };
const max_file_bytes = 64 * 1024 * 1024;

extern "kernel32" fn CreateFileW([*:0]const u16, windows.DWORD, windows.DWORD, ?*anyopaque, windows.DWORD, windows.DWORD, ?windows.HANDLE) callconv(.winapi) windows.HANDLE;
extern "kernel32" fn GetFileAttributesW([*:0]const u16) callconv(.winapi) windows.DWORD;
extern "kernel32" fn GetFileInformationByHandleEx(windows.HANDLE, c_int, *anyopaque, windows.DWORD) callconv(.winapi) windows.BOOL;
extern "kernel32" fn GetFileType(windows.HANDLE) callconv(.winapi) windows.DWORD;
extern "kernel32" fn GetFileSizeEx(windows.HANDLE, *i64) callconv(.winapi) windows.BOOL;
extern "kernel32" fn LoadLibraryExW([*:0]const u16, ?windows.HANDLE, windows.DWORD) callconv(.winapi) ?windows.HMODULE;
extern "kernel32" fn GetProcAddress(windows.HMODULE, [*:0]const u8) callconv(.winapi) ?*const anyopaque;
extern "kernel32" fn FreeLibrary(windows.HMODULE) callconv(.winapi) windows.BOOL;

/// One API instance owns its module and immutable bundle files through HPCON
/// retirement. An invalid present bundle never falls back to system ConHost.
pub const Api = struct {
    create: Create = kernel.CreatePseudoConsole,
    resize: Resize = kernel.ResizePseudoConsole,
    close: Close = kernel.ClosePseudoConsole,
    /// The bundled ConPTY's buffer clear; the system ConHost has none.
    clear: ?Clear = null,
    /// The bundled ConPTY's pack, which wraps a held console's process, reference, and signal
    /// handles in a new pseudoconsole without sending anything to OpenConsole; null without it.
    pack: ?Pack = null,
    module: ?windows.HMODULE = null,
    dll_file: ?windows.HANDLE = null,
    console_file: ?windows.HANDLE = null,

    pub fn init() !Api {
        if (builtin.cpu.arch != .x86_64) return .{};
        var path: [32768]u16 = undefined;
        const length = kernel.GetModuleFileNameW(null, &path, path.len);
        if (length == 0 or length >= path.len) return error.ConptyExecutablePathUnavailable;
        const parent = std.mem.lastIndexOfScalar(u16, path[0..length], '\\') orelse
            return error.ConptyExecutablePathUnavailable;
        const suffix = std.unicode.utf8ToUtf16LeStringLiteral("\\conpty");
        if (parent + suffix.len >= path.len) return error.ConptyExecutablePathUnavailable;
        @memcpy(path[parent..][0..suffix.len], suffix);
        path[parent + suffix.len] = 0;
        return loadAt(path[0 .. parent + suffix.len :0]);
    }

    pub fn deinit(self: *Api) void {
        if (self.module) |module| _ = FreeLibrary(module);
        if (self.console_file) |file| _ = windows.CloseHandle(file);
        if (self.dll_file) |file| _ = windows.CloseHandle(file);
        self.* = undefined;
    }

    fn loadAt(directory: [:0]const u16) !Api {
        const attributes = GetFileAttributesW(directory);
        if (attributes == 0xffff_ffff) {
            return switch (windows.GetLastError()) {
                .FILE_NOT_FOUND, .PATH_NOT_FOUND => .{},
                else => error.ConptyBundleUnavailable,
            };
        }
        if (attributes & 0x400 != 0 or attributes & 0x10 == 0) return error.ConptyBundleUnsafe;
        var dll_path: [32768]u16 = undefined;
        var console_path: [32768]u16 = undefined;
        const dll = try childPath(&dll_path, directory, "\\conpty.dll");
        const console = try childPath(&console_path, directory, "\\OpenConsole.exe");
        var result: Api = .{};
        errdefer result.deinit();
        result.dll_file = try verifiedFile(dll, pins.dll_sha256);
        result.console_file = try verifiedFile(console, pins.console_sha256);
        // DLL_LOAD_DIR | SYSTEM32: never search PATH or the working directory.
        const module = LoadLibraryExW(dll, null, 0x0000_0100 | 0x0000_0800) orelse
            return error.ConptyLibraryUnavailable;
        result.module = module;
        result.create = @ptrCast(GetProcAddress(module, "CreatePseudoConsole") orelse return error.ConptyExportUnavailable);
        result.resize = @ptrCast(GetProcAddress(module, "ResizePseudoConsole") orelse return error.ConptyExportUnavailable);
        result.close = @ptrCast(GetProcAddress(module, "ClosePseudoConsole") orelse return error.ConptyExportUnavailable);
        result.clear = @ptrCast(GetProcAddress(module, "ConptyClearPseudoConsole") orelse return error.ConptyExportUnavailable);
        // Optional: a bundle without it only moves no terminal.
        result.pack = @ptrCast(GetProcAddress(module, "ConptyPackPseudoConsole"));
        return result;
    }
};

const Found = enum(u8) { unknown, system, bundle, bundle_with_pack };

var found_state: std.atomic.Value(Found) = .init(.unknown);

/// What this Host's consoles run on. The first answer loads the bundle once; a bundle that fails
/// to load answers `unknown` and is tried again.
fn found() Found {
    const known = found_state.load(.acquire);
    if (known != .unknown) return known;
    var api = Api.init() catch return .unknown;
    defer api.deinit();
    const answer: Found = if (api.module == null) .system else if (api.pack == null) .bundle else .bundle_with_pack;
    found_state.store(answer, .release);
    return answer;
}

/// Whether the bundled ConPTY exports its pack, so this Host can take a moved terminal.
pub fn packs() bool {
    return found() == .bundle_with_pack;
}

/// Whether this Host's consoles run on its bundled ConPTY. A console started at the cursor of a
/// screen that already shows output stays in step with that screen across a resize only there:
/// the system ConHost reflows its own blank buffer and then writes where the screen has moved on.
pub fn bundled() bool {
    return switch (found()) {
        .bundle, .bundle_with_pack => true,
        .unknown, .system => false,
    };
}

fn childPath(buffer: []u16, parent: []const u16, comptime suffix: []const u8) ![:0]const u16 {
    const tail = std.unicode.utf8ToUtf16LeStringLiteral(suffix);
    const length = parent.len + tail.len;
    if (length >= buffer.len) return error.ConptyExecutablePathUnavailable;
    @memcpy(buffer[0..parent.len], parent);
    @memcpy(buffer[parent.len..length], tail);
    buffer[length] = 0;
    return buffer[0..length :0];
}

fn verifiedFile(path: [:0]const u16, expected: []const u8) !windows.HANDLE {
    // GENERIC_READ, SHARE_READ, OPEN_EXISTING, OPEN_REPARSE_POINT. Retaining
    // this handle prevents replacement or writes between hashing and loading.
    const file = CreateFileW(path, 0x8000_0000, 1, null, 3, 0x0020_0000, null);
    if (file == windows.INVALID_HANDLE_VALUE) return error.ConptyBundleUnavailable;
    errdefer _ = windows.CloseHandle(file);
    var metadata: AttributeTag = undefined;
    if (GetFileType(file) != 1 or
        !GetFileInformationByHandleEx(file, 9, &metadata, @sizeOf(AttributeTag)).toBool() or
        metadata.attributes & (0x400 | 0x10) != 0) return error.ConptyBundleUnsafe;
    var length: i64 = undefined;
    if (!GetFileSizeEx(file, &length).toBool() or length <= 0 or length > max_file_bytes)
        return error.ConptyBundleUnsafe;
    var hash = std.crypto.hash.sha2.Sha256.init(.{});
    var buffer: [64 * 1024]u8 = undefined;
    var total: usize = 0;
    while (true) {
        var count: windows.DWORD = 0;
        if (!kernel.ReadFile(file, &buffer, buffer.len, &count, null).toBool()) return error.ConptyBundleUnavailable;
        if (count == 0) break;
        total += count;
        if (total > max_file_bytes) return error.ConptyBundleUnsafe;
        hash.update(buffer[0..count]);
    }
    var actual: [32]u8 = undefined;
    hash.final(&actual);
    var declared: [32]u8 = undefined;
    const decoded = std.fmt.hexToBytes(&declared, expected) catch return error.ConptyBundleIntegrity;
    if (decoded.len != actual.len or total != length or !std.mem.eql(u8, &actual, decoded))
        return error.ConptyBundleIntegrity;
    return file;
}

pub const testing = if (builtin.is_test) struct {
    pub const loadAt = Api.loadAt;
    pub const verifyFile = verifiedFile;

    pub fn initializedFromBundle() !bool {
        var api = try Api.init();
        defer api.deinit();
        return api.module != null;
    }
} else struct {};
