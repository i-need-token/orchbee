//! The one fixed buffer a login field fills. Its bytes never grow into a new allocation, never
//! leave it except into the delivery record, and are wiped when cleared, taken, replaced or
//! dropped, panic unwinding included. Neither type has a debug form that shows them, and a copy of
//! the view shares the buffer instead of duplicating it.

use orchbee_machine_protocol::{MAXIMUM_ANSWER_BYTES, wipe};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

/// An answer's bytes in one fixed allocation, wiped on drop. It cannot be cloned or printed.
pub(crate) struct Answer {
    bytes: Box<[u8; MAXIMUM_ANSWER_BYTES]>,
    len: usize,
}

impl Answer {
    fn empty() -> Self {
        Self {
            bytes: Box::new([0; MAXIMUM_ANSWER_BYTES]),
            len: 0,
        }
    }

    /// A fixed answer Orchbee itself gives, such as `yes` to a trusted new host.
    pub(super) fn fixed(text: &'static [u8]) -> Self {
        let mut answer = Self::empty();
        let len = text.len().min(MAXIMUM_ANSWER_BYTES);
        answer.bytes[..len].copy_from_slice(&text[..len]);
        answer.len = len;
        answer
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn wipe(&mut self) {
        wipe(&mut self.bytes[..]);
        self.len = 0;
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        self.wipe();
    }
}

impl fmt::Debug for Answer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Answer")
    }
}

struct Field {
    answer: Answer,
    characters: usize,
}

/// The field's buffer behind a handle: every copy of the view that shows the field holds the same
/// buffer, so copying a view never copies an answer. Equal only to itself.
#[derive(Clone)]
pub(crate) struct SecretField(Arc<Mutex<Field>>);

impl SecretField {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(Field {
            answer: Answer::empty(),
            characters: 0,
        })))
    }

    fn lock(&self) -> MutexGuard<'_, Field> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends `character` in place; a control character, or one that would pass the answer's
    /// bound, is refused. The character's encoding is wiped from the stack after the copy.
    pub(crate) fn push(&self, character: char) -> bool {
        if character.is_control() {
            return false;
        }
        let mut scratch = [0; 4];
        let length = character.encode_utf8(&mut scratch).len();
        let mut field = self.lock();
        let start = field.answer.len;
        let admitted = start + length <= MAXIMUM_ANSWER_BYTES;
        if admitted {
            field.answer.bytes[start..start + length].copy_from_slice(&scratch[..length]);
            field.answer.len = start + length;
            field.characters += 1;
        }
        drop(field);
        wipe(&mut scratch);
        admitted
    }

    /// Removes the last character and wipes its bytes.
    pub(crate) fn pop(&self) -> bool {
        let mut field = self.lock();
        let end = field.answer.len;
        if end == 0 {
            return false;
        }
        let mut start = end - 1;
        while start > 0 && field.answer.bytes[start] & 0b1100_0000 == 0b1000_0000 {
            start -= 1;
        }
        wipe(&mut field.answer.bytes[start..end]);
        field.answer.len = start;
        field.characters -= 1;
        true
    }

    /// Wipes every byte typed so far.
    pub(crate) fn clear(&self) {
        let mut field = self.lock();
        field.answer.wipe();
        field.characters = 0;
    }

    /// How many characters were typed: all an interface may show of them, unless they begin
    /// [`Self::prefix_of`] a public word.
    pub(crate) fn characters(&self) -> usize {
        self.lock().characters
    }

    /// Whether the field holds `word`, ignoring ASCII case.
    pub(crate) fn reads(&self, word: &[u8]) -> bool {
        self.lock().answer.as_bytes().eq_ignore_ascii_case(word)
    }

    /// What was typed, while it is a prefix of the public `word` ignoring ASCII case; anything
    /// else, which may be a secret typed into the wrong field, stays unread.
    pub(crate) fn prefix_of(&self, word: &str) -> Option<String> {
        let field = self.lock();
        let typed = field.answer.as_bytes();
        (typed.len() <= word.len() && typed.eq_ignore_ascii_case(&word.as_bytes()[..typed.len()]))
            .then(|| String::from_utf8_lossy(typed).into_owned())
    }

    /// Moves the answer out for delivery and leaves the field empty: the bytes change owner, not
    /// place.
    pub(crate) fn take(&self) -> Answer {
        let mut field = self.lock();
        field.characters = 0;
        std::mem::replace(&mut field.answer, Answer::empty())
    }
}

impl fmt::Debug for SecretField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretField")
    }
}

impl PartialEq for SecretField {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SecretField {}
