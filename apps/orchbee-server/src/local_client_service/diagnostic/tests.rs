use super::Diagnostic;
use std::{fmt, io, num::NonZeroU64};

#[derive(Debug)]
struct ContentBearing;

impl fmt::Display for ContentBearing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("terminal text and $HOME/secret")
    }
}

impl std::error::Error for ContentBearing {}

#[test]
fn lines_carry_only_static_identifiers_kind_and_generation() {
    let wrapped = io::Error::new(io::ErrorKind::InvalidData, ContentBearing);
    let line = Diagnostic::new("application-connection", "terminal-read")
        .caused_by(&wrapped)
        .connection(NonZeroU64::MIN)
        .line();
    assert_eq!(
        line,
        "orchbee-server diagnostic component=local-client-service \
         operation=application-connection stage=terminal-read cause=io:InvalidData connection=1"
    );
    let protocol = Diagnostic::new("session-stop", "deliver-result")
        .caused_by(&ContentBearing)
        .line();
    assert!(protocol.ends_with("stage=deliver-result cause=protocol"));
    assert!(!protocol.contains("terminal text") && !protocol.contains("secret"));
}
