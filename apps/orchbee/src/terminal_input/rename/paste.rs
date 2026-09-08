use super::utf8::Scalar;

const END: &[u8; 6] = b"\x1b[201~";

pub(super) struct Paste {
    text: String,
    scalar: Scalar,
    remaining: usize,
    prefix: usize,
    rejected: bool,
}

pub(super) enum PasteStep {
    Pending,
    Fault,
    Complete(Option<String>),
}

impl Paste {
    pub(super) fn new(remaining: usize) -> Self {
        Self {
            text: String::new(),
            scalar: Scalar::default(),
            remaining: remaining.min(512),
            prefix: 0,
            rejected: false,
        }
    }

    pub(super) fn discard(&mut self) {
        self.rejected = true;
        self.text.clear();
    }

    pub(super) fn push(&mut self, byte: u8) -> PasteStep {
        let was_rejected = self.rejected;
        if byte == END[self.prefix] {
            self.prefix += 1;
            if self.prefix == END.len() {
                return PasteStep::Complete(
                    (!self.rejected && self.scalar.complete())
                        .then(|| std::mem::take(&mut self.text)),
                );
            }
        } else {
            for literal in &END[..self.prefix] {
                self.text_byte(*literal);
            }
            self.prefix = 0;
            if byte == END[0] {
                self.prefix = 1;
            } else {
                self.text_byte(byte);
            }
        }
        if !was_rejected && self.rejected {
            PasteStep::Fault
        } else {
            PasteStep::Pending
        }
    }

    fn text_byte(&mut self, byte: u8) {
        if self.rejected {
            return;
        }
        match self.scalar.push(byte) {
            Ok(Some(character)) if self.remaining > 0 => {
                self.text.push(character);
                self.remaining -= 1;
            }
            Ok(None) => {}
            Ok(Some(_)) | Err(()) => self.discard(),
        }
    }
}
