//! The answer delivery record `OBMA`: what a client that asked writes to a question's one-shot
//! endpoint. It is no frame of either connection. The codec writes into and reads from buffers the
//! caller owns, so the caller alone holds the answer's bytes and wipes them; nothing here copies,
//! formats or keeps an answer.

use crate::value::AnswerToken;

/// The record's first four bytes.
pub const ANSWER_MAGIC: [u8; 4] = *b"OBMA";
/// The `machine-hub` generation whose layout the record follows.
pub const ANSWER_GENERATION: u16 = 1;
/// The most answer bytes one record carries.
pub const MAXIMUM_ANSWER_BYTES: usize = 1024;
/// The bytes before the answer: magic, generation, token, and length.
pub const ANSWER_HEADER_BYTES: usize = 4 + 2 + 16 + 2;
/// The one byte the endpoint writes once it took a record.
pub const ANSWER_TAKEN: u8 = 0x06;

/// Why a record is not admissible. It never says what the answer held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnswerError {
    /// Not an `OBMA` record of generation 1.
    Foreign,
    /// Another question's token.
    Token,
    /// A length above 1,024 bytes.
    TooLong,
    /// The answer holds NUL, LF, or CR.
    Character,
}

/// Whether `answer` may be delivered: at most 1,024 bytes, without NUL, LF, or CR.
#[must_use]
pub fn admissible_answer(answer: &[u8]) -> bool {
    answer.len() <= MAXIMUM_ANSWER_BYTES
        && !answer.iter().any(|byte| matches!(byte, 0 | b'\n' | b'\r'))
}

/// Appends the record for `answer` under `token` to `record`, which the caller sized beforehand
/// to hold [`ANSWER_HEADER_BYTES`] plus the answer, so it never grows and leaves a copy behind.
///
/// # Errors
///
/// [`AnswerError::TooLong`] or [`AnswerError::Character`] for an answer that may not be delivered.
pub fn encode_answer(
    token: &AnswerToken,
    answer: &[u8],
    record: &mut Vec<u8>,
) -> Result<(), AnswerError> {
    if answer.len() > MAXIMUM_ANSWER_BYTES {
        return Err(AnswerError::TooLong);
    }
    if !admissible_answer(answer) {
        return Err(AnswerError::Character);
    }
    let length = u16::try_from(answer.len()).map_err(|_| AnswerError::TooLong)?;
    record.extend_from_slice(&ANSWER_MAGIC);
    record.extend_from_slice(&ANSWER_GENERATION.to_be_bytes());
    record.extend_from_slice(token.as_bytes());
    record.extend_from_slice(&length.to_be_bytes());
    record.extend_from_slice(answer);
    Ok(())
}

/// Reads a record's header for the question whose token is `expected` and returns the length of
/// the answer that follows. The token is compared in full whatever its first difference.
///
/// # Errors
///
/// [`AnswerError`] for a foreign header, another token, or a length above 1,024 bytes.
pub fn decode_answer_header(
    header: &[u8; ANSWER_HEADER_BYTES],
    expected: &AnswerToken,
) -> Result<usize, AnswerError> {
    let (magic, rest) = header.split_at(4);
    let (generation, rest) = rest.split_at(2);
    let (token, length) = rest.split_at(16);
    if magic != ANSWER_MAGIC || generation != ANSWER_GENERATION.to_be_bytes() {
        return Err(AnswerError::Foreign);
    }
    let difference = token
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |difference, (left, right)| difference | (left ^ right));
    if difference != 0 {
        return Err(AnswerError::Token);
    }
    let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
    if length > MAXIMUM_ANSWER_BYTES {
        return Err(AnswerError::TooLong);
    }
    Ok(length)
}

/// Overwrites `bytes` with zeros in a way the optimizer keeps, for a buffer that held an answer.
pub fn wipe(bytes: &mut [u8]) {
    bytes.fill(0);
    std::hint::black_box(&*bytes);
}
