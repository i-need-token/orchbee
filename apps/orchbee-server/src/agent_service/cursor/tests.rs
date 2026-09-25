use super::*;

const TEST_INSTANCE: u64 = 0x0102_0304_0506_0708;

fn test_key() -> [u8; KEY_BYTES] {
    let mut key = [0_u8; KEY_BYTES];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = index as u8;
    }
    key
}

fn authority() -> AgentCursorAuthority {
    AgentCursorAuthority::fixed(test_key(), TEST_INSTANCE)
}

#[test]
fn known_answer_matches_independent_vector() {
    // Independently produced with Python hmac/hashlib/base64 over the same
    // fixed key, instance, and field values.
    let authority = authority();
    let cursor = authority.mint(true, 42, 3);
    assert_eq!(
        cursor.as_str(),
        "v1.AQEAAQECAwQFBgcIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAqAAAAAAAAAAM.\
         tDQVQY9XjOh5gm0MGh0BWDMp7Yc09tOiHvsZhzpPhfM"
    );
    let cursor = authority.mint(false, 42, 3);
    assert_eq!(
        cursor.as_str(),
        "v1.AQEAAAECAwQFBgcIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAqAAAAAAAAAAM.\
         Pd7Ue9o36CelP05AQAX7l1jIBWvgOQvdlvaUHM6w7cc"
    );
}

#[test]
fn minted_cursor_verifies_and_round_trips() {
    let authority = authority();
    for (include_ended, revision, position, length) in [
        (false, 0_u64, 0_u64, 1_u64),
        (true, 9, 4, 8),
        (true, 9, 7, 8),
    ] {
        let cursor = authority.mint(include_ended, revision, position);
        assert_eq!(cursor.as_str().len(), 106);
        assert_eq!(
            authority.verify(cursor.as_str(), include_ended, revision, length),
            Ok(position)
        );
    }
}

#[test]
fn open_produces_working_authority() {
    let authority = AgentCursorAuthority::open().expect("urandom available");
    let cursor = authority.mint(false, 0, 0);
    assert_eq!(authority.verify(cursor.as_str(), false, 0, 1), Ok(0));
}

#[test]
fn mutated_tag_byte_is_invalid() {
    let authority = authority();
    let cursor = authority.mint(true, 5, 2).as_str().to_owned();
    let mut mutated = cursor.clone().into_bytes();
    let last = mutated.len() - 1;
    mutated[last] = if mutated[last] == b'A' { b'B' } else { b'A' };
    let mutated = String::from_utf8(mutated).expect("ascii");
    assert_eq!(
        authority.verify(&mutated, true, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
}

#[test]
fn mutated_payload_byte_is_invalid() {
    let authority = authority();
    let cursor = authority.mint(false, 5, 2).as_str().to_owned();
    let mut mutated = cursor.clone().into_bytes();
    mutated[10] = if mutated[10] == b'A' { b'B' } else { b'A' };
    let mutated = String::from_utf8(mutated).expect("ascii");
    assert_eq!(
        authority.verify(&mutated, false, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
}

#[test]
fn noncanonical_pad_bits_are_invalid() {
    // The last character of each segment carries pad bits; mutating only those
    // bits must still be rejected through canonical re-encoding.
    let authority = authority();
    let cursor = authority.mint(true, 5, 2).as_str().to_owned();

    let mut payload_mutant = cursor.clone().into_bytes();
    let index = 3 + 58;
    payload_mutant[index] = match payload_mutant[index] {
        b'M' => b'N',
        byte => byte ^ 0x01,
    };
    let payload_mutant = String::from_utf8(payload_mutant).expect("ascii");
    assert_eq!(
        authority.verify(&payload_mutant, true, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );

    let mut tag_mutant = cursor.into_bytes();
    let index = tag_mutant.len() - 1;
    tag_mutant[index] = match tag_mutant[index] {
        b'M' => b'N',
        byte => byte ^ 0x01,
    };
    let tag_mutant = String::from_utf8(tag_mutant).expect("ascii");
    assert_eq!(
        authority.verify(&tag_mutant, true, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
}

#[test]
fn filter_mismatch_is_invalid() {
    // A cursor minted for one includeEnded filter is rejected under another.
    let authority = authority();
    let cursor = authority.mint(true, 5, 2).as_str().to_owned();
    assert_eq!(
        authority.verify(&cursor, false, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
    let cursor = authority.mint(false, 5, 2).as_str().to_owned();
    assert_eq!(
        authority.verify(&cursor, true, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
}

#[test]
fn other_instance_and_other_key_are_invalid() {
    let cursor = authority().mint(false, 5, 2).as_str().to_owned();
    let foreign = AgentCursorAuthority::fixed(test_key(), TEST_INSTANCE + 1);
    assert_eq!(
        foreign.verify(&cursor, false, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
    let mut other_key = test_key();
    other_key[0] ^= 0xff;
    let foreign = AgentCursorAuthority::fixed(other_key, TEST_INSTANCE);
    assert_eq!(
        foreign.verify(&cursor, false, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
}

#[test]
fn revision_fencing_is_exact() {
    let authority = authority();
    let cursor = authority.mint(false, 5, 2).as_str().to_owned();
    assert_eq!(
        authority.verify(&cursor, false, 6, 10),
        Err(AgentCursorRejection::Stale)
    );
    assert_eq!(
        authority.verify(&cursor, false, 4, 10),
        Err(AgentCursorRejection::Invalid)
    );
    assert_eq!(authority.verify(&cursor, false, 5, 10), Ok(2));
}

#[test]
fn impossible_position_is_invalid() {
    let authority = authority();
    let cursor = authority.mint(false, 5, 10).as_str().to_owned();
    assert_eq!(
        authority.verify(&cursor, false, 5, 10),
        Err(AgentCursorRejection::Invalid)
    );
    assert_eq!(authority.verify(&cursor, false, 5, 11), Ok(10));
}

#[test]
fn reserved_fields_are_enforced() {
    let authority = authority();
    let base = AgentCursorPayload {
        version: AgentCursorPayload::VERSION,
        operation: AgentCursorPayload::LIST_OPERATION,
        flags: 0,
        instance: TEST_INSTANCE,
        target: 0,
        revision: 5,
        position: 2,
    };
    for mutation in [
        AgentCursorPayload { version: 2, ..base },
        AgentCursorPayload {
            operation: 2,
            ..base
        },
        AgentCursorPayload {
            flags: 0b10,
            ..base
        },
        AgentCursorPayload {
            flags: AgentCursorPayload::FLAG_INCLUDE_ENDED,
            ..base
        },
        AgentCursorPayload { target: 1, ..base },
        AgentCursorPayload {
            revision: MAXIMUM_SAFE_INTEGER + 1,
            ..base
        },
        AgentCursorPayload {
            instance: 0,
            ..base
        },
    ] {
        let bytes = mutation.encode();
        let tag = authority.tag(&bytes);
        let text = format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(bytes),
            URL_SAFE_NO_PAD.encode(tag)
        );
        assert_eq!(
            authority.verify(&text, false, 5, 10),
            Err(AgentCursorRejection::Invalid),
            "{mutation:?} must be rejected"
        );
    }
}

#[test]
fn malformed_cursor_is_invalid() {
    let authority = authority();
    for text in [
        "",
        "v1.",
        "not-a-cursor",
        "v2.AQEAAQECAwQFBgcIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAqAAAAAAAAAAM.Pd7Ue9o36CelP05AQAX7l1jIBWvgOQvdlvaUHM6w7cc",
    ] {
        assert_eq!(
            authority.verify(text, false, 5, 10),
            Err(AgentCursorRejection::Invalid),
            "{text:?} must be rejected"
        );
    }
}

#[test]
fn changes_cursor_known_answer_boot_domain_and_historical_revision() {
    let owner = authority();
    let cursor = owner.mint_changes(42);
    assert_eq!(
        cursor.as_str(),
        "v1.AQIAAAECAwQFBgcIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAqAAAAAAAAAAA.RvXNWP62aTlr20YK8M7G-Fcml8ibGVv6iy-DcItdYWk"
    );
    assert_eq!(owner.verify_changes(&cursor, 42), Ok(42));
    assert_eq!(owner.verify_changes(&cursor, 99), Ok(42));
    assert_eq!(
        owner.verify_changes(&cursor, 41),
        Err(AgentCursorRejection::Invalid)
    );
    let list = owner.mint(false, 42, 0);
    assert_eq!(
        owner.verify_changes(&AgentChangesCursor::parse(list.as_str()).unwrap(), 42),
        Err(AgentCursorRejection::Invalid)
    );
    assert_eq!(
        owner.verify(cursor.as_str(), false, 42, 1),
        Err(AgentCursorRejection::Invalid)
    );
    let other = AgentCursorAuthority::fixed([77; KEY_BYTES], TEST_INSTANCE + 1);
    assert_eq!(
        other.verify_changes(&cursor, 42),
        Err(AgentCursorRejection::Invalid)
    );
}
