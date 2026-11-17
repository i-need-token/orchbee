//! The Terminal Host's binding of the `OBKP` protocol that `schemas/keeper/README.md` defines:
//! the frame and both directions' messages of generation 1 of the `keeper` connection. It
//! performs no I/O; the Keeper and the Host's Keeper link own transports, deadlines, and slots.

pub const decode = @import("decode.zig");
pub const encode = @import("encode.zig");
pub const frame = @import("frame.zig");
pub const message = @import("message.zig");
