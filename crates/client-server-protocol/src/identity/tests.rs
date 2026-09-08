use std::any::TypeId;

use super::{IdentityError, RequestId, SiteId, TerminalId, WorkerId};

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

#[test]
fn logical_identity_domains_remain_distinct_while_admitting_equal_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    const SHARED_IDENTITY: &str = "abcdefabcdefabcdefabcdefabcdefab";

    let site_id = SiteId::parse(SHARED_IDENTITY)?;
    let worker_id = WorkerId::parse(SHARED_IDENTITY)?;
    let terminal_id = TerminalId::parse(SHARED_IDENTITY)?;

    assert_eq!(site_id.as_str(), worker_id.as_str());
    assert_eq!(worker_id.as_str(), terminal_id.as_str());
    assert_ne!(TypeId::of::<SiteId>(), TypeId::of::<WorkerId>());
    assert_ne!(TypeId::of::<WorkerId>(), TypeId::of::<TerminalId>());
    assert_ne!(TypeId::of::<SiteId>(), TypeId::of::<TerminalId>());

    assert_eq!(
        SiteId::parse("00000000000000000000000000000000"),
        Err(IdentityError::ZeroSentinel)
    );
    assert_eq!(
        TerminalId::parse("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        Err(IdentityError::InvalidFormat)
    );
    Ok(())
}
