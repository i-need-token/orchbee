use super::*;

#[test]
fn only_empty_or_exact_nonzero_lowercase_receipts_are_valid() {
    assert_eq!(parse(b"").unwrap(), ActivationState::Active);
    assert_eq!(
        parse(b"orchbee-stopped-v1:0123456789abcdef0123456789abcdef").unwrap(),
        ActivationState::Stopped {
            request_id: [
                1, 35, 69, 103, 137, 171, 205, 239, 1, 35, 69, 103, 137, 171, 205, 239
            ]
        }
    );
    for bad in [
        b"x".as_slice(),
        b"orchbee-stopped-v1:00000000000000000000000000000000",
        b"orchbee-stopped-v1:0123456789ABCDEF0123456789abcdef",
        b"orchbee-stopped-v1:01",
        b"orchbee-stopped-v2:0123456789abcdef0123456789abcdef",
        b"orchbee-stopped-v1:0123456789abcdef0123456789abcdef\n",
    ] {
        assert!(parse(bad).is_err());
    }
}
