use orchbee_capability_node_protocol::{
    AdoptBinding, AdoptBindingState, AdoptCloseInFlight, AdoptHostFailure, AdoptHostKind,
    AdoptHostOutcome, AdoptHostPage, AdoptHostPageKey, AdoptHostProcess, AdoptHostRequest,
    AdoptHostResult, AdoptHostSlot, AdoptHostTarget, CapabilityNodeMessage as Message,
    DeclinedRequestKind, HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity,
    LifecycleStepOrdinal, MAXIMUM_ADOPT_HOST_BINDINGS, MAXIMUM_ADOPT_HOST_PAGE_BINDINGS,
    MAXIMUM_FRAME_PAYLOAD_BYTES, OperationId, TerminalFence, TerminalId, WorkerGeneration,
    WorkerId, WorkerRequestDeclined, decode_message, encode_message, read_message,
};
use std::{
    io::Cursor,
    num::{NonZeroU32, NonZeroU64},
};

fn key(index: u16, last: bool) -> AdoptHostPageKey {
    AdoptHostPageKey {
        adoption_id: OperationId::from_bytes([0x11; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([0x22; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(0x0102).unwrap()),
        page: AdoptHostPage { index, last },
    }
}

fn epoch() -> HostEpoch {
    HostEpoch::from_bytes([0x33; 16]).unwrap()
}

fn process(id: u32, created: u64) -> AdoptHostProcess {
    AdoptHostProcess {
        process_id: NonZeroU32::new(id).unwrap(),
        creation_time: NonZeroU64::new(created).unwrap(),
    }
}

fn slot(slot: u8, process: Option<AdoptHostProcess>, epoch: Option<HostEpoch>) -> AdoptHostSlot {
    AdoptHostSlot {
        slot,
        kind: AdoptHostKind::Host { process },
        epoch,
    }
}

/// One Host in the first slot.
fn host(with_process: bool, epoch: Option<HostEpoch>) -> AdoptHostTarget {
    let registered = with_process.then(|| process(0x1234, 0x5678));
    AdoptHostTarget::new(vec![slot(0, registered, epoch)], None).unwrap()
}

/// A binding whose terminal, creation and Host terminal IDs all derive from `seed`.
fn binding(seed: u16, close: Option<AdoptCloseInFlight>) -> AdoptBinding {
    let [high, low] = seed.to_be_bytes();
    let id = |tag: u8| {
        let mut bytes = [tag; 16];
        bytes[14] = high;
        bytes[15] = low;
        bytes
    };
    AdoptBinding {
        terminal_id: TerminalId::from_bytes(id(0x44)).unwrap(),
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes(id(0x55)).unwrap(),
            epoch(),
            HostTerminalId::from_bytes(id(0x66)).unwrap(),
        ),
        state: AdoptBindingState::Started,
        fence: TerminalFence::new(NonZeroU64::MAX),
        close,
    }
}

fn close() -> AdoptCloseInFlight {
    AdoptCloseInFlight {
        close_operation_id: OperationId::from_bytes([0x77; 16]).unwrap(),
        attempt_generation: NonZeroU64::new(3).unwrap(),
        step_ordinal: LifecycleStepOrdinal::new(2).unwrap(),
        step_effect_unknown: true,
    }
}

fn roundtrip(message: &Message) -> Vec<u8> {
    let frame = encode_message(message).unwrap();
    assert_eq!(&decode_message(&frame).unwrap(), message);
    assert_eq!(
        &read_message(&mut Cursor::new(frame.clone())).unwrap(),
        message
    );
    frame
}

/// The fields every page and answer begin with after their kind: adoption, Worker, generation.
fn identity_bytes() -> Vec<u8> {
    [&[0x11; 16][..], &[0x22; 16], &[0, 0, 0, 0, 0, 0, 1, 2]].concat()
}

#[test]
fn pages_and_answers_match_hand_derived_bytes() {
    let started = AdoptBinding {
        terminal_id: TerminalId::from_bytes([0x44; 16]).unwrap(),
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes([0x55; 16]).unwrap(),
            epoch(),
            HostTerminalId::from_bytes([0x66; 16]).unwrap(),
        ),
        state: AdoptBindingState::Started,
        fence: TerminalFence::new(NonZeroU64::new(7).unwrap()),
        close: Some(close()),
    };
    let awaiting = AdoptBinding {
        terminal_id: TerminalId::from_bytes([0x48; 16]).unwrap(),
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes([0x58; 16]).unwrap(),
            epoch(),
            HostTerminalId::from_bytes([0x68; 16]).unwrap(),
        ),
        state: AdoptBindingState::AwaitingStart,
        fence: TerminalFence::new(NonZeroU64::MIN),
        close: None,
    };
    let page = Message::AdoptHostRequest(
        AdoptHostRequest::new(
            key(1, false),
            host(true, Some(epoch())),
            vec![started, awaiting],
        )
        .unwrap(),
    );
    let expected = [
        &b"OBCN\x00\x01\x00\xfe"[..],
        &[40],
        &identity_bytes(),
        // One slot: slot 0, a Host with process 0x1234 created at 0x5678, present epoch.
        &[
            1, 0, 1, 1, 0, 0, 0x12, 0x34, 0, 0, 0, 0, 0, 0, 0x56, 0x78, 1,
        ],
        &[0x33; 16],
        // No Keeper.
        &[0],
        // Page 1, not last, two bindings.
        &[0, 1, 0, 0, 2],
        // Started, fence 7, Close attempt 3 step 2 with an unknown effect.
        &[0x44; 16],
        &[0x55; 16],
        &[0x33; 16],
        &[0x66; 16],
        &[2, 0, 0, 0, 0, 0, 0, 0, 7, 1],
        &[0x77; 16],
        &[0, 0, 0, 0, 0, 0, 0, 3, 2, 1],
        // Awaiting start, fence 1, no Close.
        &[0x48; 16],
        &[0x58; 16],
        &[0x33; 16],
        &[0x68; 16],
        &[1, 0, 0, 0, 0, 0, 0, 0, 1, 0],
    ]
    .concat();
    assert_eq!(roundtrip(&page), expected);

    let no_host = Message::AdoptHostRequest(
        AdoptHostRequest::new(key(0, true), AdoptHostTarget::none(), Vec::new()).unwrap(),
    );
    let expected = [
        &b"OBCN\x00\x01\x00\x30"[..],
        &[40],
        &identity_bytes(),
        &[0, 0, 0, 0, 1, 0, 0],
    ]
    .concat();
    assert_eq!(roundtrip(&no_host), expected);

    // An older Host in slot 2 with neither recorded fact, and a recovery in slot 1 with the Keeper.
    let recovery = Message::AdoptHostRequest(
        AdoptHostRequest::new(
            key(0, true),
            AdoptHostTarget::new(
                vec![
                    slot(2, None, None),
                    AdoptHostSlot {
                        slot: 1,
                        kind: AdoptHostKind::Recovery,
                        epoch: Some(epoch()),
                    },
                ],
                Some(process(0x9abc, 0xdef0)),
            )
            .unwrap(),
            Vec::new(),
        )
        .unwrap(),
    );
    let expected = [
        &b"OBCN\x00\x01\x00\x53"[..],
        &[40],
        &identity_bytes(),
        &[2, 2, 1, 0, 0, 1, 2, 1],
        &[0x33; 16],
        &[1, 0, 0, 0x9a, 0xbc, 0, 0, 0, 0, 0, 0, 0xde, 0xf0],
        &[0, 0, 1, 0, 0],
    ]
    .concat();
    assert_eq!(roundtrip(&recovery), expected);

    let adopted = Message::AdoptHostResult(
        AdoptHostResult::new(
            key(1, true),
            AdoptHostOutcome::Adopted {
                imported: 5,
                quarantined: 1,
                missing: vec![TerminalId::from_bytes([0x48; 16]).unwrap()],
            },
        )
        .unwrap(),
    );
    let expected = [
        &b"OBCN\x00\x01\x00\x47"[..],
        &[41],
        &identity_bytes(),
        &[0, 1, 1, 2, 0, 0, 0, 5, 0, 0, 0, 1, 0, 1],
        &[0x48; 16],
    ]
    .concat();
    assert_eq!(roundtrip(&adopted), expected);

    let accepted = Message::AdoptHostResult(
        AdoptHostResult::new(key(0, false), AdoptHostOutcome::PageAccepted).unwrap(),
    );
    let expected = [
        &b"OBCN\x00\x01\x00\x2d"[..],
        &[41],
        &identity_bytes(),
        &[0, 0, 0, 1],
    ]
    .concat();
    assert_eq!(roundtrip(&accepted), expected);

    let failures = [
        (AdoptHostFailure::HostGone, 1),
        (AdoptHostFailure::HostUnreachable, 2),
        (AdoptHostFailure::HostUnverified, 3),
        (AdoptHostFailure::HostMismatch, 4),
        (AdoptHostFailure::HostRefused, 5),
        (AdoptHostFailure::InventoryFailed, 6),
    ];
    for (failure, tag) in failures {
        let failed = Message::AdoptHostResult(
            AdoptHostResult::new(key(2, true), AdoptHostOutcome::Failed(failure)).unwrap(),
        );
        let expected = [
            &b"OBCN\x00\x01\x00\x2e"[..],
            &[41],
            &identity_bytes(),
            &[0, 2, 1, 3, tag],
        ]
        .concat();
        assert_eq!(roundtrip(&failed), expected, "{failure:?}");
    }
}

#[test]
fn the_largest_page_and_answer_fit_one_frame() {
    let bindings = (0..u16::try_from(MAXIMUM_ADOPT_HOST_PAGE_BINDINGS).unwrap())
        .map(|seed| binding(seed, Some(close())))
        .collect();
    // The largest target: four Hosts, each with its process and epoch.
    let four = AdoptHostTarget::new(
        (0..4_u8)
            .map(|index| {
                let epoch = HostEpoch::from_bytes([0x33 + index; 16]).unwrap();
                slot(index, Some(process(0x1234, 0x5678)), Some(epoch))
            })
            .collect(),
        None,
    )
    .unwrap();
    let largest = Message::AdoptHostRequest(
        AdoptHostRequest::new(key(u16::MAX, true), four, bindings).unwrap(),
    );
    let frame = roundtrip(&largest);
    assert_eq!(frame.len(), 8 + 176 + 256 * 100);
    assert!(frame.len() - 8 <= MAXIMUM_FRAME_PAYLOAD_BYTES);

    let missing = (0..u16::try_from(MAXIMUM_ADOPT_HOST_BINDINGS).unwrap())
        .map(|seed| binding(seed, None).terminal_id)
        .collect();
    let answer = Message::AdoptHostResult(
        AdoptHostResult::new(
            key(3, true),
            AdoptHostOutcome::Adopted {
                imported: u32::MAX,
                quarantined: u32::MAX,
                missing,
            },
        )
        .unwrap(),
    );
    assert_eq!(roundtrip(&answer).len(), 8 + 55 + 1_024 * 16);

    // A Host target may lack either recorded fact.
    for target in [
        host(false, None),
        host(true, None),
        host(false, Some(epoch())),
    ] {
        roundtrip(&Message::AdoptHostRequest(
            AdoptHostRequest::new(key(0, true), target, vec![binding(1, None)]).unwrap(),
        ));
    }
}

#[test]
fn contradictory_pages_and_answers_are_refused() {
    let refused_page = |target, bindings: Vec<AdoptBinding>| {
        assert!(AdoptHostRequest::new(key(0, true), target, bindings).is_err());
    };
    refused_page(
        host(false, None),
        (0..=u16::try_from(MAXIMUM_ADOPT_HOST_PAGE_BINDINGS).unwrap())
            .map(|seed| binding(seed, None))
            .collect(),
    );
    refused_page(AdoptHostTarget::none(), vec![binding(1, None)]);
    let other_epoch = HostEpoch::from_bytes([0x34; 16]).unwrap();
    refused_page(host(true, Some(other_epoch)), vec![binding(1, None)]);
    let same_terminal = AdoptBinding {
        terminal_id: binding(1, None).terminal_id,
        ..binding(2, None)
    };
    refused_page(host(false, None), vec![binding(1, None), same_terminal]);
    let reused = binding(1, None).identity;
    let same_host_terminal = AdoptBinding {
        identity: HostTerminalIdentity::new(
            binding(2, None).identity.creation_id(),
            epoch(),
            reused.terminal_id(),
        ),
        ..binding(2, None)
    };
    refused_page(
        host(false, None),
        vec![binding(1, None), same_host_terminal],
    );
    let same_creation = AdoptBinding {
        identity: HostTerminalIdentity::new(
            reused.creation_id(),
            epoch(),
            binding(2, None).identity.terminal_id(),
        ),
        ..binding(2, None)
    };
    refused_page(host(false, None), vec![binding(1, None), same_creation]);

    let recovery = |slot| AdoptHostSlot {
        slot,
        kind: AdoptHostKind::Recovery,
        epoch: None,
    };
    let keeper = Some(process(0x9abc, 0xdef0));
    for (slots, keeper) in [
        ((0..5).map(|index| slot(index, None, None)).collect(), None),
        (vec![slot(4, None, None)], None),
        (vec![slot(1, None, None), slot(1, None, None)], None),
        (
            vec![slot(0, None, Some(epoch())), slot(1, None, Some(epoch()))],
            None,
        ),
        (vec![slot(0, None, None)], keeper),
        (vec![recovery(0)], None),
    ] {
        assert!(AdoptHostTarget::new(slots, keeper).is_err());
    }

    let adopted = |missing| AdoptHostOutcome::Adopted {
        imported: 0,
        quarantined: 0,
        missing,
    };
    assert!(AdoptHostResult::new(key(0, true), AdoptHostOutcome::PageAccepted).is_err());
    assert!(AdoptHostResult::new(key(0, false), adopted(Vec::new())).is_err());
    let failed = AdoptHostOutcome::Failed(AdoptHostFailure::HostGone);
    assert!(AdoptHostResult::new(key(0, false), failed).is_err());
    let terminal = binding(1, None).terminal_id;
    assert!(AdoptHostResult::new(key(0, true), adopted(vec![terminal, terminal])).is_err());
    let too_many = (0..=u16::try_from(MAXIMUM_ADOPT_HOST_BINDINGS).unwrap())
        .map(|seed| binding(seed, None).terminal_id)
        .collect();
    assert!(AdoptHostResult::new(key(0, true), adopted(too_many)).is_err());
}

#[test]
fn malformed_pages_and_answers_are_refused() {
    let page = encode_message(&Message::AdoptHostRequest(
        AdoptHostRequest::new(
            key(1, false),
            host(true, Some(epoch())),
            vec![binding(1, Some(close()))],
        )
        .unwrap(),
    ))
    .unwrap();
    // Offsets count from the frame start: an 8-byte header, the kind, then 40 identity bytes.
    const TARGET: usize = 49;
    const PROCESS_ID: usize = TARGET + 4;
    const CREATION: usize = PROCESS_ID + 4;
    const KEEPER: usize = CREATION + 25;
    const LAST: usize = TARGET + 36;
    const COUNT: usize = LAST + 1;
    const STATE: usize = COUNT + 2 + 64;
    const FENCE: usize = STATE + 1;
    const CLOSE: usize = FENCE + 8;
    const ATTEMPT: usize = CLOSE + 17;
    const ORDINAL: usize = ATTEMPT + 8;
    type Corruption = fn(&mut Vec<u8>);
    let cases: [(&str, Corruption); 18] = [
        ("five slots", |bad| bad[TARGET] = 5),
        ("slot past 3", |bad| bad[TARGET + 1] = 4),
        ("unknown slot kind", |bad| bad[TARGET + 2] = 3),
        ("process flag", |bad| bad[TARGET + 3] = 2),
        ("zero process id", |bad| bad[PROCESS_ID..CREATION].fill(0)),
        ("zero creation time", |bad| {
            bad[CREATION..CREATION + 8].fill(0)
        }),
        ("zero epoch", |bad| bad[CREATION + 9..CREATION + 25].fill(0)),
        ("keeper flag", |bad| bad[KEEPER] = 2),
        ("last flag", |bad| bad[LAST] = 2),
        ("count beyond the bindings", |bad| bad[COUNT + 1] = 2),
        ("unknown state", |bad| bad[STATE] = 3),
        ("zero fence", |bad| bad[FENCE..FENCE + 8].fill(0)),
        ("close flag", |bad| bad[CLOSE] = 2),
        ("zero attempt", |bad| bad[ATTEMPT..ATTEMPT + 8].fill(0)),
        ("zero ordinal", |bad| bad[ORDINAL] = 0),
        ("ordinal past 32", |bad| bad[ORDINAL] = 33),
        ("unknown effect flag", |bad| bad[ORDINAL + 1] = 2),
        ("bindings for no Host", |bad| {
            // No slot drops the slot's 32 bytes and keeps the binding.
            bad[TARGET] = 0;
            bad.drain(TARGET + 1..TARGET + 33);
            bad[7] -= 32;
        }),
    ];
    for (case, corrupt) in cases {
        let mut bad = page.clone();
        corrupt(&mut bad);
        assert!(decode_message(&bad).is_err(), "{case}");
        assert!(read_message(&mut Cursor::new(bad)).is_err(), "{case}");
    }
    let mut over = page.clone();
    over[COUNT..COUNT + 2].copy_from_slice(&257_u16.to_be_bytes());
    assert!(decode_message(&over).is_err(), "257 bindings");
    for length in [page.len() - 1, page.len() + 1] {
        let mut bad = page.clone();
        bad.resize(length, 0);
        bad[6..8].copy_from_slice(&u16::try_from(length - 8).unwrap().to_be_bytes());
        assert!(decode_message(&bad).is_err(), "length {length}");
    }

    let answer = encode_message(&Message::AdoptHostResult(
        AdoptHostResult::new(
            key(1, true),
            AdoptHostOutcome::Adopted {
                imported: 1,
                quarantined: 0,
                missing: vec![binding(1, None).terminal_id],
            },
        )
        .unwrap(),
    ))
    .unwrap();
    const OUTCOME: usize = 52;
    let cases: [(&str, Corruption); 5] = [
        ("unknown outcome", |bad| bad[OUTCOME] = 4),
        ("missing beyond the bound", |bad| {
            bad[OUTCOME + 9..OUTCOME + 11].copy_from_slice(&1_025_u16.to_be_bytes());
        }),
        ("zero missing terminal", |bad| bad[OUTCOME + 11..].fill(0)),
        ("short missing list", |bad| {
            bad.pop();
            bad[7] -= 1;
        }),
        ("not the last page", |bad| bad[OUTCOME - 1] = 0),
    ];
    for (case, corrupt) in cases {
        let mut bad = answer.clone();
        corrupt(&mut bad);
        assert!(decode_message(&bad).is_err(), "{case}");
        assert!(read_message(&mut Cursor::new(bad)).is_err(), "{case}");
    }
    let mut unknown_failure = encode_message(&Message::AdoptHostResult(
        AdoptHostResult::new(
            key(1, true),
            AdoptHostOutcome::Failed(AdoptHostFailure::HostGone),
        )
        .unwrap(),
    ))
    .unwrap();
    unknown_failure[OUTCOME + 1] = u8::MAX;
    assert!(decode_message(&unknown_failure).is_err());
}

#[test]
fn a_declined_page_names_its_adoption() {
    let page = Message::AdoptHostRequest(
        AdoptHostRequest::new(key(2, false), host(false, None), Vec::new()).unwrap(),
    );
    assert_eq!(
        WorkerRequestDeclined::of(&page),
        Some(WorkerRequestDeclined {
            kind: DeclinedRequestKind::AdoptHost,
            identity: [0x11; 16],
        })
    );
    let answer = Message::AdoptHostResult(
        AdoptHostResult::new(key(2, false), AdoptHostOutcome::PageAccepted).unwrap(),
    );
    assert_eq!(WorkerRequestDeclined::of(&answer), None);
}
