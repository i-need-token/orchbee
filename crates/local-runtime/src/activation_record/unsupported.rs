//! No retained activation effect is implemented on this host.
pub enum RetainedActivationRecord {}
impl RetainedActivationRecord {
    pub fn complete(&mut self, _request_id: [u8; 16]) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}
