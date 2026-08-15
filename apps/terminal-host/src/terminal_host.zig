const cli = @import("terminal-host-cli");
const service = @import("terminal-host-service");

pub const host_service = service.service;
pub const host_version = host_service.host_version;
pub const launcher_arguments = cli.launcher_arguments;
pub const local_endpoint = cli.local_endpoint;
pub const local_ipc = @import("terminal-host-transport");
