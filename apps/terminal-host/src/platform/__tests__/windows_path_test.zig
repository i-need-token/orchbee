const std = @import("std");
const windows_path = @import("terminal-host-platform").windows_path;

// A security boundary: an admitted path is opened, so only a file on a local drive may pass.
test "only an absolute path on a local drive is admitted" {
    for ([_][]const u8{
        "C:\\Users\\me\\image.png",
        "c:/tmp/tty-graphics-protocol-1",
        "Z:\\a\\b\\..\\c.rgb",
        "D:\\console.log",
        "D:\\com10\\lpt.txt",
    }) |path| {
        if (!windows_path.isLocalDrivePath(path)) {
            std.debug.print("refused {s}\n", .{path});
            return error.TestUnexpectedResult;
        }
    }
    for ([_][]const u8{
        "\\\\host\\share\\image.png",
        "//host/share/image.png",
        "\\/host/share/image.png",
        "\\\\.\\pipe\\name",
        "\\\\?\\C:\\image.png",
        "\\??\\C:\\image.png",
        "\\image.png",
        "image.png",
        "C:image.png",
        "C:",
        "C:\\",
        "C://",
        "1:\\image.png",
        "C:\\dir\\NUL",
        "C:\\dir\\nul.png",
        "C:\\CON\\image.png",
        "C:\\dir\\aux .txt",
        "C:\\dir\\COM1",
        "C:\\dir\\lpt9.rgb",
        "C:\\dir\\COM\u{00B9}",
        "C:\\dir\\CONOUT$",
        "C:/dir/prn",
        "C:\\image.png:stream",
        "C:\\image\x00.png",
    }) |path| {
        if (windows_path.isLocalDrivePath(path)) {
            std.debug.print("admitted {s}\n", .{path});
            return error.TestUnexpectedResult;
        }
    }
}
