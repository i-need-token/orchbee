use super::*;
const WINDOW: Duration = Duration::from_secs(4);

fn job(now: Instant) -> (Job<(), ()>, mpsc::Receiver<Option<()>>) {
    let (result, receiver) = mpsc::sync_channel(1);
    let job = Job {
        parameters: (),
        deadline: now + WINDOW,
        cancelled: Arc::new(AtomicBool::new(false)),
        result,
    };
    (job, receiver)
}
#[test]
fn queue_keeps_one_active_one_waiting_and_exact_completion() {
    let requests = Requests::default();
    let stop = AtomicBool::new(false);
    let now = Instant::now();
    let (first, _first_read) = job(now);
    let (second, _second_read) = job(now);
    let (third, _third_read) = job(now);
    requests.enqueue(first, &stop).unwrap();
    assert!(requests.waiting());
    let active = requests.take(now, &stop).unwrap();
    requests.enqueue(second, &stop).unwrap();
    assert!(!requests.waiting());
    assert_eq!(requests.enqueue(third, &stop), Err(Refusal::Busy));
    assert!(requests.take(now, &stop).is_none());
    let (foreign, _foreign_read) = job(now);
    requests.finish(&foreign);
    assert!(requests.take(now, &stop).is_none());
    requests.finish(&active);
    assert!(requests.waiting());
    assert!(requests.take(now, &stop).is_some());
}
#[test]
fn expiry_cancellation_and_shutdown_prevent_queued_work() {
    for case in ["expired", "cancelled", "stopped"] {
        let requests = Requests::default();
        let stop = AtomicBool::new(false);
        let now = Instant::now();
        let (queued, receiver) = job(now);
        let token = Arc::clone(&queued.cancelled);
        requests.enqueue(queued, &stop).unwrap();
        if case == "cancelled" {
            token.store(true, Ordering::Release);
        }
        if case == "stopped" {
            stop.store(true, Ordering::Release);
        }
        assert!(
            requests
                .take(if case == "expired" { now + WINDOW } else { now }, &stop)
                .is_none()
        );
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
    let requests = Requests::default();
    let stop = AtomicBool::new(false);
    let now = Instant::now();
    let (first, _first_read) = job(now);
    requests.enqueue(first, &stop).unwrap();
    let active = requests.take(now, &stop).unwrap();
    let (second, second_read) = job(now);
    requests.enqueue(second, &stop).unwrap();
    requests.cancel();
    assert!(active.cancelled.load(Ordering::Acquire));
    assert!(matches!(
        second_read.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    requests.finish(&active);
    assert!(requests.take(now, &stop).is_none());
}
