//! A process with the Host's root panic handlers. It arms the fatal record in the directory its
//! first argument names, then panics with its second argument as the message, the way a panic
//! formatted from program output would.
const std = @import("std");
const fatal_record = @import("terminal-host-platform").fatal_record;

pub const panic = fatal_record.panic;
pub const debug = struct {
    pub const handleSegfault = fatal_record.handleSegfault;
};

pub fn main(init: std.process.Init) !void {
    const arguments = try init.minimal.args.toSlice(init.arena.allocator());
    if (arguments.len != 3) return error.InvalidArguments;
    fatal_record.arm(arguments[1]);
    fatal_record.markThread(.dispatch);
    std.debug.panic("program output: {s}", .{arguments[2]});
}
