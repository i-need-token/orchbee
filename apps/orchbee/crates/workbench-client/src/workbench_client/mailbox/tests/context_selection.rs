use super::*;

#[test]
fn context_selection_expiry_never_enters_state_callbacks_or_burns_identity() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    let mailbox = bootstrapped_mailbox(1, 5);
    let before = mailbox.cursors();
    for now in [deadlines.forward_cutoff(), deadlines.total()] {
        let mut state = 0;
        let result = mailbox.try_enqueue_context_selection_using(
            &mut state,
            deadlines,
            &FakeClock::new(now),
            |_, _| panic!("expired admission cannot create an intent"),
            |_, _: &Result<u64, AdmissionError<()>>| {
                panic!("deadline refusal cannot reacquire the gate through callbacks")
            },
        );
        assert_eq!(result, Ok(None));
        assert_eq!(state, 0);
        assert_eq!(mailbox.cursors(), before);
    }
}

#[test]
fn context_selection_budget_includes_mutex_contention_without_reacquiring_on_refusal() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    let (clock, reached, resume) = ObservedClock::with_contention_checkpoint(base);
    let clock = Arc::new(clock);
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    let before = mailbox.cursors();
    let held = mailbox.lock();
    let worker_mailbox = Arc::clone(&mailbox);
    let worker_clock = Arc::clone(&clock);
    let task = std::thread::spawn(move || {
        worker_mailbox.try_enqueue_context_selection_using(
            &mut (),
            deadlines,
            worker_clock.as_ref(),
            |_, _| panic!("expired admission cannot mutate canonical state"),
            |_, _: &Result<u64, AdmissionError<()>>| panic!("no callback after contention expiry"),
        )
    });
    reached.recv_timeout(Duration::from_secs(1)).unwrap();
    clock.set(deadlines.forward_cutoff());
    resume.send(()).unwrap();
    assert_eq!(task.join().unwrap(), Ok(None));
    drop(held);
    assert_eq!(mailbox.cursors(), before);
    assert!(mailbox.lock().queue.is_empty());
}
