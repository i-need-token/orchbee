use super::*;

#[test]
fn selected_duplicate_admission_waits_atomically_for_poll_reply_causal_and_lease_without_burning_identity()
 {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(7, 40);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let before = mailbox.cursors();
    let busy = || {
        assert_eq!(
            mailbox.try_enqueue_settled_duplicate_for(
                &mut (),
                |_, _, _| -> Result<CreateMarker, ()> { panic!("busy must not invoke builder") },
                |_, _| {}
            ),
            Err(AdmissionError::Busy)
        );
        assert_eq!(mailbox.cursors(), before);
    };
    busy(); // Queued poll.
    let lease = mailbox.try_recv().unwrap().unwrap();
    let key = lease.poll_key().unwrap();
    busy(); // Worker lease, before reply.
    mailbox
        .publish_poll_result_before_using(key.clone(), None, base + Duration::from_secs(30), &clock)
        .unwrap();
    busy(); // Event and causal cell still held.
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    busy(); // UI consumed the event but has not settled the draw.
    mailbox
        .deposit_causal(&key, CausalSettlement::NoFeedback)
        .unwrap();
    busy(); // Worker must accept that settlement.
    assert_eq!(
        mailbox
            .take_causal_before_using(&key, base + Duration::from_secs(30), &clock)
            .unwrap(),
        Some(CausalSettlement::NoFeedback)
    );
    busy(); // Even empty cells cannot bypass the active lease.
    mailbox.finish_lease(lease).unwrap();
    let key = mailbox
        .try_enqueue_settled_duplicate_for(
            &mut (),
            |_, generation, request| Ok::<_, ()>(create_marker(base, generation, request)),
            |_, _| {},
        )
        .unwrap();
    assert_eq!(key.reserved_generation(), before.0 + 1);
    assert_eq!(mailbox.cursors().2, before.2 + 1);
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Create(_)));
    mailbox.finish_lease(lease).unwrap();
}
