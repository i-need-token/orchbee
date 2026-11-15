use super::*;

#[test]
fn status_field_accepts_only_one_valid_hex_value() {
    let good = "Name:\tfoo\nSigIgn:\t0000000000000006\nSigBlk:\t0\n";
    assert_eq!(status_field(good, "SigIgn:").unwrap(), 6);
    assert_eq!(
        status_field("SigIgn:\tffffffffffffffff\n", "SigIgn:").unwrap(),
        u64::MAX
    );
    for bad in [
        "SigIgn:\n",                    // empty
        "SigIgn:   \n",                 // whitespace only
        "SigIgn:\t\n",                  // tab only
        "SigIgn:\t00000000000000006\n", // seventeen digits
        "SigIgn:\t6 extra\n",           // trailing token
        "SigIgn:\tzz\n",                // non-hex
        "NoSuch:\t1\n",                 // missing field
        "SigIgn:\t1\nSigIgn:\t2\n",     // duplicated field
    ] {
        assert!(
            matches!(status_field(bad, "SigIgn:"), Err(ArmError::MalformedStatus)),
            "{bad:?} must be rejected"
        );
    }
}
