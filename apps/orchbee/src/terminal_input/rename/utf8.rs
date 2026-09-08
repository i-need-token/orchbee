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
}
