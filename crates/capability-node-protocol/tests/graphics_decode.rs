use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, MAXIMUM_GRAPHICS_PNG_BYTES, OperationId, TerminalGraphicsDecodeRequest,
    WorkerGeneration, WorkerId, decode_message, encode_message, read_message,
};
use std::{io::Cursor, num::NonZeroU64};

fn request() -> TerminalGraphicsDecodeRequest {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(1u32.to_be_bytes());
    png.extend(1u32.to_be_bytes());
    png.extend([0; 9]);
    TerminalGraphicsDecodeRequest {
        request_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        attachment_id: OperationId::from_bytes([3; 16]).unwrap(),
        width: 1,
        height: 1,
        png,
    }
}
#[test]
fn graphics_decode_uses_a_bounded_attachment_request_and_rejects_unread_large_bodies() {
    let value = CapabilityNodeMessage::TerminalGraphicsDecodeRequest(request());
    let bytes = encode_message(&value).unwrap();
    assert_eq!(&bytes[..8], b"OBNG\0\x01\0\0");
    assert_eq!(decode_message(&bytes).unwrap(), value);
    assert_eq!(read_message(&mut Cursor::new(&bytes)).unwrap(), value);
    let mut oversized = b"OBNG\0\x01\0\0".to_vec();
    oversized.extend(
        u32::try_from(64 + MAXIMUM_GRAPHICS_PNG_BYTES + 1)
            .unwrap()
            .to_be_bytes(),
    );
    let mut reader = Cursor::new(oversized);
    assert!(read_message(&mut reader).is_err());
    assert_eq!(reader.position(), 12);
    for (offset, value) in [(6, 1), (12, 0), (44, 0), (76, 0)] {
        let mut broken = bytes.clone();
        if offset == 12 || offset == 44 {
            broken[offset..offset + 16].fill(value);
        } else if offset == 76 {
            broken[offset..offset + 8].fill(value);
        } else {
            broken[offset] = value;
        }
        assert!(decode_message(&broken).is_err());
    }
    let mut zero = request();
    zero.width = u32::MAX;
    zero.height = u32::MAX;
    assert!(zero.validate().is_err());
}
