use super::*;

fn framed(generation: u64, bytes: &[u8]) -> Candidate {
    Candidate::decode(
        &encode(bytes, NonZeroU64::new(generation).unwrap()),
        64,
        false,
    )
}

#[test]
fn framing_preserves_exact_payload_and_has_canonical_generation_and_digest() {
    let encoded = encode(b"abc", NonZeroU64::new(7).unwrap());
    assert_eq!(&encoded[..13], b"OBCK\x01\x07\0\0\0\0\0\0\0");
    assert_eq!(
        &encoded[13..45],
        &[
            0x57, 0x1d, 0x9b, 0x4f, 0x8c, 0x56, 0x00, 0x08, 0xa6, 0x47, 0x18, 0xb3, 0x38, 0xd4,
            0x4e, 0xa3, 0x91, 0x90, 0x2b, 0x4f, 0xc8, 0x33, 0x64, 0xdc, 0xa1, 0xea, 0xce, 0x61,
            0xcb, 0x08, 0x35, 0x28,
        ]
    );
    assert_eq!(
        Candidate::decode(&encoded, 3, false).payload(),
        Some(b"abc".as_slice())
    );
    assert!(matches!(
        Candidate::decode(&encoded, 2, false),
        Candidate::Invalid {
            generation: Some(7)
        }
    ));
    let mut changed = encoded.clone();
    changed[5] ^= 1;
    assert!(matches!(
        Candidate::decode(&changed, 3, false),
        Candidate::Invalid { .. }
    ));
    let mut changed = encoded.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert!(matches!(
        Candidate::decode(&changed, 3, false),
        Candidate::Invalid {
            generation: Some(7)
        }
    ));
    assert_eq!(encoded.len(), HEADER_BYTES + 3);
}

#[test]
fn legacy_is_only_admitted_in_its_original_slot_and_unknown_frames_are_not_plaintext() {
    assert_eq!(
        Candidate::decode(b"legacy", 6, true).payload(),
        Some(b"legacy".as_slice())
    );
    assert!(matches!(
        Candidate::decode(b"legacy", 6, false),
        Candidate::Invalid { generation: None }
    ));
    for bytes in [
        b"OBCK".as_slice(),
        b"OBCK\x02garbage",
        b"OBCK\x01\0\0\0\0\0\0\0\0",
    ] {
        assert!(matches!(
            Candidate::decode(bytes, 64, true),
            Candidate::Invalid { .. }
        ));
    }
}

#[test]
fn rotation_prefers_missing_then_invalid_then_oldest_and_never_replaces_latest_valid() {
    let candidates = [
        framed(8, b"newest"),
        framed(4, b"oldest"),
        framed(6, b"middle"),
    ];
    assert_eq!(latest(&candidates), Some(0));
    let publication = prepare(&candidates, b"next", 64).unwrap().unwrap();
    assert_eq!(publication.slot, 1);
    assert!(!publication.preserve_invalid);
    let next = Candidate::decode(&publication.encoded, 64, false);
    assert_eq!(next.generation(), Some(9));
    assert_eq!(next.payload(), Some(b"next".as_slice()));
    let invalid = [
        framed(8, b"newest"),
        Candidate::Invalid {
            generation: Some(11),
        },
        Candidate::Missing,
    ];
    let publication = prepare(&invalid, b"next", 64).unwrap().unwrap();
    assert_eq!(publication.slot, 2);
    assert_eq!(
        Candidate::decode(&publication.encoded, 64, false).generation(),
        Some(9)
    );
    let invalid = [
        framed(8, b"newest"),
        Candidate::Invalid {
            generation: Some(11),
        },
        framed(6, b"middle"),
    ];
    let publication = prepare(&invalid, b"next", 64).unwrap().unwrap();
    assert_eq!(publication.slot, 1);
    assert!(publication.preserve_invalid);
}

#[test]
fn conflicting_generations_are_preserved_and_fallback_uses_the_older_valid_payload() {
    let mut candidates = [framed(5, b"a"), framed(5, b"b"), framed(4, b"older")];
    let values = validate(&mut candidates, |bytes| Some(bytes.to_vec()));
    assert_eq!(values, [None, None, Some(b"older".to_vec())]);
    assert_eq!(latest(&candidates), Some(2));
    let publication = prepare(&candidates, b"next", 64).unwrap().unwrap();
    assert!(publication.preserve_invalid);
    assert_eq!(
        Candidate::decode(&publication.encoded, 64, false).generation(),
        Some(5)
    );
    let mut duplicates = [framed(5, b"same"), framed(5, b"same"), framed(4, b"older")];
    assert!(
        validate(&mut duplicates, |bytes| Some(bytes.to_vec()))
            .iter()
            .all(Option::is_some)
    );
}

#[test]
fn complete_payload_validation_can_reject_a_valid_digest_without_losing_its_generation() {
    let mut candidates = [
        framed(9, b"invalid-domain"),
        framed(8, b"valid-domain"),
        Candidate::Missing,
    ];
    let values = validate(&mut candidates, |bytes| {
        (bytes == b"valid-domain").then_some(42)
    });
    assert_eq!(values, [None, Some(42), None]);
    assert_eq!(candidates[0].generation(), Some(9));
    assert_eq!(latest(&candidates), Some(1));
}

#[test]
fn damaged_header_generation_cannot_exhaust_publication_after_a_valid_snapshot() {
    let mut damaged = encode(b"damaged", NonZeroU64::new(7).unwrap());
    damaged[5..13].copy_from_slice(&u64::MAX.to_le_bytes());
    let damaged = Candidate::decode(&damaged, 64, false);
    assert!(matches!(
        &damaged,
        Candidate::Invalid {
            generation: Some(generation)
        } if *generation == u64::MAX
    ));
    let candidates = [framed(7, b"valid"), damaged, Candidate::Missing];
    let publication = prepare(&candidates, b"next", 64).unwrap().unwrap();
    assert_eq!(publication.slot, 2);
    assert_eq!(
        Candidate::decode(&publication.encoded, 64, false).generation(),
        Some(8)
    );
}

#[test]
fn domain_invalid_generation_cannot_exhaust_an_all_invalid_store() {
    let mut candidates = [
        framed(u64::MAX, b"invalid-domain"),
        Candidate::Invalid { generation: None },
        Candidate::Missing,
    ];
    let values = validate(&mut candidates, |_| None::<()>);
    assert_eq!(values, [None, None, None]);
    assert!(matches!(
        &candidates[0],
        Candidate::Invalid {
            generation: Some(generation)
        } if *generation == u64::MAX
    ));
    let publication = prepare(&candidates, b"first-valid", 64).unwrap().unwrap();
    assert_eq!(publication.slot, 2);
    assert_eq!(
        Candidate::decode(&publication.encoded, 64, false).generation(),
        Some(1)
    );
}

#[test]
fn unchanged_payload_does_not_rotate_but_legacy_is_upgraded_and_exhaustion_refuses_effects() {
    let candidates = [framed(9, b"same"), framed(8, b"older"), Candidate::Missing];
    assert!(prepare(&candidates, b"same", 64).unwrap().is_none());
    let legacy = [
        Candidate::decode(b"same", 64, true),
        Candidate::Missing,
        Candidate::Missing,
    ];
    let publication = prepare(&legacy, b"same", 64).unwrap().unwrap();
    assert_eq!(publication.slot, 1);
    assert_eq!(
        Candidate::decode(&publication.encoded, 64, false).generation(),
        Some(1)
    );
    let candidates = [
        framed(u64::MAX, b"valid"),
        Candidate::Invalid { generation: None },
        Candidate::Missing,
    ];
    assert!(prepare(&candidates, b"next", 64).is_err());
    assert!(prepare(&candidates, &[0; 65], 64).is_err());
}

#[test]
fn corrupt_original_grammar_only_admits_nonzero_canonical_owned_names() {
    assert!(is_corrupt_original(
        "checkpoint.corrupt-00000000000000000000000000000001.raw"
    ));
    for name in [
        "checkpoint.corrupt-00000000000000000000000000000000.raw",
        "checkpoint.corrupt-0000000000000000000000000000000A.raw",
        "checkpoint.corrupt-0000000000000000000000000000001.raw",
        "checkpoint.corrupt-00000000000000000000000000000001.raw.extra",
        "other.corrupt-00000000000000000000000000000001.raw",
    ] {
        assert!(!is_corrupt_original(name));
    }
}
