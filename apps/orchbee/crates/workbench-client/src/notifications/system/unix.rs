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

/// Ring the terminal bell for one audible notification sound.
pub fn sound(kind: orchbee_workbench_protocol::NotificationSound) {
    use std::io::Write;
    if kind != orchbee_workbench_protocol::NotificationSound::None {
        let mut output = std::io::stdout().lock();
        let _ = output.write_all(b"\x07").and_then(|()| output.flush());
    }
}
