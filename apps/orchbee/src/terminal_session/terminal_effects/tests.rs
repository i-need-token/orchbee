use super::*;
use crate::terminal_session::state::{AttachmentKey, PaneKey};
use orchbee_workbench_protocol::{WorkbenchPanelId, WorkbenchTabId};

fn pane(incarnation: u32) -> PaneRef {
    PaneRef::new(
        PaneKey::new(
            AttachmentKey::new(
                1,
                WorkbenchTabId::parse("00000000000000000000000000000001").unwrap(),
            ),
            WorkbenchPanelId::parse("00000000000000000000000000000002").unwrap(),
        ),
        incarnation,
    )
}

#[test]
fn cancellation_wins_before_os_dispatch_but_not_after_it_begins() {
    let mut queue = Queue::default();
    assert!(queue.enqueue_selection(pane(1), NonZeroU64::MIN, "old".into()));
    let job = queue.dispatch().unwrap();
    queue.cancel_pane(&pane(1));
    assert!(!job.begin());
    assert_eq!(queue.take_outcome(), None);
    drop(job);
    assert_eq!(
        queue.take_outcome(),
        Some((Kind::Clipboard, Outcome::Cancelled))
    );
    assert!(queue.enqueue_selection(pane(2), NonZeroU64::MIN, "new".into()));
    let job = queue.dispatch().unwrap();
    assert!(job.begin());
    queue.close();
    assert_eq!(queue.take_outcome(), None);
    job.settle(Outcome::Delivered);
    assert_eq!(
        queue.take_outcome(),
        Some((Kind::Clipboard, Outcome::Delivered))
    );
    assert!(queue.dispatch().is_none());
}

#[test]
fn adapter_loss_is_definite_before_dispatch_and_unknown_after_dispatch_without_replay() {
    for (begin, expected) in [(false, Outcome::Failed), (true, Outcome::Unknown)] {
        let mut queue = Queue::default();
        assert!(queue.enqueue_selection(pane(1), NonZeroU64::MIN, "once".into()));
        let job = queue.dispatch().unwrap();
        if begin {
            assert!(job.begin());
        }
        drop(job);
        assert_eq!(queue.take_outcome(), Some((Kind::Clipboard, expected)));
        assert_eq!(queue.take_outcome(), None);
        assert!(queue.dispatch().is_none());
    }
}

#[test]
fn keyboard_copy_completion_retains_exact_episode_export_and_pane_identity() {
    let mut queue = Queue::default();
    let episode = NonZeroU64::new(4).unwrap();
    let generation = NonZeroU64::new(5).unwrap();
    let export = NonZeroU64::new(6).unwrap();
    assert!(queue.enqueue_copy_selection(pane(2), generation, export, episode, "exact".into(),));
    let job = queue.dispatch().unwrap();
    assert!(job.begin());
    job.settle(Outcome::Delivered);
    drop(job);
    assert_eq!(
        queue.take_completion(),
        Some(Completion {
            key: pane(2),
            scope: Scope::Selection {
                generation,
                export: Some(export),
                episode: Some(episode),
            },
            kind: Kind::Clipboard,
            outcome: Outcome::Delivered,
        })
    );
}

#[test]
fn replacing_one_incarnation_preserves_other_fifo_entries_and_rejects_truncated_unicode() {
    let mut queue = Queue::default();
    assert!(queue.enqueue_selection(pane(1), NonZeroU64::MIN, "stale".into()));
    assert!(queue.enqueue_selection(pane(2), NonZeroU64::MIN, "中文e\u{301}\nnext".into()));
    assert!(!queue.enqueue_selection(pane(2), NonZeroU64::MIN, "cut\0off".into()));
    queue.cancel_pane(&pane(1));
    let job = queue.dispatch().unwrap();
    assert_eq!(job.text(), Some("中文e\u{301}\nnext"));
    assert!(job.begin());
    assert!(!job.begin());
    job.settle(Outcome::Delivered);
    assert_eq!(
        queue.take_outcome(),
        Some((Kind::Clipboard, Outcome::Delivered))
    );
    assert_eq!(queue.bytes, 0);
}

fn identity() -> (AttachmentId, HostEpoch) {
    (
        AttachmentId::from_bytes([1; 16]).unwrap(),
        HostEpoch::from_bytes([2; 16]).unwrap(),
    )
}

#[test]
fn new_selection_cancels_only_selection_and_keeps_mixed_program_fifo_without_replay() {
    let mut queue = Queue::default();
    let (attachment, epoch) = identity();
    assert!(queue.enqueue_selection(pane(1), NonZeroU64::MIN, "selection".into()));
    assert!(queue.enqueue_program(pane(1), attachment, epoch, Payload::Bell));
    assert!(queue.enqueue_program(
        pane(1),
        attachment,
        epoch,
        Payload::Clipboard("program\n中文".into())
    ));
    queue.cancel_selection(&pane(1));
    let bell = queue.dispatch().unwrap();
    assert_eq!(bell.kind(), Kind::Bell);
    assert!(bell.text().is_none());
    assert!(queue.dispatch().is_none());
    assert!(bell.begin());
    bell.settle(Outcome::Unknown);
    drop(bell);
    assert_eq!(queue.take_outcome(), Some((Kind::Bell, Outcome::Unknown)));
    let clipboard = queue.dispatch().unwrap();
    assert_eq!(clipboard.text(), Some("program\n中文"));
    assert!(clipboard.begin());
    clipboard.settle(Outcome::Delivered);
    drop(clipboard);
    assert_eq!(
        queue.take_outcome(),
        Some((Kind::Clipboard, Outcome::Delivered))
    );
    assert!(queue.dispatch().is_none());
    assert_eq!(queue.bytes, 0);
}

#[test]
fn program_fences_use_attachment_and_epoch_not_selection_generation() {
    let (attachment, epoch) = identity();
    let mut queue = Queue::default();
    assert!(queue.enqueue_selection(pane(1), NonZeroU64::MIN, "old generation".into()));
    assert!(queue.enqueue_program(pane(1), attachment, epoch, Payload::Bell));
    queue.reconcile(|_, scope| scope == Scope::Program { attachment, epoch });
    let bell = queue.dispatch().unwrap();
    queue.cancel_selection(&pane(1));
    assert!(!bell.cancelled());
    queue.cancel_pane(&pane(2));
    assert!(!bell.cancelled());
    queue.cancel_pane(&pane(1));
    assert!(bell.cancelled());
    assert!(!bell.begin());
    assert_eq!(queue.take_outcome(), None);
    drop(bell);
    assert_eq!(queue.take_outcome(), Some((Kind::Bell, Outcome::Cancelled)));
}

#[test]
fn capacity_never_evicts_and_cancelled_active_text_stays_charged_until_adapter_release() {
    let (attachment, epoch) = identity();
    let mut queue = Queue::default();
    assert!(queue.enqueue_selection(pane(1), NonZeroU64::MIN, "x".repeat(BYTE_CAPACITY)));
    let job = queue.dispatch().unwrap();
    queue.cancel_pane(&pane(1));
    assert!(!queue.enqueue_program(
        pane(2),
        attachment,
        epoch,
        Payload::Clipboard("next".into())
    ));
    assert_eq!(queue.bytes, BYTE_CAPACITY);
    assert_eq!(queue.take_outcome(), None);
    drop(job);
    assert_eq!(
        queue.take_outcome(),
        Some((Kind::Clipboard, Outcome::Cancelled))
    );
    for _ in 0..CAPACITY {
        assert!(queue.enqueue_program(pane(2), attachment, epoch, Payload::Bell));
    }
    assert!(!queue.enqueue_program(
        pane(2),
        attachment,
        epoch,
        Payload::Clipboard("late".into())
    ));
    for _ in 0..CAPACITY {
        let job = queue.dispatch().unwrap();
        assert_eq!(job.kind(), Kind::Bell);
        assert!(job.begin());
        job.settle(Outcome::Delivered);
        drop(job);
        assert_eq!(queue.take_outcome(), Some((Kind::Bell, Outcome::Delivered)));
    }
    assert!(queue.dispatch().is_none());
}

fn effect_record(
    attachment: AttachmentId,
    epoch: HostEpoch,
    sequence: u64,
    clipboard: bool,
) -> Vec<u8> {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, TerminalBell,
        TerminalClipboardWriteRequest, TerminalEffectEvent, encode_terminal_effect_event,
        encode_transport_envelope,
    };
    let sequence = NonZeroU64::new(sequence).unwrap();
    let effect = if clipboard {
        TerminalEffectEvent::ClipboardWriteRequest(TerminalClipboardWriteRequest {
            attachment_id: attachment,
            sequence,
            text: "中文e\u{301}\nnext".into(),
        })
    } else {
        TerminalEffectEvent::Bell(TerminalBell {
            attachment_id: attachment,
            sequence,
        })
    };
    let encoded = encode_terminal_effect_event(&effect).unwrap();
    encode_transport_envelope(&Envelope {
        correlation_id: None,
        host_epoch: Some(epoch),
        kind: encoded.kind,
        optional_message: false,
        payload: &encoded.payload,
        payload_encoding: PayloadEncoding::ControlJson,
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
    })
    .unwrap()
}

#[test]
fn effect_streams_advance_independently_and_foreign_or_repeated_records_do_not_mutate_them() {
    let (attachment, epoch) = identity();
    let mut stream = Stream::new(attachment, epoch);
    let foreign = AttachmentId::from_bytes([3; 16]).unwrap();
    let other_epoch = HostEpoch::from_bytes([4; 16]).unwrap();
    for clipboard in [false, true] {
        assert!(
            stream
                .receive(&effect_record(foreign, epoch, 3, clipboard))
                .is_err()
        );
        assert!(
            stream
                .receive(&effect_record(attachment, other_epoch, 3, clipboard))
                .is_err()
        );
        let admitted = stream
            .receive(&effect_record(attachment, epoch, 2, clipboard))
            .unwrap()
            .unwrap();
        assert_eq!(
            admitted.kind(),
            if clipboard {
                Kind::Clipboard
            } else {
                Kind::Bell
            }
        );
        assert!(
            stream
                .receive(&effect_record(attachment, epoch, 2, clipboard))
                .is_err()
        );
        assert!(
            stream
                .receive(&effect_record(attachment, epoch, 1, clipboard))
                .is_err()
        );
        assert!(
            stream
                .receive(&effect_record(attachment, epoch, 3, clipboard))
                .unwrap()
                .is_some()
        );
    }
    let mut replacement = Stream::new(foreign, other_epoch);
    assert!(
        replacement
            .receive(&effect_record(foreign, other_epoch, 1, true))
            .unwrap()
            .is_some()
    );
}

#[test]
fn program_text_bound_preserves_line_breaks_and_rejects_terminal_controls_before_admission() {
    let (attachment, epoch) = identity();
    let mut queue = Queue::default();
    let limit = orchbee_terminal_protocol::MAXIMUM_CLIPBOARD_TEXT_BYTES;
    assert!(queue.enqueue_program(
        pane(1),
        attachment,
        epoch,
        Payload::Clipboard("x".repeat(limit))
    ));
    for text in [
        "x".repeat(limit + 1),
        "escape\x1b[0m".into(),
        "del\x7f".into(),
        "nul\0".into(),
    ] {
        assert!(!queue.enqueue_program(pane(1), attachment, epoch, Payload::Clipboard(text)));
    }
    assert!(queue.enqueue_program(
        pane(1),
        attachment,
        epoch,
        Payload::Clipboard("\t中文e\u{301}\r\n".into())
    ));
    let first = queue.dispatch().unwrap();
    assert_eq!(first.text().unwrap().len(), limit);
    drop(first);
    assert_eq!(
        queue.take_outcome(),
        Some((Kind::Clipboard, Outcome::Failed))
    );
    let second = queue.dispatch().unwrap();
    assert_eq!(second.text(), Some("\t中文e\u{301}\r\n"));
}
