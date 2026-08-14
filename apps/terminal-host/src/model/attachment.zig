pub const Capabilities = packed struct {
    input: bool,
    read: bool,
    resize: bool,
};

pub const ProjectionDemand = enum {
    active,
    paused,
};
