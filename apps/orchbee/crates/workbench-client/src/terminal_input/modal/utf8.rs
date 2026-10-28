//! Accumulates one valid UTF-8 scalar for modal text admission and reports malformed or
//! incomplete input separately. It does not recognize terminal control frames or decide an
//! editor's text capacity.

#[derive(Default)]
pub(super) struct Scalar {
    bytes: [u8; 4],
    length: usize,
}

impl Scalar {
    pub(super) fn push(&mut self, byte: u8) -> Result<Option<char>, ()> {
        self.bytes[self.length] = byte;
        self.length += 1;
        match std::str::from_utf8(&self.bytes[..self.length]) {
            Ok(text) => {
                let value = text.chars().next().ok_or(())?;
                self.length = 0;
                Ok(Some(value))
            }
            Err(error) if error.error_len().is_none() && self.length < 4 => Ok(None),
            Err(_) => {
                self.length = 0;
                Err(())
            }
        }
    }

    pub(super) const fn complete(&self) -> bool {
        self.length == 0
    }

    /// How many continuation bytes the in-progress scalar still expects.
    /// After the bounded wait expires the frame keeps that many late bytes
    /// owned instead of releasing them as fresh input.
    pub(super) fn remaining(&self) -> usize {
        if self.length == 0 {
            return 0;
        }
        let expected: usize = match self.bytes[0] {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        expected.saturating_sub(self.length)
    }
}
