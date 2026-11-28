//! The refined values File Bar JSON records share: texts and lists bounded as the schema bounds
//! them, safe integers, canonical counters, opaque cursors, content digests, native paths, file
//! names, relative paths and Git names. Each value refuses what `schemas/files/v1` refuses and also
//! what only a binding can check, such as UTF-8 byte lengths, so a value that deserializes is valid
//! wherever the protocol carries it. Whether a path is inside a Workspace's scope is not decided
//! here.

use std::{fmt, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// The longest path a record carries, in UTF-8 bytes.
pub const MAXIMUM_PATH_BYTES: usize = 4096;
/// The largest integer a JSON number carries exactly, `2^53 - 1`.
pub const MAXIMUM_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A value outside its protocol domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidValue;

impl fmt::Display for InvalidValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid File Bar value")
    }
}

impl std::error::Error for InvalidValue {}

/// Implements string serialization for a refined text type whose `new` validates.
macro_rules! text_serde {
    ($name:ident $(<$($bound:ident),+>)?, $expecting:literal) => {
        impl$(<$(const $bound: usize),+>)? Serialize for $name$(<$($bound),+>)? {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de $(, $(const $bound: usize),+)?> Deserialize<'de> for $name$(<$($bound),+>)? {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                Self::new(text).map_err(|_| de::Error::custom($expecting))
            }
        }
    };
}

/// Text of `MIN` to `MAX` UTF-8 bytes without NUL.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Text<const MIN: usize, const MAX: usize>(String);

impl<const MIN: usize, const MAX: usize> Text<MIN, MAX> {
    /// Admits text within the bounds that holds no NUL.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for text outside the bounds or holding NUL.
    pub fn new(text: impl Into<String>) -> Result<Self, InvalidValue> {
        let text = text.into();
        if (MIN..=MAX).contains(&text.len()) && !text.contains('\0') {
            Ok(Self(text))
        } else {
            Err(InvalidValue)
        }
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(Text<MIN, MAX>, "text outside its bounds");

/// An absolute path in the machine's native spelling, as Clients send it and `orchbee-files`
/// reports it: 1 to [`MAXIMUM_PATH_BYTES`] UTF-8 bytes without NUL. `orchbee-files` refuses the
/// relative, dot-segment, device, verbatim and UNC forms when it resolves one; the codec does not
/// know the machine's platform.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FilePath(String);

impl FilePath {
    /// Admits 1 to [`MAXIMUM_PATH_BYTES`] bytes without NUL.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty, overlong or NUL-holding path.
    pub fn new(path: impl Into<String>) -> Result<Self, InvalidValue> {
        let path = path.into();
        if (1..=MAXIMUM_PATH_BYTES).contains(&path.len()) && !path.contains('\0') {
            Ok(Self(path))
        } else {
            Err(InvalidValue)
        }
    }

    /// The path as the machine spells it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(
    FilePath,
    "an absolute path of at most 4096 bytes without NUL"
);

/// One name in a directory, as a Client names a file to look for: 1 to 1,024 UTF-8 bytes without
/// NUL or `/`, and neither `.` nor `..`. `orchbee-files` refuses again what the machine's own rules
/// refuse, such as a backslash or a reserved device name on Windows.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileName(String);

impl FileName {
    /// Admits 1 to 1,024 bytes that hold no NUL or `/` and are neither `.` nor `..`.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for any other text.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        let name = name.into();
        if is_name(&name) && name.len() <= 1024 {
            Ok(Self(name))
        } else {
            Err(InvalidValue)
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(
    FileName,
    "a file name without NUL or '/' that is not '.' or '..'"
);

/// A path below a directory the request names, in `/`-separated names whatever the machine: 1 to
/// [`MAXIMUM_PATH_BYTES`] UTF-8 bytes of names that [`FileName`] admits, so it never leaves that
/// directory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativePath(String);

impl RelativePath {
    /// Admits 1 to [`MAXIMUM_PATH_BYTES`] bytes of names joined by single `/`.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty or overlong path, a leading, trailing or doubled `/`, or a
    /// `.`, `..` or NUL-holding name.
    pub fn new(path: impl Into<String>) -> Result<Self, InvalidValue> {
        let path = path.into();
        if path.len() <= MAXIMUM_PATH_BYTES && path.split('/').all(is_name) {
            Ok(Self(path))
        } else {
            Err(InvalidValue)
        }
    }

    /// The path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its names, outermost first.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

text_serde!(
    RelativePath,
    "a relative path of '/'-separated names, none '.' or '..', at most 4096 bytes without NUL"
);

/// Whether `name` is one non-empty name: no NUL or `/`, and neither `.` nor `..`.
fn is_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\0'])
}

/// A VS Code encoding identifier, such as `utf8` or `windows1252`: 1 to 24 lowercase letters and
/// digits. Whether `orchbee-files` decodes it is its own answer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EncodingName(String);

impl EncodingName {
    /// Admits 1 to 24 lowercase ASCII letters and digits.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for any other text.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        let name = name.into();
        if (1..=24).contains(&name.len())
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        {
            Ok(Self(name))
        } else {
            Err(InvalidValue)
        }
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(EncodingName, "a VS Code encoding identifier");

/// A full Git object name: SHA-1 or SHA-256 in lowercase hexadecimal.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CommitName(String);

impl CommitName {
    /// Admits 40 or 64 lowercase hexadecimal digits.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for any other text.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        let name = name.into();
        if matches!(name.len(), 40 | 64) && name.bytes().all(is_lowercase_hex) {
            Ok(Self(name))
        } else {
            Err(InvalidValue)
        }
    }

    /// The object name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(CommitName, "a full lowercase Git object name");

/// A remote or branch name a Client names: 1 to 1,024 UTF-8 bytes without NUL, never starting with
/// `-`, so it can never read as an option where Git takes it as an argument.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GitName(String);

impl GitName {
    /// Admits 1 to 1,024 bytes without NUL that do not start with `-`.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for any other text.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidValue> {
        let name = name.into();
        if (1..=1024).contains(&name.len()) && !name.starts_with('-') && !name.contains('\0') {
            Ok(Self(name))
        } else {
            Err(InvalidValue)
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(
    GitName,
    "a Git name without NUL that does not start with '-'"
);

/// An open kebab-case token, such as an `unavailable` reason or a Git error code: 1 to 64 bytes of
/// lowercase words joined by single hyphens. A reader treats one it does not know as absent.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Token(String);

impl Token {
    /// Admits lowercase ASCII words of letters and digits joined by single hyphens, starting with
    /// a letter.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for any other text.
    pub fn new(token: impl Into<String>) -> Result<Self, InvalidValue> {
        let token = token.into();
        let valid = (1..=64).contains(&token.len())
            && token.as_bytes()[0].is_ascii_lowercase()
            && token.split('-').all(|word| {
                !word.is_empty()
                    && word
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            });
        if valid {
            Ok(Self(token))
        } else {
            Err(InvalidValue)
        }
    }

    /// The token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

text_serde!(Token, "a kebab-case token");

/// An opaque page continuation, 32 lowercase hexadecimal digits, which only the page that
/// returned it gives meaning.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PageCursor([u8; 16]);

impl PageCursor {
    /// The cursor these bytes spell.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// The cursor's bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Serialize for PageCursor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&lowercase_hex(&self.0))
    }
}

impl<'de> Deserialize<'de> for PageCursor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        decode_lowercase_hex::<16>(&text)
            .map(Self)
            .ok_or_else(|| de::Error::custom("a page cursor of 32 lowercase hexadecimal digits"))
    }
}

/// The SHA-256 digest of a file's bytes, 64 lowercase hexadecimal digits: what a read's final
/// chunk reports and a save sends back as its precondition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// The digest these bytes spell.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The digest's bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&lowercase_hex(&self.0))
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        decode_lowercase_hex::<32>(&text)
            .map(Self)
            .ok_or_else(|| de::Error::custom("a SHA-256 digest of 64 lowercase hexadecimal digits"))
    }
}

/// An unsigned 64-bit value in canonical decimal: no sign, no leading zero, at most
/// `18446744073709551615`. JSON numbers lose such values, so records spell them as strings.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Counter(u64);

impl Counter {
    /// The counter at `value`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The counter's value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Serialize for Counter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Counter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        parse_canonical_u64(&text)
            .map(Self)
            .ok_or_else(|| de::Error::custom("a canonical unsigned 64-bit decimal"))
    }
}

/// An integer from `MIN` to `MAX`, which never exceeds [`MAXIMUM_SAFE_INTEGER`]. Any JSON
/// spelling of such an integer is admitted, `1.0` and `1e0` included, as JSON Schema's `integer`
/// admits them; it is written without a fraction or exponent.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Integer<const MIN: u64, const MAX: u64>(u64);

impl<const MIN: u64, const MAX: u64> Integer<MIN, MAX> {
    /// Admits `value` within the bounds.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for a value outside them.
    pub fn new(value: u64) -> Result<Self, InvalidValue> {
        if (MIN..=MAX.min(MAXIMUM_SAFE_INTEGER)).contains(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidValue)
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A count or size: any safe nonnegative integer.
pub type SafeInteger = Integer<0, MAXIMUM_SAFE_INTEGER>;

impl<const MIN: u64, const MAX: u64> Serialize for Integer<MIN, MAX> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0)
    }
}

impl<'de, const MIN: u64, const MAX: u64> Deserialize<'de> for Integer<MIN, MAX> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = deserializer.deserialize_any(SafeIntegerVisitor)?;
        u64::try_from(value)
            .ok()
            .and_then(|value| Self::new(value).ok())
            .ok_or_else(|| de::Error::custom("an integer outside its bounds"))
    }
}

/// A signed safe integer, such as a modification time in milliseconds since the Unix epoch or an
/// operating system's error number.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Signed(i64);

impl Signed {
    /// Admits `value` when its magnitude is at most [`MAXIMUM_SAFE_INTEGER`].
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for a larger magnitude.
    pub fn new(value: i64) -> Result<Self, InvalidValue> {
        if value.unsigned_abs() <= MAXIMUM_SAFE_INTEGER {
            Ok(Self(value))
        } else {
            Err(InvalidValue)
        }
    }

    /// The value.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl Serialize for Signed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.0)
    }
}

impl<'de> Deserialize<'de> for Signed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = deserializer.deserialize_any(SafeIntegerVisitor)?;
        Self::new(value).map_err(|_| de::Error::custom("an integer outside the safe range"))
    }
}

/// Reads any JSON spelling of a safe integer.
struct SafeIntegerVisitor;

impl de::Visitor<'_> for SafeIntegerVisitor {
    type Value = i64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a safe integer")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<i64, E> {
        if value > MAXIMUM_SAFE_INTEGER {
            return Err(E::custom("an integer outside the safe range"));
        }
        i64::try_from(value).map_err(|_| E::custom("an integer outside the safe range"))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<i64, E> {
        if value.unsigned_abs() > MAXIMUM_SAFE_INTEGER {
            return Err(E::custom("an integer outside the safe range"));
        }
        Ok(value)
    }

    // A finite integral value within the safe range converts exactly.
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<i64, E> {
        if value.is_finite() && value.trunc() == value && value.abs() <= MAXIMUM_SAFE_INTEGER as f64
        {
            Ok(value as i64)
        } else {
            Err(E::custom("a finite integer within the safe range"))
        }
    }
}

/// `MIN` to `MAX` values, as the schema bounds the array.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct List<T, const MIN: usize, const MAX: usize>(Vec<T>);

impl<T, const MIN: usize, const MAX: usize> List<T, MIN, MAX> {
    /// Admits `values` when their number is within the bounds.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for too few or too many values.
    pub fn new(values: Vec<T>) -> Result<Self, InvalidValue> {
        if (MIN..=MAX).contains(&values.len()) {
            Ok(Self(values))
        } else {
            Err(InvalidValue)
        }
    }

    /// The values.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// The values, owned.
    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<T: Serialize, const MIN: usize, const MAX: usize> Serialize for List<T, MIN, MAX> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>, const MIN: usize, const MAX: usize> Deserialize<'de>
    for List<T, MIN, MAX>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(ListVisitor(PhantomData))
    }
}

/// Reads a bounded array, refusing one more value than `MAX` before reading on.
struct ListVisitor<T, const MIN: usize, const MAX: usize>(PhantomData<T>);

impl<'de, T: Deserialize<'de>, const MIN: usize, const MAX: usize> de::Visitor<'de>
    for ListVisitor<T, MIN, MAX>
{
    type Value = List<T, MIN, MAX>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "an array of {MIN} to {MAX} values")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element()? {
            if values.len() == MAX {
                return Err(de::Error::custom("too many values"));
            }
            values.push(value);
        }
        List::new(values).map_err(|_| de::Error::custom("too few values"))
    }
}

/// `serde` adapter for an optional field: when present it must hold a value, since the schema
/// refuses `null` where it allows absence.
pub(crate) fn some<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// `serde` adapter for a required field that may be `null`, which serde would otherwise also read
/// from a missing field.
pub(crate) fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

/// `serde` predicate: an optional boolean is written only when true, as absent means false.
pub(crate) const fn is_false(value: &bool) -> bool {
    !*value
}

const fn is_lowercase_hex(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'a'..=b'f')
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// The lowercase hexadecimal spelling of `bytes`.
pub(crate) fn lowercase_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    text
}

/// The `BYTES` bytes exactly `2 * BYTES` lowercase hexadecimal digits spell.
pub(crate) fn decode_lowercase_hex<const BYTES: usize>(text: &str) -> Option<[u8; BYTES]> {
    let text = text.as_bytes();
    if text.len() != BYTES * 2 {
        return None;
    }
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    let mut bytes = [0_u8; BYTES];
    for (byte, pair) in bytes.iter_mut().zip(text.chunks_exact(2)) {
        *byte = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(bytes)
}

/// The value of a canonical unsigned 64-bit decimal: digits only, no leading zero but `0` itself,
/// and no overflow.
pub(crate) fn parse_canonical_u64(text: &str) -> Option<u64> {
    let canonical = !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'));
    if canonical { text.parse().ok() } else { None }
}
