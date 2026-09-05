use orchbee_client_server_protocol::{
    MAXIMUM_VIEWER_RECORD_BYTES, ReadMessageError, RequestId, ServerMessage, TerminalViewerOutcome,
    TerminalViewerResult, decode_client_frame, decode_server_frame, encode_server_frame,
    read_client_message, read_server_message,
};
use std::io::{Cursor, Read};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn result(outcome: TerminalViewerOutcome) -> Result<ServerMessage, Box<dyn std::error::Error>> {
    Ok(ServerMessage::TerminalViewerResult(
        TerminalViewerResult::new(
            RequestId::parse("0123456789abcdef0123456789abcdef")?,
            outcome,
        ),
    ))
}

#[test]
fn large_binary_record_and_following_json_keep_stream_alignment() -> TestResult {
    let record = result(TerminalViewerOutcome::Record(vec![0xa5; 100_000]))?;
    let empty = result(TerminalViewerOutcome::Empty {})?;
    let frame = encode_server_frame(&record)?;
    assert_eq!(&frame[..8], b"OBSR\0\x01\0\0");
    assert_eq!(decode_server_frame(&frame)?, record);
    assert!(decode_client_frame(&frame).is_err());
    let mut wrong_direction = Cursor::new(&frame);
    assert!(read_client_message(&mut wrong_direction).is_err());
    assert_eq!(wrong_direction.position(), 8);
    let mut stream = frame;
    stream.extend(encode_server_frame(&empty)?);
    let mut partial = ShortReads(Cursor::new(stream));
    assert_eq!(read_server_message(&mut partial)?, record);
    assert_eq!(read_server_message(&mut partial)?, empty);
    Ok(())
}

#[test]
fn binary_bounds_and_header_errors_fail_before_reading_the_body() -> TestResult {
    for length in [0, 79, 17 + MAXIMUM_VIEWER_RECORD_BYTES, u32::MAX as usize] {
        let mut header = b"OBSR\0\x01\0\0".to_vec();
        header.extend(u32::try_from(length)?.to_be_bytes());
        assert!(matches!(
            read_server_message(&mut Cursor::new(header)),
            Err(ReadMessageError::Protocol(_))
        ));
    }
    let valid = encode_server_frame(&result(TerminalViewerOutcome::Record(vec![0; 64]))?)?;
    for offset in [4, 5, 6, 7] {
        let mut bad = valid.clone();
        bad[offset] = 9;
        assert!(decode_server_frame(&bad).is_err());
    }
    let mut zero = valid.clone();
    zero[12..28].fill(0);
    assert!(decode_server_frame(&zero).is_err());
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(decode_server_frame(&trailing).is_err());
    assert!(decode_server_frame(&valid[..valid.len() - 1]).is_err());
    assert!(encode_server_frame(&result(TerminalViewerOutcome::Record(vec![0; 63]))?).is_err());
    Ok(())
}

struct ShortReads(Cursor<Vec<u8>>);
impl Read for ShortReads {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let length = buffer.len().min(3);
        self.0.read(&mut buffer[..length])
    }
}
