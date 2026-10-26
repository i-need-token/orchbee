use super::*;
use crate::terminal_coordinator::{BindingViewerTarget, WorkerPlacement};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, TerminalId, WorkerGeneration,
    WorkerId,
};

fn job(now: Instant) -> (Job, mpsc::Receiver<Option<CapturedAgentExplanation>>) {
    let (result, receiver) = mpsc::sync_channel(1);
    let job = Job {
        target: ProcessObservationTarget {
            session_id: orchbee_workbench_protocol::SessionId::parse(&"1".repeat(32)).unwrap(),
            binding: serde_json::from_value(
                serde_json::json!({"workerId":"2".repeat(32),"terminalId":"3".repeat(32)}),
            )
            .unwrap(),
            target: BindingViewerTarget {
                terminal_id: TerminalId::from_bytes([3; 16]).unwrap(),
                identity: HostTerminalIdentity::new(
                    HostCreationId::from_bytes([4; 16]).unwrap(),
                    HostEpoch::from_bytes([5; 16]).unwrap(),
                    HostTerminalId::from_bytes([6; 16]).unwrap(),
                ),
                worker: WorkerPlacement {
                    worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
                    generation: WorkerGeneration::new(std::num::NonZeroU64::MIN),
                },
            },
        },
        provider: AgentReportProvider::Codex,
        deadline: now + READ_WINDOW,
        cancelled: Arc::new(AtomicBool::new(false)),
        result,
    };
    (job, receiver)
}
#[test]
fn explanation_queue_keeps_one_active_one_waiting_and_exact_completion() {
    let requests = Requests::default();
    let stop = AtomicBool::new(false);
    let now = Instant::now();
    let (first, _first_read) = job(now);
    let (second, _second_read) = job(now);
    let (third, _third_read) = job(now);
    requests.enqueue(first, &stop).unwrap();
    let active = requests.take(now, &stop).unwrap();
    requests.enqueue(second, &stop).unwrap();
    assert_eq!(requests.enqueue(third, &stop), Err(Failure::ResourceLimit));
    assert!(requests.take(now, &stop).is_none());
    let (foreign, _foreign_read) = job(now);
    requests.finish(&foreign);
    assert!(requests.take(now, &stop).is_none());
    requests.finish(&active);
    assert!(requests.take(now, &stop).is_some());
}
#[test]
fn explanation_expiry_cancellation_and_shutdown_prevent_queued_work() {
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
                .take(
                    if case == "expired" {
                        now + READ_WINDOW
                    } else {
                        now
                    },
                    &stop
                )
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
