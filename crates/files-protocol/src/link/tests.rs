use super::*;

fn call(value: u64) -> CallId {
    CallId::new(value).unwrap()
}

fn connection(value: u64) -> ConnectionId {
    ConnectionId::new(value).unwrap()
}

fn workspace(last: u8) -> WorkspaceId {
    let mut bytes = [0; 16];
    bytes[15] = last;
    WorkspaceId::from_bytes(bytes).unwrap()
}

fn build(commit: Option<[u8; 20]>) -> BuildIdentity {
    BuildIdentity::new("0.1.0-beta.27", commit, "ob-test-0123456789abcdef", [7; 32]).unwrap()
}

/// One message of every kind, with the optional members both present and absent.
fn every_kind() -> Vec<LinkMessage> {
    vec![
        LinkMessage::Hello(LinkHello {
            build: build(None),
            token: LinkToken::from_bytes([9; 32]),
        }),
        LinkMessage::Hello(LinkHello {
            build: build(Some([3; 20])),
            token: LinkToken::from_bytes([1; 32]),
        }),
        LinkMessage::Admitted,
        LinkMessage::Refused(LinkRefusal::BuildMismatch),
        LinkMessage::Request(LinkRequest {
            call: call(1),
            connection: connection(2),
            lane: LaneId::new(3).unwrap(),
            workspace: Some(workspace(4)),
            grant: GrantClass::Lifecycle,
            record: b"OBFB-request".to_vec(),
        }),
        LinkMessage::Request(LinkRequest {
            call: call(u64::MAX),
            connection: connection(u64::MAX),
            lane: LaneId::new(1).unwrap(),
            workspace: None,
            grant: GrantClass::TerminalCreate,
            record: vec![0; MAXIMUM_LINK_PAYLOAD_BYTES - 41],
        }),
        LinkMessage::Frame(LinkFrame {
            call: call(5),
            kind: FrameKind::Update,
            record: b"x".to_vec(),
        }),
        LinkMessage::Close {
            call: call(6),
            reason: CallClose::OutcomeUnknown,
        },
        LinkMessage::Cancel { call: call(7) },
        LinkMessage::Scope(ScopeUpdate {
            workspace: workspace(8),
            scope: WorkspaceScope::Root("C:\\Users\\person\\project".to_owned()),
        }),
        LinkMessage::Scope(ScopeUpdate {
            workspace: workspace(9),
            scope: WorkspaceScope::NoRoot,
        }),
        LinkMessage::Scope(ScopeUpdate {
            workspace: workspace(10),
            scope: WorkspaceScope::Closed,
        }),
        LinkMessage::ConnectionLost {
            connection: connection(11),
        },
        LinkMessage::LaneClosed {
            connection: connection(12),
            lane: LaneId::new(13).unwrap(),
        },
        LinkMessage::AgentSettled {
            workspace: workspace(14),
        },
        LinkMessage::Drain(DrainReason::Session),
        LinkMessage::Activity { holds: true },
    ]
}

#[test]
fn every_kind_round_trips_from_its_sender_only() {
    for message in every_kind() {
        let frame = encode_link_message(&message).unwrap();
        let sender = message.sender();
        assert_eq!(decode_link_message(&frame, sender), Ok(message.clone()));
        assert_eq!(
            read_link_message(&mut frame.as_slice(), sender).unwrap(),
            message
        );
        let other = match sender {
            LinkSide::Server => LinkSide::Files,
            LinkSide::Files => LinkSide::Server,
        };
        assert_eq!(
            decode_link_message(&frame, other),
            Err(LinkError::UnexpectedKind)
        );
    }
}

#[test]
fn a_frame_is_refused_before_its_body_is_read() {
    let frame = encode_link_message(&LinkMessage::Admitted).unwrap();
    let mut wrong_magic = frame.clone();
    wrong_magic[0] = b'X';
    assert_eq!(
        decode_link_message(&wrong_magic, LinkSide::Server),
        Err(LinkError::InvalidMagic)
    );
    let mut next_version = frame.clone();
    next_version[5] = 2;
    assert_eq!(
        decode_link_message(&next_version, LinkSide::Server),
        Err(LinkError::UnsupportedVersion)
    );
    let mut unknown_kind = frame.clone();
    unknown_kind[7] = 99;
    assert_eq!(
        decode_link_message(&unknown_kind, LinkSide::Server),
        Err(LinkError::UnexpectedKind)
    );
    // A length past the bound is refused from the header, before any body byte is allocated.
    let mut oversized = encode_link_message(&LinkMessage::Frame(LinkFrame {
        call: call(1),
        kind: FrameKind::Result,
        record: b"x".to_vec(),
    }))
    .unwrap();
    let length = u32::try_from(MAXIMUM_LINK_PAYLOAD_BYTES + 1).unwrap();
    oversized[8..12].copy_from_slice(&length.to_be_bytes());
    let error = read_link_message(&mut oversized.as_slice(), LinkSide::Files).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn values_outside_their_domain_are_refused() {
    let encode = |message| encode_link_message(&message).unwrap();
    let mut zero_call = encode(LinkMessage::Cancel { call: call(1) });
    zero_call[12..20].fill(0);
    assert_eq!(
        decode_link_message(&zero_call, LinkSide::Server),
        Err(LinkError::InvalidBody)
    );
    let mut zero_workspace = encode(LinkMessage::AgentSettled {
        workspace: workspace(1),
    });
    zero_workspace[12..28].fill(0);
    assert_eq!(
        decode_link_message(&zero_workspace, LinkSide::Server),
        Err(LinkError::InvalidBody)
    );
    let mut unknown_reason = encode(LinkMessage::Close {
        call: call(1),
        reason: CallClose::Complete,
    });
    unknown_reason[20] = 7;
    assert_eq!(
        decode_link_message(&unknown_reason, LinkSide::Files),
        Err(LinkError::InvalidBody)
    );
    let mut invalid_root = encode(LinkMessage::Scope(ScopeUpdate {
        workspace: workspace(1),
        scope: WorkspaceScope::Root("/a".to_owned()),
    }));
    let last = invalid_root.len() - 1;
    invalid_root[last] = 0xff;
    assert_eq!(
        decode_link_message(&invalid_root, LinkSide::Server),
        Err(LinkError::InvalidBody)
    );
    let mut uppercase_environment = encode(LinkMessage::Hello(LinkHello {
        build: build(None),
        token: LinkToken::from_bytes([0; 32]),
    }));
    let last = uppercase_environment.len() - 1;
    uppercase_environment[last] = b'F';
    assert_eq!(
        decode_link_message(&uppercase_environment, LinkSide::Files),
        Err(LinkError::InvalidBody)
    );
    assert_eq!(
        BuildIdentity::new("1.0.0", None, "", [0; 32]),
        Err(LinkError::InvalidBuild)
    );
    assert_eq!(
        BuildIdentity::new("1.0 beta", None, "orchbee", [0; 32]),
        Err(LinkError::InvalidBuild)
    );
    assert_eq!(
        encode_link_message(&LinkMessage::Frame(LinkFrame {
            call: call(1),
            kind: FrameKind::Result,
            record: Vec::new(),
        })),
        Err(LinkError::InvalidLength)
    );
}

/// Changing any one byte of a valid frame either is refused or decodes to a message that encodes
/// back to exactly the changed frame; the decoder never admits bytes it would not write.
#[test]
fn every_single_byte_change_is_refused_or_canonical() {
    for message in every_kind() {
        let frame = encode_link_message(&message).unwrap();
        if frame.len() > 4096 {
            continue;
        }
        let sender = message.sender();
        for index in 0..frame.len() {
            for delta in [1_u8, 0x80] {
                let mut changed = frame.clone();
                changed[index] = changed[index].wrapping_add(delta);
                if let Ok(decoded) = decode_link_message(&changed, sender) {
                    assert_eq!(encode_link_message(&decoded).unwrap(), changed);
                }
            }
        }
        for cut in 0..frame.len() {
            assert!(decode_link_message(&frame[..cut], sender).is_err());
            let mut extended = frame.clone();
            extended.push(0);
            assert!(decode_link_message(&extended, sender).is_err());
        }
    }
}

#[test]
fn tokens_compare_by_value_and_never_print() {
    let token = LinkToken::from_bytes([0xab; 32]);
    assert_eq!(LinkToken::parse(&token.to_hex()), Ok(token.clone()));
    assert_ne!(token, LinkToken::from_bytes([0xaa; 32]));
    assert_eq!(format!("{token:?}"), "LinkToken(..)");
    assert!(LinkToken::parse(&"AB".repeat(32)).is_err());
    assert!(WorkspaceId::parse(&"0".repeat(32)).is_err());
    assert_eq!(
        WorkspaceId::parse(&format!("{:032x}", 255)).unwrap(),
        workspace(255)
    );
}
