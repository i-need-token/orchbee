use super::*;
#[test]
fn machine_probe_refusals_and_transient_network_errors_have_distinct_retry_policy() {
    assert_eq!(
        probe_exit_kind(Some(255), b"Permission denied (publickey)."),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        probe_exit_kind(Some(255), b"Host key verification failed."),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(probe_exit_kind(Some(125), b""), io::ErrorKind::NotFound);
    assert_eq!(
        probe_exit_kind(Some(255), b"Connection timed out"),
        io::ErrorKind::ConnectionRefused
    );
}
