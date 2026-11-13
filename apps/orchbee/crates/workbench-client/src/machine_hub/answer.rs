//! Delivering an answer to a question's one-shot endpoint in the contract's `OBMA` record. The
//! endpoint must be served by this user; the record is built in a buffer sized up front, written
//! once and wiped, and the endpoint's receipt is awaited for at most 5 seconds. The caller owns the
//! answer's bytes and wipes them; nothing here logs, formats or keeps them.

use orchbee_local_ipc::LocalStream;
use orchbee_machine_protocol::{
    ANSWER_HEADER_BYTES, ANSWER_TAKEN, admissible_answer, encode_answer, hub::AnswerEndpoint, wipe,
};
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

/// The bound on reaching the endpoint, writing the record and reading its receipt.
const DELIVERY: Duration = Duration::from_secs(5);

/// Why an answer was not taken. None says what the answer held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryFailure {
    /// More than 1,024 bytes, or NUL, LF or CR.
    Inadmissible,
    /// The endpoint is gone or is not served by this user: the question closed.
    Unreachable,
    /// The endpoint closed without its receipt: another token, a second delivery, or a question
    /// closed meanwhile.
    Refused,
}

/// A buffer that held an answer, wiped when dropped. It has no debug form.
struct Wiped(Vec<u8>);

impl Drop for Wiped {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

/// Delivers `answer` for the question whose endpoint is `endpoint` and waits for its receipt.
///
/// # Errors
///
/// [`DeliveryFailure`] when the answer may not be sent or the endpoint did not take it.
pub fn deliver(endpoint: &AnswerEndpoint, answer: &[u8]) -> Result<(), DeliveryFailure> {
    if !admissible_answer(answer) {
        return Err(DeliveryFailure::Inadmissible);
    }
    let deadline = Instant::now() + DELIVERY;
    // The connection proves the server runs as this user before a byte is written.
    let stream = LocalStream::connect_before(Path::new(endpoint.endpoint.as_str()), deadline)
        .map_err(|_| DeliveryFailure::Unreachable)?;
    let mut record = Wiped(Vec::with_capacity(ANSWER_HEADER_BYTES + answer.len()));
    encode_answer(&endpoint.token, answer, &mut record.0)
        .map_err(|_| DeliveryFailure::Inadmissible)?;
    let remaining = |deadline: Instant| {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(DeliveryFailure::Refused)
    };
    stream
        .set_write_timeout(Some(remaining(deadline)?))
        .map_err(|_| DeliveryFailure::Refused)?;
    let written = (&stream).write_all(&record.0);
    drop(record);
    written.map_err(|_| DeliveryFailure::Refused)?;
    stream
        .set_read_timeout(Some(remaining(deadline)?))
        .map_err(|_| DeliveryFailure::Refused)?;
    let mut receipt = [0; 1];
    match (&stream).read(&mut receipt) {
        Ok(1) if receipt[0] == ANSWER_TAKEN => Ok(()),
        _ => Err(DeliveryFailure::Refused),
    }
}
