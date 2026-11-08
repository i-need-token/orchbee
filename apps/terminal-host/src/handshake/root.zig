//! The Terminal Host's binding of the `OBHS` connection handshake that
//! `schemas/connection/README.md` defines: the frame, strict payload JSON, both directions'
//! messages, the acceptor's admission decision, and the `worker-host` registry entry. It performs
//! no I/O; the transport owns reading, deadlines, and slots.

pub const admission = @import("admission.zig");
pub const decode = @import("decode.zig");
pub const encode = @import("encode.zig");
pub const frame = @import("frame.zig");
pub const json = @import("json.zig");
pub const message = @import("message.zig");
pub const registry = @import("registry.zig");
