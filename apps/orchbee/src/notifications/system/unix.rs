//! Unix system notifications return Unsupported; no external notification process is launched.
#[derive(Default)]
pub(crate) struct SystemNotifications;

impl SystemNotifications {
    /// Construct the unavailable capability without owning native resources.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Consume the unavailable capability; it has no native owner to join.
    pub(crate) fn finish(self) {}

    pub(crate) fn submit(
        &mut self,
        _message: orchbee_workbench_protocol::Notification,
    ) -> std::io::Result<()> {
        Err(crate::tui_capabilities::Unsupported(
            crate::tui_capabilities::Capability::SystemNotification,
        )
        .into())
    }
}
