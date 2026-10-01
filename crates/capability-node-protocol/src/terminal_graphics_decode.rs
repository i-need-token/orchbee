//! One bounded read-only decode on an already admitted viewer attachment.
use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

pub const MAXIMUM_GRAPHICS_PNG_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalGraphicsDecodeRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub attachment_id: OperationId,
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}
impl TerminalGraphicsDecodeRequest {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.width == 0 || self.height == 0 || self.width > 4096 || self.height > 4096 {
            return Err(ProtocolError::InvalidPayload);
        }
        let pixel_bytes = u64::from(self.width) * u64::from(self.height) * 4;
        if self.width == 0
            || self.height == 0
            || self.width > 4096
            || self.height > 4096
            || pixel_bytes > 32 * 1024 * 1024
            || self.png.len() < 33
            || self.png.len() > MAXIMUM_GRAPHICS_PNG_BYTES
            || &self.png[..8] != b"\x89PNG\r\n\x1a\n"
            || self.png[8..16] != [0, 0, 0, 13, b'I', b'H', b'D', b'R']
            || self.png[16..20] != self.width.to_be_bytes()
            || self.png[20..24] != self.height.to_be_bytes()
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(())
    }
}
