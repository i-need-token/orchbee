use super::{IdentityError, RequestId, WorkerId};

#[test]
fn accepts_only_nonzero_lowercase_opaque_identities() {
    assert_eq!(
        RequestId::parse("11111111111111111111111111111111").map(|identity| identity.to_string()),
        Ok("11111111111111111111111111111111".to_owned())
    );
    assert!(WorkerId::parse("abcdefabcdefabcdefabcdefabcdefab").is_ok());
    assert_eq!(
        RequestId::parse("00000000000000000000000000000000"),
        Err(IdentityError::ZeroSentinel)
    );
    for invalid in [
        "1111111111111111111111111111111",
        "111111111111111111111111111111111",
        "1111111111111111111111111111111g",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
        assert_eq!(
            RequestId::parse(invalid),
            Err(IdentityError::InvalidFormat),
            "identity {invalid}"
        );
    }
}
