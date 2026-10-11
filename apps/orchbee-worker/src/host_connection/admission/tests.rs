use super::*;
use std::{thread, time::Duration};

#[test]
fn byte_progress_cannot_extend_the_original_host_admission_deadline() -> io::Result<()> {
    let (stream, mut peer) = UnixStream::pair()?;
    stream.set_nonblocking(true)?;
    let peer = thread::spawn(move || {
        for _ in 0..5 {
            thread::sleep(Duration::from_millis(40));
            if peer.write_all(b"x").is_err() {
                break;
            }
        }
    });
    let started = Instant::now();
    let mut admission = Admission {
        stream,
        deadline: started + Duration::from_millis(100),
    };
    let mut bytes = [0; 5];
    assert_eq!(
        admission.read_exact(&mut bytes).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert!(started.elapsed() < Duration::from_millis(190));
    assert_eq!(
        admission.write_all(b"after expiry").unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    drop(admission);
    peer.join().unwrap();
    Ok(())
}
