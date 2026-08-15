comptime {
    _ = @import("connection_request_test.zig");
    _ = @import("event_delivery_test.zig");
    _ = @import("input_request_test.zig");
    _ = @import("interaction_delivery_test.zig");
    _ = @import("interaction_replay_test.zig");
    _ = @import("interaction_transaction_test.zig");
    _ = @import("lifecycle_request_test.zig");
    _ = @import("outgoing_test.zig");
    _ = @import("projection_test.zig");
    _ = @import("response_test.zig");
    _ = @import("runtime_aggregate_attachment_test.zig");
    _ = @import("runtime_aggregate_cleanup_test.zig");
    _ = @import("runtime_aggregate_lifecycle_test.zig");
    _ = @import("terminal_integration_test.zig");
}
