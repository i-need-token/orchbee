//! Behavioral witnesses for complete replacement, source identity, bounded chunks and producer cleanup.
use super::*;
use base64::Engine;
use orchbee_workbench_protocol::{
    MAXIMUM_GRAPHICS_SET_BYTES, Navigation, PaneGraphicsFormat, PaneGraphicsFrame,
    PaneGraphicsPlacement, PanelNode, PanelRightClickTarget, Presentation, ProtocolVersion, SiteId,
    Tab, TerminalContext, TerminalPresentationId, WorkbenchPanelId, WorkbenchTabId,
    WorkbenchTerminalContextId,
};

#[test]
fn canonical_commit_retires_stream_pixels_without_waiting_for_a_graphics_reader() {
    let (snapshot, target) = fixture();
    let encoded = orchbee_workbench_protocol::serialize_document(
        &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot.clone()),
    )
    .unwrap();
    let mut projection =
        crate::workbench_projection::WorkbenchProjectionStore::load(&encoded).unwrap();
    let owner = projection.graphics();
    let connection = std::num::NonZeroU64::MIN;
    let stream =
        orchbee_workbench_protocol::WorkbenchRequestId::parse(&format!("{:032x}", 100)).unwrap();
    let source = PaneGraphicsAuthority::resolve(&snapshot, &target).unwrap();
    {
        let mut authority = owner.lock().unwrap();
        authority
            .open(&snapshot, connection, &stream, &target, "live", 0)
            .unwrap();
        let image = DecodedGraphicsImage::decode(
            &PaneGraphicsFrame {
                format: PaneGraphicsFormat::Rgba,
                width: 1,
                height: 1,
                data_base64: "AQIDBA==".into(),
                placement: PaneGraphicsPlacement::default(),
            },
            MAXIMUM_GRAPHICS_SET_BYTES,
            |_, _, _| panic!("raw frame must not decode PNG"),
        )
        .unwrap();
        authority
            .frame(
                &snapshot,
                PaneGraphicsFrameCommit {
                    connection,
                    stream_id: &stream,
                    source: &source,
                    sequence: WorkbenchRevision::new(1),
                    frame_revision: WorkbenchRevision::new(0),
                    image,
                },
            )
            .unwrap();
        assert_eq!(authority.bytes, 4);
    }
    let mut next = snapshot.clone();
    next.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    next.terminal_contexts[0].tabs.clear();
    let encoded = orchbee_workbench_protocol::serialize_document(
        &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(next),
    )
    .unwrap();
    projection.advance_projection(&encoded).unwrap();
    // Inspect the owner directly: no catalog, pixels, clear or stream-close triggered retirement.
    let mut authority = owner.lock().unwrap();
    assert_eq!(authority.bytes, 0);
    assert!(authority.layers.is_empty());
    assert_eq!(
        authority.close(connection, &stream),
        Err(Failure::StreamClosed)
    );
}

#[test]
fn decoded_resident_budget_refuses_a_third_resource_and_preserves_both_installed_layers() {
    let (snapshot, target) = fixture();
    let source = PaneGraphicsAuthority::resolve(&snapshot, &target).unwrap();
    let image = DecodedGraphicsImage::decode(
        &PaneGraphicsFrame {
            format: PaneGraphicsFormat::Png,
            width: 4096,
            height: 2048,
            data_base64: base64::engine::general_purpose::STANDARD.encode(png_header(4096, 2048)),
            placement: PaneGraphicsPlacement::default(),
        },
        MAXIMUM_GRAPHICS_SET_BYTES,
        |_, _, _| Ok(vec![0; MAXIMUM_GRAPHICS_RESIDENT_BYTES / 2]),
    )
    .unwrap();
    let mut authority = PaneGraphicsAuthority::default();
    authority
        .set(&snapshot, &source, "first", 0, image.clone())
        .unwrap();
    authority
        .set(&snapshot, &source, "second", 0, image.clone())
        .unwrap();
    let before = authority.catalog(&snapshot, &target).unwrap();
    assert_eq!(
        authority.set(&snapshot, &source, "third", 0, image),
        Err(Failure::MemoryLimit)
    );
    assert_eq!(authority.bytes, MAXIMUM_GRAPHICS_RESIDENT_BYTES);
    assert_eq!(authority.catalog(&snapshot, &target).unwrap(), before);
}
fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(width.to_be_bytes());
    png.extend(height.to_be_bytes());
    png.extend([0; 9]);
    png
}
fn fixture() -> (Snapshot, PaneGraphicsTarget) {
    let panel_id = WorkbenchPanelId::parse(&format!("{:032x}", 4)).unwrap();
    let presentation_id = TerminalPresentationId::parse(&format!("{:032x}", 5)).unwrap();
    let context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 2)).unwrap();
    let snapshot = Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&format!("{:032x}", 1)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            projects: vec![],
            standalone_terminal_context_id: context.clone(),
            workspace_order: vec![],
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: context,
            groups: vec![],
            tabs: vec![Tab {
                automatic_label: false,
                group_id: None,
                label: "Tab".into(),
                pinned: false,
                tab_id: WorkbenchTabId::parse(&format!("{:032x}", 3)).unwrap(),
                root: PanelNode::Panel {
                    right_click: PanelRightClickTarget::default(),
                    panel_id: panel_id.clone(),
                    child_presentations: vec![],
                    primary_presentation: Presentation {
                        presentation_id,
                        binding: None,
                        custom_label: None,
                        label: "Panel".into(),
                    },
                },
            }],
        }],
    };
    orchbee_workbench_protocol::serialize_document(
        &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot.clone()),
    )
    .unwrap();
    (
        snapshot,
        PaneGraphicsTarget {
            panel_id,
            presentation_id: None,
        },
    )
}
fn raw(format: PaneGraphicsFormat, bytes: &[u8]) -> PaneGraphicsFrame {
    PaneGraphicsFrame {
        format,
        width: 1,
        height: 1,
        data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        placement: PaneGraphicsPlacement::default(),
    }
}
fn image(bytes: &[u8]) -> DecodedGraphicsImage {
    DecodedGraphicsImage::decode(
        &raw(PaneGraphicsFormat::Rgba, bytes),
        MAXIMUM_GRAPHICS_SET_BYTES,
        |_, _, _| panic!("raw data cannot use PNG decoder"),
    )
    .unwrap()
}
fn id(value: u128) -> WorkbenchRequestId {
    WorkbenchRequestId::parse(&format!("{value:032x}")).unwrap()
}
fn connection(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}

#[test]
fn graphics_prepared_file_open_preserves_pixels_and_rechecks_primary_identity_before_commit() {
    let (snapshot, target) = fixture();
    let source = PaneGraphicsAuthority::resolve(&snapshot, &target).unwrap();
    let stream = id(200);
    let intent = PaneGraphicsOpen {
        connection: connection(1),
        stream_id: &stream,
        target: &target,
        layer_id: "primary",
        z_index: 0,
    };
    let mut owner = PaneGraphicsAuthority::default();
    owner
        .set(&snapshot, &source, "primary", 0, image(&[1, 2, 3, 255]))
        .unwrap();
    assert!(matches!(
        owner.prepare_open(&snapshot, &intent).unwrap(),
        PaneGraphicsOpenPreparation::Ready(_)
    ));
    assert_eq!(owner.bytes, 4);
    assert_eq!(layers(&mut owner, &snapshot, &target).len(), 1);
    let mut changed = snapshot.clone();
    let PanelNode::Panel {
        primary_presentation,
        child_presentations,
        ..
    } = &mut changed.terminal_contexts[0].tabs[0].root
    else {
        panic!("Panel");
    };
    child_presentations.push(primary_presentation.clone());
    primary_presentation.presentation_id =
        TerminalPresentationId::parse(&format!("{:032x}", 6)).unwrap();
    assert_eq!(
        owner.commit_open(
            &changed,
            &intent,
            &source,
            Some("C:\\private\\unused".into())
        ),
        Err(Failure::SourceChanged)
    );
    assert_eq!(owner.bytes, 4);
    assert!(owner.retained_file_directories().is_empty());
    let old = PaneGraphicsTarget {
        panel_id: source.panel_id,
        presentation_id: Some(source.presentation_id),
    };
    assert_eq!(layers(&mut owner, &changed, &old).len(), 1);
}

#[test]
fn graphics_file_directory_identity_is_stable_connection_owned_and_retired_with_its_producer() {
    let (snapshot, target) = fixture();
    let stream = id(201);
    let intent = PaneGraphicsOpen {
        connection: connection(1),
        stream_id: &stream,
        target: &target,
        layer_id: "live",
        z_index: 0,
    };
    let mut owner = PaneGraphicsAuthority::default();
    let PaneGraphicsOpenPreparation::Ready(source) =
        owner.prepare_open(&snapshot, &intent).unwrap()
    else {
        panic!("new stream");
    };
    let opened = owner
        .commit_open(
            &snapshot,
            &intent,
            &source,
            Some("C:\\private\\producer".into()),
        )
        .unwrap();
    assert_eq!(
        owner
            .commit_open(&snapshot, &intent, &source, Some("C:\\foreign".into()))
            .unwrap(),
        opened
    );
    assert_eq!(
        owner
            .producer_file_directory(connection(1), &stream)
            .unwrap()
            .as_deref(),
        Some("C:\\private\\producer")
    );
    assert_eq!(
        owner.producer_file_directory(connection(2), &stream),
        Err(Failure::StreamClosed)
    );
    assert_eq!(
        owner
            .connection_file_directory(&snapshot, &target, connection(2))
            .unwrap(),
        None
    );
    assert_eq!(
        owner
            .connection_file_directory(&snapshot, &target, connection(1))
            .unwrap()
            .as_deref(),
        Some("C:\\private\\producer")
    );
    let image = DecodedGraphicsImage::from_file_pixels(
        PaneGraphicsFormat::Bgra,
        1,
        1,
        PaneGraphicsPlacement::default(),
        vec![3, 2, 1, 255],
    )
    .unwrap();
    assert_eq!(image.bytes(), [1, 2, 3, 255]);
    owner
        .frame(
            &snapshot,
            PaneGraphicsFrameCommit {
                connection: connection(1),
                stream_id: &stream,
                source: &source,
                sequence: WorkbenchRevision::new(1),
                frame_revision: WorkbenchRevision::new(u64::MAX),
                image,
            },
        )
        .unwrap();
    assert_eq!(owner.retained_file_directories(), ["C:\\private\\producer"]);
    owner.retire_streams();
    assert!(owner.retained_file_directories().is_empty());
    assert_eq!(owner.bytes, 0);
    assert_eq!(
        owner.close(connection(1), &stream),
        Err(Failure::StreamClosed)
    );
}
fn layers(
    owner: &mut PaneGraphicsAuthority,
    snapshot: &Snapshot,
    target: &PaneGraphicsTarget,
) -> Vec<PaneGraphicsLayer> {
    let Outcome::Catalog { layers, .. } = owner.catalog(snapshot, target).unwrap() else {
        panic!("catalog")
    };
    layers
}
#[test]
fn graphics_raw_normalization_and_malformed_decoding_never_publish_partial_pixels() {
    let rgb = DecodedGraphicsImage::decode(
        &raw(PaneGraphicsFormat::Rgb, &[10, 20, 30]),
        512 * 1024,
        |_, _, _| panic!(),
    )
    .unwrap();
    let bgra = DecodedGraphicsImage::decode(
        &raw(PaneGraphicsFormat::Bgra, &[30, 20, 10, 255]),
        512 * 1024,
        |_, _, _| panic!(),
    )
    .unwrap();
    assert_eq!(rgb, bgra);
    assert_eq!(rgb.bytes(), [10, 20, 30, 255]);
    for bytes in [&[][..], &[1, 2, 3], &[1, 2, 3, 4, 5]] {
        assert_eq!(
            DecodedGraphicsImage::decode(
                &raw(PaneGraphicsFormat::Rgba, bytes),
                512 * 1024,
                |_, _, _| panic!()
            ),
            Err(Failure::InvalidImage)
        );
    }
    let png = raw(PaneGraphicsFormat::Png, &png_header(1, 1));
    assert_eq!(
        DecodedGraphicsImage::decode(&png, 512 * 1024, |_, _, _| Err(Failure::InvalidImage)),
        Err(Failure::InvalidImage)
    );
    assert_eq!(
        DecodedGraphicsImage::decode(&png, 512 * 1024, |_, _, _| Ok(vec![1; 3])),
        Err(Failure::InvalidImage)
    );
}
#[test]
fn graphics_complete_static_replacement_noop_and_stale_chunk_fences() {
    let (snapshot, target) = fixture();
    let source = PaneGraphicsAuthority::resolve(&snapshot, &target).unwrap();
    let mut owner = PaneGraphicsAuthority::default();
    assert!(matches!(
        owner
            .set(&snapshot, &source, "primary", 0, image(&[1, 2, 3, 255]))
            .unwrap(),
        Outcome::Applied { changed: true, .. }
    ));
    let first = layers(&mut owner, &snapshot, &target)
        .remove(0)
        .resource_revision;
    assert!(
        matches!(owner.set(&snapshot,&source,"primary",0,image(&[1,2,3,255])).unwrap(),Outcome::Applied {changed:false,revision,..} if revision==first)
    );
    assert!(matches!(
        owner
            .pixels(&snapshot, &target, "primary", first, 0, 4)
            .unwrap(),
        Outcome::Pixels {
            complete: true,
            total_bytes: 4,
            ..
        }
    ));
    assert!(matches!(
        owner
            .set(&snapshot, &source, "primary", 2, image(&[4, 5, 6, 255]))
            .unwrap(),
        Outcome::Applied { changed: true, .. }
    ));
    assert_eq!(
        owner.pixels(&snapshot, &target, "primary", first, 0, 4),
        Err(Failure::StaleResource)
    );
    assert_eq!(owner.bytes, 4);
    assert!(matches!(
        owner.clear(&snapshot, &target, "primary").unwrap(),
        Outcome::Applied { changed: true, .. }
    ));
    assert!(matches!(
        owner.clear(&snapshot, &target, "primary").unwrap(),
        Outcome::Applied { changed: false, .. }
    ));
    assert_eq!(owner.bytes, 0);
}
#[test]
fn graphics_stream_connection_conflict_sequence_and_eof_do_not_steal_replacements() {
    let (snapshot, target) = fixture();
    let mut owner = PaneGraphicsAuthority::default();
    let a = connection(1);
    let b = connection(2);
    let stream = id(10);
    let other = id(11);
    owner
        .open(&snapshot, a, &stream, &target, "primary", 1)
        .unwrap();
    assert_eq!(
        owner.open(&snapshot, b, &other, &target, "primary", 1),
        Err(Failure::Conflict)
    );
    assert_eq!(
        owner.clear(&snapshot, &target, "primary"),
        Err(Failure::Conflict)
    );
    let source = owner
        .producer_source(&snapshot, a, &stream, WorkbenchRevision::new(1))
        .unwrap();
    assert_eq!(
        owner.set(&snapshot, &source, "primary", 0, image(&[1, 2, 3, 4])),
        Err(Failure::Conflict)
    );
    assert_eq!(
        owner.producer_source(&snapshot, b, &stream, WorkbenchRevision::new(1)),
        Err(Failure::StreamClosed)
    );
    owner
        .frame(
            &snapshot,
            PaneGraphicsFrameCommit {
                connection: a,
                stream_id: &stream,
                source: &source,
                sequence: WorkbenchRevision::new(1),
                frame_revision: WorkbenchRevision::new(u64::MAX),
                image: image(&[1, 2, 3, 255]),
            },
        )
        .unwrap();
    assert_eq!(
        owner.producer_source(&snapshot, a, &stream, WorkbenchRevision::new(1)),
        Err(Failure::InvalidSequence)
    );
    assert_eq!(owner.close(b, &stream), Err(Failure::StreamClosed));
    owner.disconnect(a);
    assert_eq!(owner.bytes, 0);
    assert!(layers(&mut owner, &snapshot, &target).is_empty());
    owner
        .open(&snapshot, b, &other, &target, "primary", 0)
        .unwrap();
    owner.disconnect(a);
    assert!(
        owner
            .producer_source(&snapshot, b, &other, WorkbenchRevision::new(1))
            .is_ok()
    );
    assert_eq!(owner.close(a, &stream), Err(Failure::StreamClosed));
    owner.close(b, &other).unwrap();
}
#[test]
fn graphics_layer_capacity_retirement_and_revision_exhaustion_preserve_cleanup() {
    let (mut snapshot, target) = fixture();
    let source = PaneGraphicsAuthority::resolve(&snapshot, &target).unwrap();
    let mut owner = PaneGraphicsAuthority::default();
    for n in 0..16 {
        owner
            .set(
                &snapshot,
                &source,
                &format!("layer-{n}"),
                n,
                image(&[1, 2, 3, 4]),
            )
            .unwrap();
    }
    assert_eq!(
        owner.set(&snapshot, &source, "extra", 0, image(&[1, 2, 3, 4])),
        Err(Failure::LayerLimit)
    );
    snapshot.terminal_contexts[0].tabs[0].label = "Renamed".into();
    assert_eq!(layers(&mut owner, &snapshot, &target).len(), 16);
    snapshot.terminal_contexts[0].tabs.clear();
    owner.retain(&snapshot).unwrap();
    assert_eq!(owner.bytes, 0);
    assert!(owner.layers.is_empty());
    let (snapshot, target) = fixture();
    owner
        .open(&snapshot, connection(1), &id(10), &target, "primary", 0)
        .unwrap();
    owner.revision = u64::MAX;
    let source = owner
        .producer_source(&snapshot, connection(1), &id(10), WorkbenchRevision::new(1))
        .unwrap();
    assert_eq!(
        owner.frame(
            &snapshot,
            PaneGraphicsFrameCommit {
                connection: connection(1),
                stream_id: &id(10),
                source: &source,
                sequence: WorkbenchRevision::new(1),
                frame_revision: WorkbenchRevision::new(1),
                image: image(&[1, 2, 3, 4])
            }
        ),
        Err(Failure::RevisionExhausted)
    );
    owner.disconnect(connection(1));
    assert!(owner.layers.is_empty());
    assert_eq!(
        owner.catalog(&snapshot, &target),
        Err(Failure::RevisionExhausted)
    );
}
