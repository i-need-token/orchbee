use std::thread;

use orchbee_files_protocol::{DrainReason, LinkRefusal};

use super::*;

#[test]
fn a_token_line_is_exactly_64_lowercase_digits_and_a_line_feed() {
    let digits = "0123456789abcdef".repeat(4);
    assert_eq!(
        read_token(&mut format!("{digits}\n").as_bytes()).unwrap(),
        LinkToken::parse(&digits).unwrap()
    );
    for line in [
        format!("{}\n", digits.to_uppercase()),
        format!("{digits}\r\n"),
        format!("{}\n", &digits[1..]),
        digits.clone(),
    ] {
        assert!(read_token(&mut line.as_bytes()).is_err(), "{line:?}");
    }
}

/// Admits over a connected pair whose Server end reads the hello and sends `answer`, or closes
/// without one, and checks the hello named the start's token and this build.
fn answered(answer: Option<LinkMessage>) -> Result<(), AdmissionError> {
    let (files, mut server) = LocalStream::pair().unwrap();
    let token = LinkToken::from_bytes([5; 32]);
    let server = thread::spawn(move || {
        let hello = read_link_message(&mut server, LinkSide::Files).unwrap();
        // The Server end stays open until admission read the answer, which a closed end could
        // discard.
        let server = answer.map(|answer| {
            write_link_message(&mut server, &answer).unwrap();
            server
        });
        (hello, server)
    });
    let admitted = admit(&files, token.clone());
    let (hello, _server) = server.join().unwrap();
    assert_eq!(
        hello,
        LinkMessage::Hello(LinkHello {
            build: this_build(),
            token,
        })
    );
    admitted
}

#[test]
fn the_servers_one_answer_decides_admission() {
    assert_eq!(answered(Some(LinkMessage::Admitted)), Ok(()));
    assert_eq!(
        answered(Some(LinkMessage::Refused(LinkRefusal::BuildMismatch))),
        Err(AdmissionError::Refused)
    );
    assert_eq!(
        answered(Some(LinkMessage::Drain(DrainReason::Session))),
        Err(AdmissionError::Handshake(io::ErrorKind::InvalidData))
    );
    assert!(matches!(answered(None), Err(AdmissionError::Handshake(_))));
}
