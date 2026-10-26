use super::*;
use crate::local_client::cancellation::Cancellation;
use std::sync::Arc;

#[test]
fn poisoned_notification_closes_the_writer_and_faults_existing_borrowers() -> io::Result<()> {
    let signal = Arc::new(Signal::new()?);
    let producer = Arc::clone(&signal);
    let state = AtomicU8::new(0);
    let borrowed = Cancellation::new(Some(signal.wake(&state)));
    assert!(!borrowed.is_cancelled()?);
    let failed = std::thread::spawn(move || {
        let _held = producer.write.lock().unwrap();
        panic!("notification owner failed while holding its writer");
    });
    assert!(failed.join().is_err());
    signal.notify();
    assert!(borrowed.is_cancelled().is_err());
    Ok(())
}
