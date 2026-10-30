//! Unix system notifications return Unsupported; no external notification process is launched.
#[derive(Default)]
pub struct SystemNotifications;

impl SystemNotifications {
    /// Construct the unavailable capability without owning native resources.
    pub fn new() -> Self {
        Self
    }

    /// Consume the unavailable capability; it has no native owner to join.
    pub fn finish(self) {}

    pub fn submit(
        &mut self,
        _message: orchbee_workbench_protocol::Notification,
    ) -> std::io::Result<()> {
        Err(crate::host_capabilities::Unsupported(
            crate::host_capabilities::Capability::SystemNotification,
        )
        .into())
    }
}
