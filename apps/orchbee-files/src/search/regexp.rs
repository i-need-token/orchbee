//! The structure of a JavaScript regular expression without the `u` flag, as regexpp 3.1 parses
//! it with Annex B, which VS Code uses to place each `\n` escape: in a class, a class range, a
//! quantified element or a lookbehind. Offsets are UTF-16 code units. A pattern regexpp rejects
//! has no structure: unterminated groups and classes, an unmatched `)`, nothing to repeat, a
//! reversed class range or quantifier, an invalid group or group name, and a trailing `\`.

/// One parsed alternative's elements.
pub(super) type Alternative = Vec<Element>;

pub(super) enum Element {
    /// A character; `newline` when it is the escape `\n`.
    Character(Atom),
    /// An assertion other than a lookaround, a character set or a backreference.
    Other,
    Class(Class),
    /// A capturing or non-capturing group.
    Group(Vec<Alternative>),
    Lookaround {
        behind: bool,
        alternatives: Vec<Alternative>,
    },
    Quantifier(Box<Element>),
}

/// A character's place in the pattern.
#[derive(Clone, Copy)]
pub(super) struct Atom {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) newline: bool,
}

pub(super) struct Class {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) negated: bool,
    pub(super) items: Vec<ClassItem>,
}

pub(super) enum ClassItem {
    Character(Atom),
    Range(Atom, Atom),
    Set,
}

/// regexpp rejects the pattern.
#[derive(Debug)]
pub(super) struct Rejected;

/// The pattern's alternatives.
pub(super) fn parse(pattern: &[u16]) -> Result<Vec<Alternative>, Rejected> {
    let mut parser = Parser {
        text: pattern,
        at: 0,
        named: has_named_group(pattern),
        names: Vec::new(),
        references: Vec::new(),
    };
    let alternatives = parser.disjunction()?;
    if parser.at < pattern.len() {
        return Err(Rejected);
    }
    if parser
        .references
        .iter()
        .any(|reference| !parser.names.contains(reference))
    {
        return Err(Rejected);
    }
    Ok(alternatives)
}

fn has_named_group(pattern: &[u16]) -> bool {
    let mut at = 0;
    while at + 3 < pattern.len() {
        if pattern[at] == u16::from(b'\\') {
            at += 2;
            continue;
        }
        if pattern[at..at + 3] == [u16::from(b'('), u16::from(b'?'), u16::from(b'<')]
            && !matches!(pattern[at + 3], 0x3D | 0x21)
        {
            return true;
        }
        at += 1;
    }
    false
}

struct Parser<'a> {
    text: &'a [u16],
    at: usize,
    /// Whether the pattern has a named group, under which `\k` must name one.
    named: bool,
    names: Vec<Vec<u16>>,
    references: Vec<Vec<u16>>,
}

fn is(unit: Option<u16>, ch: u8) -> bool {
    unit == Some(u16::from(ch))
}

fn is_hex(unit: u16) -> bool {
    u8::try_from(unit).is_ok_and(|byte| byte.is_ascii_hexdigit())
}

fn hex(units: &[u16]) -> u32 {
    units.iter().fold(0, |value, unit| {
        value * 16
            + char::from_u32(u32::from(*unit))
                .and_then(|ch| ch.to_digit(16))
                .unwrap_or(0)
    })
}

impl Parser<'_> {
    fn peek(&self) -> Option<u16> {
        self.text.get(self.at).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u16> {
        self.text.get(self.at + offset).copied()
    }

    fn starts(&self, prefix: &str) -> bool {
        let prefix: Vec<u16> = prefix.encode_utf16().collect();
        self.text
            .get(self.at..)
            .is_some_and(|rest| rest.starts_with(&prefix))
    }

    fn disjunction(&mut self) -> Result<Vec<Alternative>, Rejected> {
        let mut alternatives = vec![self.alternative()?];
        while is(self.peek(), b'|') {
            self.at += 1;
            alternatives.push(self.alternative()?);
        }
        Ok(alternatives)
    }

    fn alternative(&mut self) -> Result<Alternative, Rejected> {
        let mut elements = Vec::new();
        while let Some(unit) = self.peek() {
            if unit == u16::from(b'|') || unit == u16::from(b')') {
                break;
            }
            elements.push(self.term()?);
        }
        Ok(elements)
    }

    fn term(&mut self) -> Result<Element, Rejected> {
        let (element, quantifiable) = if self.starts("^") || self.starts("$") {
            self.at += 1;
            (Element::Other, false)
        } else if self.starts("\\b") || self.starts("\\B") {
            self.at += 2;
            (Element::Other, false)
        } else if self.starts("(?=") || self.starts("(?!") {
            self.at += 3;
            (self.lookaround(false)?, true)
        } else if self.starts("(?<=") || self.starts("(?<!") {
            self.at += 4;
            (self.lookaround(true)?, false)
        } else {
            (self.atom()?, true)
        };
        if quantifiable && self.quantifier()? {
            return Ok(Element::Quantifier(Box::new(element)));
        }
        Ok(element)
    }

    fn lookaround(&mut self, behind: bool) -> Result<Element, Rejected> {
        let alternatives = self.disjunction()?;
        self.close()?;
        Ok(Element::Lookaround {
            behind,
            alternatives,
        })
    }

    fn close(&mut self) -> Result<(), Rejected> {
        if !is(self.peek(), b')') {
            return Err(Rejected);
        }
        self.at += 1;
        Ok(())
    }

    /// A quantifier after an element, if one follows; a lazy `?` belongs to it.
    fn quantifier(&mut self) -> Result<bool, Rejected> {
        match self.peek() {
            Some(0x2A | 0x2B | 0x3F) => self.at += 1,
            Some(0x7B) => match self.braced() {
                Some((end, minimum, maximum)) => {
                    if maximum.is_some_and(|maximum| maximum < minimum) {
                        return Err(Rejected);
                    }
                    self.at = end;
                }
                None => return Ok(false),
            },
            _ => return Ok(false),
        }
        if is(self.peek(), b'?') {
            self.at += 1;
        }
        Ok(true)
    }

    /// A braced quantifier at the current offset: its end, minimum and maximum.
    fn braced(&self) -> Option<(usize, f64, Option<f64>)> {
        let mut at = self.at + 1;
        let digits = |at: &mut usize| {
            let start = *at;
            while self
                .text
                .get(*at)
                .is_some_and(|unit| (0x30..=0x39).contains(unit))
            {
                *at += 1;
            }
            (*at > start).then(|| {
                self.text[start..*at]
                    .iter()
                    .fold(0.0, |value, unit| value * 10.0 + f64::from(unit - 0x30))
            })
        };
        let minimum = digits(&mut at)?;
        let maximum = if self.text.get(at) == Some(&u16::from(b',')) {
            at += 1;
            digits(&mut at)
        } else {
            Some(minimum)
        };
        (self.text.get(at) == Some(&u16::from(b'}'))).then_some((at + 1, minimum, maximum))
    }

    fn atom(&mut self) -> Result<Element, Rejected> {
        let start = self.at;
        let Some(unit) = self.peek() else {
            return Err(Rejected);
        };
        match unit {
            0x2E => {
                self.at += 1;
                Ok(Element::Other)
            }
            0x5C => self.escape(),
            0x28 => self.group(),
            0x5B => self.class().map(Element::Class),
            0x2A | 0x2B | 0x3F => Err(Rejected),
            0x7B if self.braced().is_some() => Err(Rejected),
            _ => {
                self.at += 1;
                Ok(Element::Character(Atom {
                    start,
                    end: self.at,
                    newline: false,
                }))
            }
        }
    }

    fn escape(&mut self) -> Result<Element, Rejected> {
        let start = self.at;
        let Some(next) = self.peek_at(1) else {
            return Err(Rejected);
        };
        let character = |parser: &mut Self, length: usize, newline: bool| {
            parser.at += length;
            Ok(Element::Character(Atom {
                start,
                end: parser.at,
                newline,
            }))
        };
        match u8::try_from(next).unwrap_or(0) {
            b'd' | b'D' | b's' | b'S' | b'w' | b'W' => {
                self.at += 2;
                Ok(Element::Other)
            }
            b'1'..=b'9' => {
                self.at += 2;
                while self
                    .peek()
                    .is_some_and(|unit| (0x30..=0x39).contains(&unit))
                {
                    self.at += 1;
                }
                Ok(Element::Other)
            }
            b'k' if self.named => {
                self.at += 2;
                let name = self.group_name()?;
                self.references.push(name);
                Ok(Element::Other)
            }
            b'c' if self.peek_at(2).is_some_and(|unit| {
                u8::try_from(unit).is_ok_and(|byte| byte.is_ascii_alphabetic())
            }) =>
            {
                character(self, 3, false)
            }
            b'c' => character(self, 1, false),
            b'x' if self
                .text
                .get(self.at + 2..self.at + 4)
                .is_some_and(|digits| digits.iter().all(|unit| is_hex(*unit))) =>
            {
                character(self, 4, false)
            }
            b'u' if self
                .text
                .get(self.at + 2..self.at + 6)
                .is_some_and(|digits| digits.iter().all(|unit| is_hex(*unit))) =>
            {
                character(self, 6, false)
            }
            b'n' => character(self, 2, true),
            _ => character(self, 2, false),
        }
    }

    fn group(&mut self) -> Result<Element, Rejected> {
        if self.starts("(?:") {
            self.at += 3;
        } else if self.starts("(?<") {
            self.at += 2;
            let name = self.group_name()?;
            if self.names.contains(&name) {
                return Err(Rejected);
            }
            self.names.push(name);
        } else if self.starts("(?") {
            return Err(Rejected);
        } else {
            self.at += 1;
        }
        let alternatives = self.disjunction()?;
        self.close()?;
        Ok(Element::Group(alternatives))
    }

    /// `<name>` at the current offset.
    fn group_name(&mut self) -> Result<Vec<u16>, Rejected> {
        if !is(self.peek(), b'<') {
            return Err(Rejected);
        }
        self.at += 1;
        let start = self.at;
        while let Some(unit) = self.peek() {
            if unit == u16::from(b'>') {
                break;
            }
            let ch = char::from_u32(u32::from(unit)).unwrap_or('\u{FFFD}');
            let admitted = ch == '$'
                || ch == '_'
                || ch.is_alphabetic()
                || (self.at > start
                    && (ch.is_alphanumeric() || matches!(ch, '\u{200C}' | '\u{200D}')));
            if !admitted {
                return Err(Rejected);
            }
            self.at += 1;
        }
        if self.at == start || !is(self.peek(), b'>') {
            return Err(Rejected);
        }
        let name = self.text[start..self.at].to_vec();
        self.at += 1;
        Ok(name)
    }

    fn class(&mut self) -> Result<Class, Rejected> {
        let start = self.at;
        self.at += 1;
        let negated = is(self.peek(), b'^');
        if negated {
            self.at += 1;
        }
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None => return Err(Rejected),
                Some(0x5D) => break,
                Some(_) => {}
            }
            let (first, first_value) = self.class_atom()?;
            let ranged = is(self.peek(), b'-')
                && self.peek_at(1).is_some_and(|unit| unit != u16::from(b']'));
            if !ranged {
                items.push(first.map_or(ClassItem::Set, ClassItem::Character));
                continue;
            }
            let dash = Atom {
                start: self.at,
                end: self.at + 1,
                newline: false,
            };
            self.at += 1;
            let (second, second_value) = self.class_atom()?;
            match (first, first_value, second, second_value) {
                (Some(first), Some(low), Some(second), Some(high)) => {
                    if low > high {
                        return Err(Rejected);
                    }
                    items.push(ClassItem::Range(first, second));
                }
                _ => {
                    items.push(first.map_or(ClassItem::Set, ClassItem::Character));
                    items.push(ClassItem::Character(dash));
                    items.push(second.map_or(ClassItem::Set, ClassItem::Character));
                }
            }
        }
        self.at += 1;
        Ok(Class {
            start,
            end: self.at,
            negated,
            items,
        })
    }

    /// One class atom: its character, or `None` for a set, and its value.
    fn class_atom(&mut self) -> Result<(Option<Atom>, Option<u32>), Rejected> {
        let start = self.at;
        let unit = self.peek().ok_or(Rejected)?;
        let atom = |parser: &mut Self, length: usize, value: u32, newline: bool| {
            parser.at += length;
            Ok((
                Some(Atom {
                    start,
                    end: parser.at,
                    newline,
                }),
                Some(value),
            ))
        };
        if unit != u16::from(b'\\') {
            return atom(self, 1, u32::from(unit), false);
        }
        let next = self.peek_at(1).ok_or(Rejected)?;
        let digit = |unit: Option<u16>, high: u16| unit.filter(|unit| (0x30..=high).contains(unit));
        match u8::try_from(next).unwrap_or(0) {
            b'd' | b'D' | b's' | b'S' | b'w' | b'W' => {
                self.at += 2;
                Ok((None, None))
            }
            b'b' => atom(self, 2, 8, false),
            b'n' => atom(self, 2, 10, true),
            b't' => atom(self, 2, 9, false),
            b'v' => atom(self, 2, 11, false),
            b'f' => atom(self, 2, 12, false),
            b'r' => atom(self, 2, 13, false),
            b'c' => match self.peek_at(2).and_then(|unit| u8::try_from(unit).ok()) {
                Some(control) if control.is_ascii_alphanumeric() || control == b'_' => {
                    atom(self, 3, u32::from(control) % 32, false)
                }
                _ => atom(self, 1, u32::from(b'\\'), false),
            },
            b'x' if self
                .text
                .get(self.at + 2..self.at + 4)
                .is_some_and(|digits| digits.iter().all(|unit| is_hex(*unit))) =>
            {
                let value = hex(&self.text[self.at + 2..self.at + 4]);
                atom(self, 4, value, false)
            }
            b'u' if self
                .text
                .get(self.at + 2..self.at + 6)
                .is_some_and(|digits| digits.iter().all(|unit| is_hex(*unit))) =>
            {
                let value = hex(&self.text[self.at + 2..self.at + 6]);
                atom(self, 6, value, false)
            }
            b'0'..=b'7' => {
                let mut length = 2;
                let mut value = u32::from(next - 0x30);
                let limit = if next <= u16::from(b'3') { 2 } else { 1 };
                while length - 1 <= limit {
                    let Some(more) = digit(self.peek_at(length), u16::from(b'7')) else {
                        break;
                    };
                    value = value * 8 + u32::from(more - 0x30);
                    length += 1;
                }
                atom(self, length, value, false)
            }
            _ => atom(self, 2, u32::from(next), false),
        }
    }
}
