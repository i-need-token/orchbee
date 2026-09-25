use super::*;
use crate::{LocalClientMessage, read_message, write_message};

fn sample(mode: ServerMode) -> ServerMetadata {
    ServerMetadata::new(
        "0.1.0",
        ClientHello::new("abc").unwrap(),
        NonZeroU32::new(42).unwrap(),
        NonZeroU8::new(2).unwrap(),
        mode,
        7,
    )
    .unwrap()
}

#[test]
fn metadata_golden_and_unknown_feature_bits_preserve_bounded_diagnostic_data() {
    let expected = b"\x07\x050.1.0\x03abc\0\0\0\x2a\x02\x01\0\0\0\x07";
    assert_eq!(encode(&sample(ServerMode::ManagedReady)), expected);
    assert_eq!(decode(expected).unwrap(), sample(ServerMode::ManagedReady));
    for mode in [
        ServerMode::Unmanaged,
        ServerMode::ManagedReady,
        ServerMode::ManagedStopping,
    ] {
        let maximum = ServerMetadata::new(
            &"v".repeat(64),
            ClientHello::new(&"c".repeat(64)).unwrap(),
            NonZeroU32::new(u32::MAX).unwrap(),
            NonZeroU8::new(u8::MAX).unwrap(),
            mode,
            u32::MAX,
        )
        .unwrap();
        let mut bytes = Vec::new();
        write_message(
            &mut bytes,
            &LocalClientMessage::ServerMetadata(maximum.clone()),
        )
        .unwrap();
        assert_eq!(bytes.len(), 8 + 141);
        assert_eq!(
            read_message(&mut bytes.as_slice()).unwrap(),
            LocalClientMessage::ServerMetadata(maximum)
        );
    }
    let mut hello = Vec::new();
    write_message(
        &mut hello,
        &LocalClientMessage::ClientHello(ClientHello::new("abc").unwrap()),
    )
    .unwrap();
    assert_eq!(hello, b"OBLC\0\x01\0\x05\x01\x03abc");
    let mut request = Vec::new();
    write_message(&mut request, &LocalClientMessage::ServerMetadataRequest).unwrap();
    assert_eq!(request, b"OBLC\0\x01\0\x01\x06");
    assert_eq!(
        read_message(&mut request.as_slice()).unwrap(),
        LocalClientMessage::ServerMetadataRequest
    );
    assert!(read_message(&mut b"OBLC\0\x01\0\x02\x06\0".as_slice()).is_err());
}

#[test]
fn incomplete_invalid_and_extended_metadata_never_decode() {
    let original = encode(&sample(ServerMode::ManagedReady));
    for end in 0..original.len() {
        assert!(decode(&original[..end]).is_err(), "prefix {end}");
    }
    for (offset, value) in [
        (0, 6),
        (1, 0),
        (1, 65),
        (2, 0x1b),
        (2, b' '),
        (2, 0xff),
        (7, 0),
        (7, 65),
        (14, 0),
        (15, 0),
        (16, 3),
    ] {
        let mut bad = original.clone();
        bad[offset] = value;
        assert!(
            decode(&bad).is_err(),
            "invalid offset {offset} value {value}"
        );
    }
    let mut trailing = original;
    trailing.push(0);
    assert!(decode(&trailing).is_err());
    for invalid in ["", "version space", "v\n", "版本", &"v".repeat(65)] {
        assert!(
            ServerMetadata::new(
                invalid,
                ClientHello::new("a").unwrap(),
                NonZeroU32::MIN,
                NonZeroU8::MIN,
                ServerMode::Unmanaged,
                0
            )
            .is_err()
        );
    }
}
