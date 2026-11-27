use std::{
    sync::{Arc, Barrier},
    time::Duration,
};

use orchbee_files_protocol::{ErrorCode, Operation, Token};

use super::{Helpers, MAXIMUM_ABANDONED};

#[test]
fn stuck_helpers_refuse_new_work_until_they_return() {
    let helpers = Helpers::with_deadline(Duration::from_millis(50));
    assert_eq!(helpers.run(Operation::FsStat, || 7), Ok(7));
    // Each stuck helper waits for the release, with the test as the last party.
    let release = Arc::new(Barrier::new(MAXIMUM_ABANDONED + 1));
    for _ in 0..MAXIMUM_ABANDONED {
        let release = Arc::clone(&release);
        let error = helpers
            .run(Operation::FsGrant, move || {
                release.wait();
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Timeout);
    }
    let refused = helpers.run(Operation::FsStat, || 7).unwrap_err();
    assert_eq!(
        refused.reason,
        Some(Token::new("stuck-filesystem").unwrap())
    );
    release.wait();
    // The released helpers return on their own threads; work is admitted once they have.
    let admitted = (0..200).find_map(|_| {
        std::thread::sleep(Duration::from_millis(10));
        helpers.run(Operation::FsStat, || 7).ok()
    });
    assert_eq!(admitted, Some(7));
}
