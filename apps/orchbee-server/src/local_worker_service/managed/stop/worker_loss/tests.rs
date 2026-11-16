use super::WorkerLoss;

// Only a Worker exit that no stop explains fails the generation, and it counts once however many
// looks see it.
#[test]
fn a_worker_exit_outside_a_stop_counts_once() {
    let mut recorded = 0;
    let mut loss = WorkerLoss::default();
    loss.look(false, false, || recorded += 1);
    assert_eq!(recorded, 0, "a running Worker");
    for _ in 0..3 {
        loss.look(true, false, || recorded += 1);
    }
    assert_eq!(recorded, 1, "an exit outside a stop");
    loss.look(true, true, || recorded += 1);
    assert_eq!(recorded, 1);

    let mut stopped = WorkerLoss::default();
    stopped.look(true, true, || recorded += 1);
    stopped.look(true, false, || recorded += 1);
    assert_eq!(recorded, 1, "an exit first seen during a stop");
}
