use super::*;

#[test]
fn notice_codec_is_closed_and_bounded_and_never_contains_payload_data() {
    let golden = b"orchbee-checkpoint-restore-v1:abababababababababababababababab:E";
    assert_eq!(encode([0xab; 16], Some(Notice::Empty)).unwrap(), golden);
    for notice in [None, Some(Notice::Recovered), Some(Notice::Empty)] {
        let bytes = encode([0xab; 16], notice).unwrap();
        assert_eq!(decode(&bytes).unwrap(), ([0xab; 16], notice));
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
        let mut trailing = bytes.clone();
        trailing.push(b'\n');
        assert!(decode(&trailing).is_err());
        let mut unknown = bytes;
        *unknown.last_mut().unwrap() = b'X';
        assert!(decode(&unknown).is_err());
    }
    assert!(encode([0; 16], None).is_err());
    assert!(decode(b"orchbee-checkpoint-restore-v1:00000000000000000000000000000000:N").is_err());
    assert!(decode(b"orchbee-checkpoint-restore-v1:ABababababababababababababababab:E").is_err());
}
